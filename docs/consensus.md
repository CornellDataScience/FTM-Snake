# Multi-Paxos interface scaffold

The interface follows the terminology in **CS5414 Section 4: Lab 3 Intro 2 -
Multi Paxos** and **Week 6: Consensus and Reliable Broadcast**. It remains a
teaching scaffold: elections, quorum collection, peer transport, catch-up and
log garbage collection are not implemented. `--PAXOS` still exits with an
explanation. The playable `--NONE` server uses its own direct 10 Hz game loop.

## Read it alongside the notes

Section 4 pages 11-17 describe the two phases; pages 29-35 cover leadership and
shared server state; pages 37-47 cover garbage collection, holes and execution.
Week 6 pages 74-75 introduce client commands; pages 81-86 describe the roles and
replica state.

- **Ballot**: a proposal number `(counter, node_id)`. Compare the counter first,
  then node ID. Multi-Paxos shares a ballot across slots.
- **Slot**: a position in the replicated log. This project keeps zero-based slots;
  the lecture pseudocode starts at one. Slot numbers are separate from game ticks.
- **Command**: the operation decided for a slot. Here `Command::Tick(TickBatch)`
  contains ordered player inputs plus one game step. `Command::Noop` fills a hole
  without moving snakes or consuming randomness.
- **PValue**: the notes' p-value `<ballot, slot, command>`. A proposed or accepted
  p-value is not necessarily decided.
- **P1a / P1b**: prepare request and successful prepare reply. P1b returns the
  acceptor's accepted p-values, including their original ballots.
- **P2a / P2b**: propose request and successful accept reply. P2a carries a p-value.
  Our P2b echoes the full p-value, making ballot/slot/command matching explicit.
- **Decision**: a decided `<slot, command>` that a replica can execute in order.
- **Replica**: holds application state, `slot_num`, and `decisions` as in Week 6.
  `slot_num` is the next slot to execute, also called `slot_out` in Section 4.
- **Scout**: the record of a leader's phase 1 attempt and distinct P1b replies.
- **Commander**: the record of one phase 2 proposal and distinct P2b replies.

A browser input corresponds to Week 6's `<client, cid, op>`:
`PlayerInput.request_id.client_id`, `request_id.sequence`, and `direction`.
We keep those names to match the browser protocol. Several client operations
can be batched into one Paxos command; a no-op is not a client operation.

The game-specific `TickSession` retains one pending batch while the proposer
assigns its slot. It does not implement the lecture replica's general-purpose
set of concurrent proposals. This preserves the project's one-unresolved-tick
policy; deciding a conflicting command during recovery keeps unexecuted player
requests available for a later proposal.

## Files and group ownership

`paxos.rs` defines **one `PaxosServer` protocol state owner**, following the Lab 3
organization discussed on Section 4 page 35. Both groups implement methods on
this same struct. Its acceptor state is shared; no group creates its own copy.
Scout and Commander are data records, not separate threads or network actors.
The game execution helper remains separate so it can be tested without a network.
A future host owns a `TickSession<PaxosServer>` and handles events serially.

- `types.rs`: ballots, slots, commands, p-values, decisions, peer messages and effects.
- `acceptor.rs`: the implemented in-memory ballot/accepted-p-value checks.
- `paxos.rs`: shared protocol state and message/timer dispatch.
- **Group 1 - `phase1.rs`**: `on_p1a`, `on_p1b`, `on_preempted`,
  `on_phase1_timer`, and `on_recovery_executed`. Own election, complete P1b
  histories, highest-ballot recovery proposals and the recovery barrier.
- **Group 2 - `phase2.rs`**: `on_p2a`, `on_p2b`, `on_phase2_timer`,
  `begin_recovery`, `activate`, and the `Proposer` implementation. Own matching
  P2b majorities, decisions and re-proposals under the new ballot.
- `proposer.rs`: the game-facing `Proposer` trait and `LocalProposer` test double.
- `replica.rs`: ordered, deterministic decision execution and duplicate requests.
- `batcher.rs` / `session.rs`: input batching, one outstanding tick, and recovery
  reconciliation between the game and proposer.
- `tests.rs`: protocol contracts and tests with a delayed proposer.

The old names were `Prepare/Promise/Accept/Accepted` (now `P1a/P1b/P2a/P2b`),
`AcceptedEntry` (now `PValue`), `ChosenEntry` (now `Decision`), and `GameAdapter`
(now `Replica`). `Agreement::submit_fresh` is now `Proposer::propose`;
`submit_recovered` is `repropose`. `LocalAgreement` is now `LocalProposer`.
The former `Leadership` and `PaxosAgreement` owners are combined in `PaxosServer`.
These are source and peer-message format changes; no deployed peer protocol
exists to migrate, and browser messages are unchanged.

## Prepare and propose contracts

**P1a** adopts a higher ballot across all slots. A retry at the same ballot
returns the accepted history again; a lower ballot is rejected. A successful
P1b is sent only after updating the acceptor state.

**P2a** can be accepted only if its ballot **equals the adopted ballot**, matching
Section 4 page 34. This changes the former helper's `ballot >= promised` rule.
An acceptor with no ballot, or with a lower or higher ballot than the P2a, rejects
it without changing its state. A higher P2a needs a successful P1a first. A
conflicting command at the same ballot and slot is also rejected. Repeating the
same p-value is safe. Send P2b only after successful acceptance.

These local checks already work. The server's network handlers intentionally
return `Unsupported`; they do not yet call the helpers, send replies or count
votes. Group 1 and Group 2 must implement that wiring on `self.acceptor`.

A commander may announce a decision only after a **majority of distinct configured
acceptors** return P2b for its exact ballot, slot and command. A duplicate response
counts once. Outsiders and stale/mismatched replies never contribute. Future
handlers must validate membership and the match/log identity before processing
any message, including Decision and catch-up replies.

## Propose, decide, execute

1. The future runtime authenticates a browser's joined player ID and queues its
   `PlayerInput`. If a legacy client omits sequence, allocate one or reject the
   input explicitly. The NONE handler currently ignores sequences.
2. `TickSession::on_deadline` closes one `TickBatch` and calls `propose(&batch)`.
   The caller retains the batch for every outcome. `Proposed(slot)` means admitted
   by the proposer, not decided or executed. `Busy`, `NotLeader`, `Recovering` and
   `Unsupported` retain all pending inputs.
3. Phase 2 sends P2a and awaits matching P2b replies. Until the proposal is decided
   **and executed**, another deadline cannot create a second fresh proposal.
   Incoming player requests can still queue.
4. On a majority, announce `PaxosMessage::Decision` to replicas and emit
   `Effect::Decision` locally. The host delivers validated decisions through
   `TickSession::on_decision` to `Replica::on_decision`.
5. The replica stores decisions past a hole but executes only a contiguous prefix
   starting at `slot_num`. A duplicate identical decision does not execute again;
   conflicting decisions and wrong game-tick numbers return an error.
6. Execute each batch's inputs in order, then move the game once. The first legal
   turn per player per tick wins. Repeated client request IDs never execute twice.
   A no-op advances `slot_num` but not `GameTick`.
7. Only after execution may the host send `input_applied` and game snapshots.
   `ExecutedSlot.results` supplies the receipts, including original tick numbers
   for duplicate requests. A receipt confirms processing, not that a turn was legal.

For example, slot 0 can be a no-op, slot 1 game tick 0, and slot 2 game tick 1.
`slot_num` never decreases, and the game reflects decisions only below that cursor.
This is the ordered execution behavior shown in Week 6's replica pseudocode.

`LocalProposer` exercises this boundary without a network: it emits a decision
immediately but still waits for execution before accepting another proposal.
It is a test double, not Paxos or fault tolerance, and is not used by `--NONE`.

## Recovery handoff

1. On suspected leader failure, pause fresh proposals with
   `session.begin_recovery()`. Group 1 chooses a higher unique ballot and enters
   `Scouting`. A timeout is suspicion, not proof of failure. Preemption revokes
   readiness; use heartbeats to avoid continuously competing with a live leader.
2. Collect complete P1b histories from a majority of distinct fixed members.
   `AcceptedHistory::Page` is incomplete until every page from slot zero is
   present. A final page alone does not prove that unlisted slots are empty.
3. For each slot, preserve the command from the highest accepted ballot. Fill
   proven holes through the recovery boundary with no-ops. Emit
   `Effect::Phase1Complete(RecoveryPlan { ballot, proposals, through })`.
4. Group 2's `begin_recovery` installs that plan and enters `Recovering`.
   `repropose(slot, &command)` preserves the original slot/command while using the
   new ballot. No fresh game ticks may be invented during this process.
5. Route recovered decisions through the normal replica execution path. Report
   the executed prefix to `on_recovery_executed`. A P2b alone, or a decision past
   a hole, is insufficient to complete recovery.
6. Once every slot through the boundary is decided and executed, Group 1 emits
   `LeaderReady`. The host calls `session.reconcile_after_recovery(plan.through)`
   before `session.proposer.activate(ballot)` enters `Active`. Reconciliation
   clears obsolete proposal tracking, removes already-executed request IDs and
   derives the pending batch's tick from replayed state. Unexecuted requests stay.

The `Recovering`/`Active` barrier and one outstanding fresh tick are this game's
policy, stricter than a general pipelined Multi-Paxos implementation. The notes'
role names do not imply that we have implemented their complete algorithms.

## Heartbeats, catch-up and garbage collection

`Heartbeat` and `HeartbeatReply` carry a ballot and `executed_through`; `CatchUp`
and `CatchUpReply` reserve the decision transfer interface. These handlers are
placeholders. Section 4 page 37 describes discarding slots only after **everyone**
has executed them, not merely a majority. No log entries are discarded here.
Before implementing GC, define how complete histories, catch-up, replica state
and request deduplication interact with that boundary.

These peer heartbeats are independent of the browser's 25 ms heartbeat and
1 second disconnect timeout. Suspecting a peer must not directly kill a snake.
`TickSession` owns no clock. A future host can call `on_deadline` periodically
(the earlier design targets 30 Hz), skip missed opportunities, and remain
responsive while waiting for consensus. The current NONE game stays at 10 Hz.

## Determinism and failure assumptions

`Replica` wraps the shared `Round` from `src/round.rs`, preserving the existing
seeded RNG, ordered roster and game rules. Every replica must start with the same
board config, roster, seed and match/log identity. Match setup and readiness
are not replicated by this scaffold. Pin identical builds and Cargo.lock;
StdRng is not a portable, version-independent wire RNG.

This is a crash-stop, in-memory scaffold. Restart loses the adopted ballot,
accepted p-values, decisions and receipts. Do not rejoin an old log with lost
state under the same identity. Crash/restart support needs durable acceptor
updates before P1b/P2b, persistent ballot counters and an authoritative recovery
path. Logs, input queues and deduplication records remain unbounded.

## Run and check

```sh
cargo run --bin server -- --NONE --width 60 --height 40 --apples 20
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```

To expose a completed protocol, connect its runtime through `ServerMode::Paxos`
in `src/server/mod.rs`. See [the server guide](server.md).
