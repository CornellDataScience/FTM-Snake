//! Game rules shared by the desktop client and the game server.
pub mod apple;
pub mod game;
pub mod player;
pub mod protocol;
pub mod snake;

pub type Point = (i16, i16);

/// Number of snakes in a match.
pub const PLAYER_COUNT: usize = 2;
/// Apples kept on the board while free cells remain.
pub const APPLE_COUNT: usize = 3;

#[derive(Debug, Clone, Copy)]
pub struct Board {
    pub width: i16,
    pub height: i16,
}

impl Board {
    pub fn contains(self, (x, y): Point) -> bool {
        (0..self.width).contains(&x) && (0..self.height).contains(&y)
    }
}

impl Default for Board {
    fn default() -> Self {
        Self {
            width: 25,
            height: 25,
        }
    }
}
