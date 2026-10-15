use apple_logic::player::Direction;
use macroquad::prelude::{KeyCode, is_key_pressed};

pub fn read_direction() -> Option<Direction> {
    [
        (KeyCode::Up, Direction::Up),
        (KeyCode::Down, Direction::Down),
        (KeyCode::Left, Direction::Left),
        (KeyCode::Right, Direction::Right),
    ]
    .into_iter()
    .find_map(|(key, direction)| is_key_pressed(key).then_some(direction))
}
