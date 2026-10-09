use macroquad::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn is_opposite(self, other: Direction) -> bool {
        match (self, other) {
            (Direction::Up, Direction::Down) => true,
            (Direction::Down, Direction::Up) => true,
            (Direction::Left, Direction::Right) => true,
            (Direction::Right, Direction::Left) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct Player {
    pub current_direction: Direction,
}

impl Player {
    pub fn new() -> Self {
        Self {
            current_direction: Direction::Right,
        }
    }

    pub fn set_direction(&mut self, direction: Direction) {
        if !self.current_direction.is_opposite(direction) {
            self.current_direction = direction;
        }
    }

    pub fn current_direction(&self) -> Direction {
        self.current_direction
    }

    pub fn handle_input(&mut self) {
        if is_key_pressed(KeyCode::Up) {
            self.set_direction(Direction::Up);
        } else if is_key_pressed(KeyCode::Down) {
            self.set_direction(Direction::Down);
        } else if is_key_pressed(KeyCode::Left) {
            self.set_direction(Direction::Left);
        } else if is_key_pressed(KeyCode::Right) {
            self.set_direction(Direction::Right);
        }
    }
}