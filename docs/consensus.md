# Multi-Paxos interface scaffold

The server currently runs **LocalAgreement**, a volatile, single-node adapter.
It provides **no distributed fault tolerance**. The Phase 1/2 paths deliberately
return `Unsupported`/`NotLeader`, never a fake commitment. No peer transport,
election, quorum algorithm, durable storage, or distributed match setup exists.

## What was already here

- `src/bin/server.rs`: Axum WebSockets, one task per connection; browser heartbeats
  are independent of consensus heartbeats. The existing 25 ms / 1 s settings stay.
- `src/lobby.rs`: one Tokio actor owns players, readiness and the current round.
  Connections send events over bounded channels; outbound queues are nonblocking.
- `src/game.rs`: window/clock-free game rules, simultaneous collisions, and an
  injected random generator. Previously, directions mutated Game immediately and
  the lobby called Game::tick every 100 ms using a process-local RNG.
- The browser sends directions and draws authoritative snapshots. The standalone
  desktop/demo binaries are separate and retain their existing 10 Hz clocks.

## Files and ownership

- `consensus/types.rs`: shared IDs, messages, values, effects and recovery handoff.
- `consensus/acceptor.rs`: **one** `AcceptorState` per node/log, owned by the future
  peer actor. Both groups borrow the same `&mut AcceptorState`; do not clone it or
  put independent promise/accepted maps in the two protocol implementations.
- **Group 1:** `consensus/leadership.rs`, including distinct-member Promise history
  collection, leader timers, ballot takeover and the authoritative recovery plan.
- **Group 2:** `consensus/agreement.rs` for Phase 2; `batcher.rs`, `application.rs`
  and `session.rs` provide the working batching/execution boundary to build on.
- `consensus/tests.rs`: contract tests with a controllable delayed agreement stub.

Methods return `Effect::Send`, `Schedule`, `Established`, `Ready`, or `Chosen`.
The future peer actor executes network/timer effects and feeds messages back
with their sender NodeId. Protocol code owns no socket, timer task or game.
`TickSession` consumes Chosen effects and returns applied records plus remaining
effects to its host. The local lobby has no peer effects to dispatch.

## Normal tick flow

1. Browser sends `direction` with a per-connection `sequence`. The server derives
   `ClientRequestId { client_id: joined_player_id, sequence }`; clients cannot
   choose someone else's player ID. Older clients may omit sequence, in which
   case the server allocates it (those clients cannot identify retransmissions).
2. The actor queues `PlayerInput`. The batcher retains request IDs and receipt
   order. At the next deadline, the session closes ONE batch, including an empty
   batch when there are no inputs. Later arrivals stay in the open queue.
3. `Agreement::submit_fresh(&batch)` returns `Submitted(slot)`, `Busy`,
   `NotLeader`, `Recovering`, or `Unsupported`. Submitted means proposal admission,
   **not chosen or applied**. The caller keeps its batch on every outcome.
4. LocalAgreement assigns increasing slots and immediately returns Chosen. The
   future Phase 2 must instead await a majority of distinct configured acceptors
   acknowledging the **same ballot, slot and value**. Duplicate responses count
   once; outsiders and stale/mismatched responses never contribute.
5. `GameAdapter::receive_chosen` stores entries beyond a gap and applies only the
   contiguous prefix, exactly once. Conflicting decisions or wrong tick numbers
   fail closed. A no-op consumes a slot but neither moves snakes nor consumes RNG.
6. For a TickBatch, execute inputs in chosen order, then call Game::tick once.
   The existing first-valid-turn-per-player rule stays: reversals and subsequent
   turns are committed but ignored. Previously applied request IDs are skipped.
7. Only after application, send `input_applied { sequence, tick, duplicate }` and
   broadcast the resulting board/result. A success acknowledges committed
   processing, not a guarantee the turn was legal. Retries of applied requests
   receive the cached result while the round exists; pending retries get no early
   success. Receipts use best-effort existing outbound queues, not reliable delivery.

Slots and ticks both start at zero **per match**, but they are different types.
For example: slot 0 = no-op; slot 1 = tick 0; slot 2 = tick 1. Never derive game
clock numbers from slot numbers or wall-clock elapsed time.

## Scheduling and responsiveness

The server has periodic deadlines at `1_000_000_000 / 30` nanoseconds (~33.3 ms).
Tokio `MissedTickBehavior::Skip` drops missed opportunities instead of accumulating
catch-up work. The clock resets only when a new round starts, never after a
commitment. A late commitment can therefore be followed by a shorter wait until
the next scheduled deadline; it does not force another full 33.3 ms delay.

One unresolved fresh batch blocks additional fresh proposals until **chosen and
applied**, even when a chosen entry is waiting behind a log gap. Deadlines while
busy do not create more closed batches. The actor still handles inputs, socket
closures, and other events; a future agreement must never block awaiting a quorum.
Slow/unavailable consensus slows or pauses simulation without inventing ticks.
The input queue and retained history are intentionally unbounded for this teaching
scaffold; before production, define backpressure and snapshot/compaction policy.

## Leadership/recovery handoff (students implement the protocol)

1. On loss/suspicion of leadership, call `session.begin_recovery()` to stop fresh
   submissions without discarding pending or newly arriving inputs. Suspicion is
   not proof of failure. Group 1 enters Attempting with a higher `(counter, node)`.
2. Prepare/Accept handlers use the shared acceptor operations. Reject ballots
   below the highest promise. Promise/Accepted replies must follow the state
   update (and durable write if restart recovery is added). Same-ballot/slot
   conflicting values are rejected. Accept also updates the promise.
3. Group 1 collects complete histories from a majority of **distinct fixed
   members**. `AcceptedHistory::Page` is explicitly incomplete; collect all pages
   at the relevant ballot before treating missing slots as empty. Prepare starts
   at slot zero; page responses identify from_slot and the next request position.
   A final page (next_slot=None) completes history only when the entire page chain
   from zero has been collected. The current
   acceptor returns Complete and retains all accepted history; no compaction.
4. Emit `Established(RecoveryPlan)` with the established ballot and highest-ballot
   accepted value per slot. Preserve values/tick numbers exactly and cover every
   slot through the recovery boundary, using no-ops only for proven-empty holes.
   Enter Recovering, not Ready. Pass the plan to Group 2's `begin_recovery`.
5. Group 2 calls `submit_recovered(slot, value)` to re-propose exact recovered
   values under the new ballot, without fresh ticks or pipelining. Do not execute
   merely accepted entries. Route chosen/catch-up decisions through `on_chosen`.
   Report ordered applied-through progress to both groups.
6. Only after **all required slots are chosen AND applied**, Group 1 may emit
   Ready. The coordinator calls `session.reconcile_after_recovery(plan.through)`
   before `agreement.ready_for_fresh(ballot)`. Reconciliation rejects an unapplied
   boundary, clears the old submitted-slot marker, removes applied request IDs
   from both the retained batch and queued inputs, and derives the next tick from
   replayed state. Remaining inputs are retried at that tick on a later deadline.
   Neither group may infer readiness just from successful sends or quorum admission.

The wire Decision/catch-up interface assumes trusted non-Byzantine peers. Students
must validate membership, match identity, ballots and decision provenance; exposing
an arbitrary browser message as a ChosenEntry would bypass consensus entirely.

## Determinism and failure assumptions

The existing local lobby chooses a seed **once per round** and prints it with the
ordered player IDs. GameAdapter owns a seeded StdRng; all food generation, including
initial apples, uses that stream. Hash sets are used only for membership checks,
not iteration order. Game movement depends only on the chosen batches, config,
ordered roster and random stream, never on local clocks or packet delivery order.

Before running replicas, agree/replicate immutable match metadata (board config,
ordered player IDs, RNG seed, match/log identity). Join/readiness and the match
lifecycle are still local; this scaffold does NOT make lobby creation replicated.
Pin identical builds/Cargo.lock on replicas: StdRng's sequence is not guaranteed
across versions. Production portable snapshots need a specified/versioned RNG,
its state, applied log boundary, tick number and request deduplication state.

This is a **crash-stop, in-memory** scaffold. Restart loses promises, accepted
values, chosen history and receipts, and starts a new unrelated match. Never
rejoin the old distributed log with lost state or reuse its node identity without
recovery. Crash/restart support requires atomic durable promise/accept updates
before acknowledgements, persistent ballot counters, and authoritative recovery.
Client IDs/sequences are scoped to a live connection/match; reconnect/resume and
cross-match request fencing are future work. Peer heartbeats are separate from
browser connection heartbeats and must not directly kill a replicated snake.

## Run/check and first assignments

```sh
cargo run --bin server -- --width 60 --height 40 --apples 20
npm --prefix web run dev
# Optional, in a third terminal:
npm --prefix web run tunnel

cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
npm --prefix web run build
```

**Group 1 first:** implement fixed-membership validation, leader timers and higher
ballots; Prepare/Promise handling over the shared state; distinct complete-history
quorums; highest-ballot recovery plans and the applied-through readiness barrier.

**Group 2 first:** implement Accept/Accepted handlers over the same shared state;
match acknowledgements to the exact proposal and count distinct voters; emit
Chosen/Decision only at a majority; implement recovered proposals, one-slot
backpressure, decision catch-up, and the handoff into the existing session.

Relevant references: [Paxos Made Simple](https://lamport.azurewebsites.net/pubs/paxos-simple.pdf)
and [Tokio missed-deadline behavior](https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html).
