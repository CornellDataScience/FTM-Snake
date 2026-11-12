use apple_logic::{LOCAL_PLAYER_COUNT, player::Direction};
use macroquad::prelude::{KeyCode, is_key_pressed};

/// Player 1 steers with WASD, player 2 with the arrow keys.
const PLAYER_KEYS: [[(KeyCode, Direction); 4]; LOCAL_PLAYER_COUNT] = [
    [
        (KeyCode::W, Direction::Up),
        (KeyCode::S, Direction::Down),
        (KeyCode::A, Direction::Left),
        (KeyCode::D, Direction::Right),
    ],
    [
        (KeyCode::Up, Direction::Up),
        (KeyCode::Down, Direction::Down),
        (KeyCode::Left, Direction::Left),
        (KeyCode::Right, Direction::Right),
    ],
];

pub fn read_directions() -> [Option<Direction>; LOCAL_PLAYER_COUNT] {
    PLAYER_KEYS.map(|keys| {
        keys.into_iter()
            .find_map(|(key, direction)| is_key_pressed(key).then_some(direction))
    })
}

pub fn restart_pressed() -> bool {
    is_key_pressed(KeyCode::Space)
}
