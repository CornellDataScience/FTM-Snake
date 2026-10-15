# FTM Snake

A local single-player Snake prototype built with Rust and Macroquad.

## Run and check

```sh
cargo run
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```

Use the arrow keys to turn. The board is 25 × 25 cells; the snake starts with
three cells and moves every 0.15 seconds. Apples grow the snake by one cell.
Wall and self collisions stop movement. Only one valid turn is accepted per tick.
Scores, match results, restart, local multiplayer, and LAN play remain future work.

## Code layout

- `lib.rs`: shared cell coordinates and board dimensions.
- `game.rs`: game state, tick updates, and apple replacement.
- `snake.rs`: segment storage, turns, growth, and collision rules.
- `player.rs`: direction values and their movement offsets.
- `apple.rs`: random selection of unoccupied cells.
- `input.rs`: desktop keyboard bindings.
- `render.rs`: board and entity drawing, sized to the window.
- `main.rs`: application setup and timing.

The library has no keyboard, window, or clock calls. A future server can drive
`Game::set_direction` and `Game::tick` directly and supply its own random generator.
Adding multiplayer will also require player identities and simultaneous collision
resolution; the current tick updates one snake.

Snake segments include the head at the front of a queue. An occupancy set mirrors
that queue for collision lookup. A non-growing move may enter the cell vacated by
the tail. Fatal moves preserve the last valid positions.
