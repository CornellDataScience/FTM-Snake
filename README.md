# FTM Snake

A local two-player Snake prototype built with Rust and Macroquad.

## Run and check

```sh
cargo run
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

## Code layout

- `lib.rs`: shared cell coordinates, board dimensions, player and apple counts.
- `game.rs`: game state, simultaneous collision rules, match result, and apple refills.
- `snake.rs`: segment storage, turns, and growth.
- `player.rs`: direction values and their movement offsets.
- `apple.rs`: random selection of unoccupied cells.
- `input.rs`: desktop keyboard bindings.
- `render.rs`: board and entity drawing, sized to the window.
- `main.rs`: application setup and timing.

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

The client connects to `ws://localhost:8080` by default; override it with
`VITE_SERVER_URL=ws://host:port npm run dev`. Messages are JSON, defined in
`web/src/protocol.ts`:

- Client → server: `{"type":"join","name":"…"}`, `{"type":"direction","direction":"Up"}`
- Server → client: `{"type":"welcome","playerId":"…"}`, `{"type":"state","state":{…}}`, `{"type":"error","message":"…"}`

`Point` (`[x, y]`) and `Direction` (`"Up"`, …) match serde's default encoding of
the Rust types.
