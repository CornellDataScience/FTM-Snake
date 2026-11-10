# FTM Snake

A local two-player Snake prototype built with Rust and Macroquad.

## Run and check

The package has three binaries, so `cargo run` needs `--bin`:

```sh
cargo run --bin apple_logic   # the desktop game
cargo run --bin demo          # the same game on a fixed-rate tick clock
cargo run --bin server        # WebSocket echo server for the web client
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```

Player 1 (green, top left) steers with WASD; player 2 (blue, bottom right) steers
with the arrow keys. The board is 25 × 25 cells; each snake starts with three
cells and moves every 0.10 seconds. `APPLE_COUNT` apples (in `lib.rs`) stay on the
board, and each grows a snake by one cell. Hitting a wall, either snake's body, or
a dead snake kills a snake; the last snake alive wins, whatever its length, and
snakes dying on the same tick draw. In a head-on collision both heads move into the
shared cell (or stay put if they tried to pass through each other) and implode
before the draw is shown. Press Space to restart after a match. Only one valid
turn is accepted per tick. LAN play remains future work.

## Demo

`cargo run --bin demo` shows everything the game does so far, in one window:

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

The demo changes no game rules; it reuses `input.rs` and `render.rs` and only
replaces the timing loop in `main.rs`.

## Code layout

- `lib.rs`: shared cell coordinates, board dimensions, player and apple counts.
- `game.rs`: game state, simultaneous collision rules, match result, and apple refills.
- `snake.rs`: segment storage, turns, and growth.
- `player.rs`: direction values and their movement offsets.
- `apple.rs`: random selection of unoccupied cells.
- `input.rs`: desktop keyboard bindings.
- `render.rs`: board and entity drawing, sized to the window.
- `main.rs`: application setup and timing.
- `bin/demo.rs`: the same game on a fixed-rate tick clock.
- `bin/server.rs`: WebSocket server, currently an echo.

The library has no keyboard, window, or clock calls. A future server can drive
`Game::set_direction(player, direction)` and `Game::tick` directly and supply its
own random generator.

Snake segments include the head at the front of a queue. An occupancy set mirrors
that queue for collision lookup. Each tick, `Game` plans every snake's next head
before moving any of them, so collisions are judged against where the other snake
is going. A move may enter a tail cell that is being vacated (by either snake), but
not if that snake is growing or dying this tick. Fatal moves preserve the last
valid positions.

## Web frontend

`web/` is a TypeScript (Vite) browser client: a join page and a Google Snake–style
board. It holds no game logic — it sends the player's name and key presses to the
server and draws whatever state the server sends back.

```sh
cd web
npm install
npm run dev        # http://localhost:5173
npm run build      # type-check and bundle into web/dist
```

The join page has a **Lobby IP** field below your name. Enter the server computer's
IP or hostname (for example, `192.168.1.10`), optionally with a port
(`192.168.1.10:8080`), or a full `ws://` / `wss://` URL. Bare hosts use port 8080.
The field defaults to the page's hostname on port 8080, using `wss://` for HTTPS
pages; override that initial value with `VITE_SERVER_URL=ws://host:port npm run dev`.
To serve the frontend to other computers on your LAN, run
`npm run dev -- --host 0.0.0.0` and open `http://<server-ip>:5173` on those computers. Run the WebSocket
server on the selected host and allow its port through the host's firewall.

Messages are JSON, defined in
`web/src/protocol.ts`:

- Client → server: `{"type":"join","name":"…"}`, `{"type":"direction","direction":"Up"}`
- Server → client: `{"type":"welcome","playerId":"…"}`, `{"type":"state","state":{…}}`, `{"type":"error","message":"…"}`

`Point` (`[x, y]`) and `Direction` (`"Up"`, …) match serde's default encoding of
the Rust types.
