# FTM Snake

A multiplayer Snake prototype with a Rust game server, a TypeScript browser
client, and a two-player desktop version built with Macroquad.

## Run and check

The package has two binaries, so `cargo run` needs `--bin`:

```sh
cargo run --bin ftm_snake     # the two-player desktop game
cargo run --bin server        # WebSocket game server for the web client
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```

In the desktop version, player 1 (green, top left) steers with WASD; player 2 (blue, bottom right) steers
with the arrow keys. The board is 25 × 25 cells; each snake starts with three
cells and moves every 0.10 seconds. `APPLE_COUNT` apples (in `lib.rs`) stay on the
board, and each grows a snake by one cell. Hitting a wall, either snake's body, or
a dead snake kills a snake; the last snake alive wins, whatever its length, and
snakes dying on the same tick draw. In a head-on collision both heads move into the
shared cell (or stay put if they tried to pass through each other) and implode
before the draw is shown. Press Space to restart after a match. Only one valid
turn is accepted per tick. The browser client also supports LAN and tunneled play.

## Configure a multiplayer server

Pass the server mode and game options after Cargo's `--` separator:

```sh
cargo run --bin server -- --NONE --width 60 --height 40 --apples 20
cargo run --bin server -- --help
```

- `--NONE`: original standalone server, **10 moves per second**; the default if omitted.
- `--PB`: primary/backup placeholder. Prints an explanation and exits without listening.
- `--PAXOS`: Multi-Paxos placeholder. Prints an explanation and exits without listening.
- Choose only one mode. PB and Paxos algorithms are not implemented yet.
- `--width`: board columns, default **25**.
- `--height`: board rows, default **25**.
- `--apples`: target apples on the board, default **3**; **0** disables apples.

Settings apply to every round until the server restarts. Invalid options fail
with a readable error before the server listens. Dimensions must each be at
least 5 and fit in a signed 16-bit coordinate (maximum 32767). The board must
have room for at least two starting areas. Apple count cannot exceed total
board cells; when snakes occupy the available space, fewer apples can spawn.

There is no fixed two-player or four-player cap. Each player gets a **5 x 5
starting area**, so capacity is `floor(width / 5) * floor(height / 5)`:
25 x 25 supports 25 players; 60 x 40 supports 96; 100 x 100 supports 400.
The server prints capacity on startup. These are space limits, not performance
guarantees: CPU, browser rendering, and network bandwidth still limit practical
player counts. A finite board cannot hold literally infinite snakes.

Have everyone **join before clicking Ready**. A round starts as soon as at
least two players are present and **every joined player is ready**. Players
cannot join midway through a round. Eliminated players can watch the remaining
snakes; the last one alive wins, and simultaneous final deaths draw. After a
round, connected players return to the lobby with readiness cleared.

The frontend reads dimensions and all snakes from the server's snapshots, so
there is no frontend board-size setting. Each snake has a stable color, your
snake has a white outline, and the status bar shows how many players remain.

## Desktop game

`cargo run --bin ftm_snake` shows everything the game does so far, in one window:

- **Local multiplayer:** two snakes on one keyboard, P1 on WASD and P2 on the
  arrow keys, each score (snake length) shown at the top.
- **Apple generation:** three apples on the board while free cells remain, each
  respawned on a random cell that no snake or apple occupies.
- **Simultaneous moves:** both snakes plan their move before either advances,
  so collisions are judged against where the other snake is going; a snake may
  follow into a tail that is moving away.
- **Match results:** walls and bodies kill, dead snakes stay as obstacles, the
  last snake alive wins, and head-on crashes implode into a draw.
- **Fixed-rate ticks:** the snakes move exactly ten times a second whatever the
  frame rate, and a stalled window (over 0.25 s) does not replay a burst of moves.
- **Restart:** Space starts a new match at any time.


## Code layout

- `lib.rs`: shared cell coordinates, board dimensions, player and apple counts.
- `config.rs`: game rules configuration, defaults, validation, and starting-area capacity.
- `game.rs`: game state, simultaneous collision rules, match result, and apple refills.
- `snake.rs`: segment storage, turns, and growth.
- `player.rs`: direction values and their movement offsets.
- `input.rs`: desktop keyboard bindings.
- `render.rs`: board and entity drawing, sized to the window.
- `main.rs`: desktop setup and the fixed-rate tick clock.
- `round.rs`: one match's game, random stream, and player-to-snake mapping; shared
  by the lobby and the consensus adapter.
- `bin/server.rs`: small command-line entry point.
- `server/options.rs`: mode flags and board/apple argument parsing.
- `server/mod.rs`: mode selection and listener startup; add future runtimes here.
- `server/connection.rs`: WebSocket connections and heartbeat timeouts.
- `lobby/mod.rs`: standalone player readiness, game clock, and broadcasts.
- `lobby/snapshot.rs`: builds the browser's view of the running round.
- `consensus/`: separate Multi-Paxos teaching interfaces and local test adapter.

The game rules have no keyboard, window, or clock calls. In `--NONE`, directions
call `Game::set_direction` directly and the round calls `Game::tick` every 100 ms.
The consensus test adapter is not part of this runtime.
See [the server guide](docs/server.md) for the flow and extension points.

Snake segments include the head at the front of a queue. An occupancy set mirrors
that queue for collision lookup. Each tick, `Game` plans every snake's next head
before moving any of them, so collisions are judged against where the other snake
is going. A move may enter a tail cell that is being vacated (by either snake), but
not if that snake is growing or dying this tick. Fatal moves preserve the last
valid positions.

To extend the rules, add settings to `GameConfig` and `ServerOptions::from_args`, then
pass them through `Game::with_config`. Snakes and collision plans are vectors,
so the game loop handles every joined player. `starting_snakes` is the single
place to change spawn placement; `SPAWN_SPACING` controls reserved starting
space. `LOCAL_PLAYER_COUNT` only controls the two-player desktop game.

## Web frontend

`web/` is a TypeScript (Vite) browser client: a join page and a Google Snake–style
board. It holds no game logic — it sends the player's name and key presses to the
server and draws whatever state the server sends back.

```sh
cd web
npm install
npm run dev        # http://localhost:5180
npm run build      # type-check and bundle into web/dist
```

The join page has a **Lobby IP** field below your name. Enter the server computer's
IP or hostname (for example, `192.168.1.10`), optionally with a port
(`192.168.1.10:8080`), or a full `ws://` / `wss://` URL. Bare hosts use port 8080.
The field defaults to `/ws` on the website's own host and port, using `wss://`
for HTTPS pages. Vite proxies that connection to Rust on `127.0.0.1:8080`.
Leave the field unchanged when using the normal local, LAN, or tunnel setup.
Override it only for a separate backend, for example with
`VITE_SERVER_URL=ws://host:8080 npm run dev`.
To serve the frontend to other computers on your LAN, run
`npm run dev -- --host 0.0.0.0` and open `http://<server-ip>:5180` on those computers. Run the WebSocket
server on the same computer; LAN players only need access to the frontend port.

## Share your local game through Cloudflare

Both the frontend and Rust backend keep running on your computer. One temporary
public HTTPS link carries the website and the game's WebSocket messages:

`Player's browser → Cloudflare Tunnel → Vite :5180 → /ws proxy → Rust :8080`

### One-time setup

Install frontend dependencies with `npm install` inside `web/`. Install
[`cloudflared`](https://developers.cloudflare.com/tunnel/downloads/) if needed
(`brew install cloudflared` on macOS with Homebrew). Quick Tunnels do not require
a Cloudflare account, domain, or router port forwarding.

### Start a session

Open three terminals, each starting in the repository root:

1. Start the Rust backend:
   ```sh
   cargo run --bin server
   ```
2. Start the frontend:
   ```sh
   cd web
   npm run dev
   ```
3. Start the tunnel:
   ```sh
   cd web
   npm run tunnel
   ```

The third terminal prints an `https://...trycloudflare.com` address. Share that
HTTPS link with the other players and open it yourself. Each player enters a
different name, leaves **Lobby IP** at its automatically filled value, clicks
**Play**, waits for everyone to join, and then clicks **Ready**. The game starts
when at least two players have joined and everyone is ready.
Players need only their browser; they do not install or run anything locally.
You can also run the frontend commands from the repository root as
`npm --prefix web run dev` and `npm --prefix web run tunnel`. Here `--prefix web`
tells npm to use the package in the `web/` directory without changing your
terminal's current directory.
To test access from outside your LAN, open the link on a phone using cellular
data instead of Wi-Fi.

Keep all three processes running and your computer awake. Stop each with
**Ctrl+C** when finished. Restarting the tunnel generates a new link; share the
new link and reload it. This is a development setup: anyone with the link can
reach the game and frontend development assets. The board size determines how
many players can join.

The tunnel script sets the origin Host header to `localhost:5180`, which Vite
already accepts. It does not disable Vite's host checks or allow every
`trycloudflare.com` subdomain. Vite uses a fixed port and exits if 5180 is taken.

### Troubleshooting

- **Cloudflare 502:** check that `npm run dev` is running on port 5180.
- **Website loads, but joining disconnects:** check the Rust terminal and port
  8080. The automatically filled lobby address should end in `/ws` and use
  `wss://` on the public HTTPS page, without `:8080`. Remove an old
  `VITE_SERVER_URL` override and restart Vite if the field points elsewhere.
- **Port 5180 is already in use:** stop the existing frontend before restarting;
  the tunnel always forwards to 5180.
- **Tunnel cannot connect on your network:** try `npm run tunnel -- --protocol http2`
  to use HTTP/2 instead of QUIC.
- **Quick Tunnel configuration conflict:** if you already use a Cloudflare
  `config.yaml` in `~/.cloudflared`, follow the Quick Tunnel documentation before
  changing that existing configuration.

See [Cloudflare Quick Tunnels](https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/)
and [Vite proxy configuration](https://vite.dev/config/server-options#server-proxy).

## Network messages

Messages are JSON, defined in
`web/src/protocol.ts`:

- Client → server: `join`, `set_ready`, `direction`, and `heartbeat`.
- Server → client in `--NONE`: `welcome`, `lobby`, `state`, `game_over`, and `error`.

For example, a turn is `{"type":"direction","direction":"up"}` and a welcome
is `{"type":"welcome","player_id":"1"}`. Coordinates are `[x, y]` pairs.
The standalone server runs **10 simulation ticks per second**, moving snakes
once and broadcasting a full snapshot afterward. The optional direction
`sequence` and `input_applied` receipt are reserved for future replicated modes;
`--NONE` ignores sequences and does not send consensus receipts. Browsers send
`{"type":"heartbeat"}` every 25 ms (about 40 per second). Rust marks a joined
player disconnected after 1 second without a heartbeat, using a deadline that
resets on each heartbeat. Direction messages do not reset it. Browser timers
may slow down in background tabs or when the device sleeps, so keep the game
visible while playing.

## Multi-Paxos teaching scaffold

The scaffold uses the CS5414 notes’ `P1a/P1b/P2a/P2b`, p-value, command, decision,
and replica terminology. See [the interface guide](docs/consensus.md) for shared types, group ownership,
normal tick flow, recovery handoff, deterministic replay assumptions and tests.
**LocalProposer** remains a memory-only test adapter with **no distributed fault
tolerance**. It is used by the consensus tests, not the playable server.
The distributed protocol methods explicitly report unsupported/not-ready;
students implement elections and quorum logic.

`--PAXOS` reserves the mode name but exits until a distributed runtime is added.
The existing scaffold provides an ordered log, seeded game adapter, input
batching, and decided-and-executed receipts. One unresolved fresh tick blocks the
next proposal while inputs keep queuing. Recovery no-ops consume log slots
without moving the game. `--PB` is also a reserved mode; no primary/backup
implementation exists yet.

For an isolated local test, set `SNAKE_BIND_ADDRESS=127.0.0.1:18080` before the
server command. The default remains `0.0.0.0:8080`; the normal Vite proxy expects
that default port.
