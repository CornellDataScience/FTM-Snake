# Server guide

## Running a mode

```sh
# Play the original standalone game (also the default with no mode flag).
cargo run --bin server -- --NONE --width 60 --height 40 --apples 20

# Reserved modes: each prints an explanation and exits with a failure status.
cargo run --bin server -- --PB
cargo run --bin server -- --PAXOS

cargo run --bin server -- --help
```

Pass at most one mode flag. `--NONE` has no consensus log, replication or voting.
`--PB` means primary/backup; that protocol has not been implemented.
`--PAXOS` has separate teaching interfaces, but no distributed runtime yet.
Neither placeholder falls back to a playable standalone server.

The listener defaults to `0.0.0.0:8080`. To run an isolated second server:

```sh
SNAKE_BIND_ADDRESS=127.0.0.1:18080 cargo run --bin server -- --NONE
```

Vite's normal `/ws` proxy still targets port 8080. Restart your Rust process
after changing code; an already-running binary keeps its old behavior.

## Reading the code

1. `src/bin/server.rs` reads arguments and the bind address, prints help or errors,
   and calls the library entry point.
2. `src/server/options.rs` parses `ServerOptions`. `GameConfig` in `src/config.rs`
   contains only game settings, validation, and board capacity.
3. `src/server/mod.rs` selects the runtime. Only NONE currently binds a listener.
4. `src/server/connection.rs` owns each WebSocket, waits for an accepted join,
   maps browser messages to lobby events, and enforces heartbeat timeouts.
5. `src/lobby/mod.rs` owns the player list and readiness. A single event loop
   handles input and the 100 ms clock, so shared game state needs no mutex.
6. `src/round.rs` owns the active Game, ordered player IDs and random stream, and
   maps player IDs to snakes. `src/lobby/snapshot.rs` builds the browser's view.
7. `src/game.rs` and `src/snake.rs` implement the rules shared with the desktop game.

## Standalone message flow

A connection sends a Join event and waits for the lobby's reply. On acceptance,
the client receives `welcome`, then lobby updates. At least two players must join,
and everyone must be ready before a round starts. The initial board is sent
immediately; the first move follows 100 ms later.

Direction events update the snake's pending turn directly. The first legal turn
per player per tick wins. Every 100 ms, the game advances one cell and the lobby
broadcasts a full `state`, or `game_over` if the round ended. Missed clock deadlines
are skipped to avoid a burst of catch-up moves. Optional direction sequences are
accepted but ignored in NONE; it does not emit `input_applied` receipts.

Browser heartbeats arrive every 25 ms. A joined connection with no heartbeat for
1 second is disconnected; other traffic does not reset that deadline. Heartbeats
never advance the game. During a round, a disconnected player's snake keeps its
last direction. After the round, disconnected players are removed and readiness
is cleared for everyone else.

Connections use bounded event and outgoing-message queues. Outgoing messages use
`try_send`, so a slow client may lose updates instead of blocking all players.
The browser protocol includes reserved message types for future features;
`src/protocol.rs` and `web/src/protocol.ts` must remain compatible.

## Extending it

- Add a game setting to `GameConfig`, validate it there, expose it in
  `ServerOptions::from_args`, and consume it in `Game::with_config`.
- Add a browser message to both protocol definitions, route it in the connection
  handler, then handle its event in the lobby.
- Implement each replicated mode in its own runtime module and connect it through
  the match in `server::run`. Keep peer membership, transport and replication
  state in that runtime. Reuse the game rules and browser protocol; define how
  roster, readiness and match creation are replicated before starting replicas.
- For Paxos, build on `src/consensus/` and [its interface guide](consensus.md).
  `LocalProposer` is only a local test adapter. Wiring it to `--PAXOS` would not
  implement Paxos. Primary/backup will need its own protocol and runtime.

`cargo test --all-targets` covers mode parsing, unavailable modes, game rules,
lobby lifecycle, standalone timing and the separate consensus contracts.
