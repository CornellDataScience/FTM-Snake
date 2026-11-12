//! Game rules shared by the desktop client and the game server.
pub mod apple;
pub mod config;
pub mod consensus;
pub mod game;
pub mod lobby;
pub mod player;
pub mod protocol;
pub mod snake;

pub type Point = (i16, i16);

/// Keyboard-controlled players in the desktop demo, not a network player limit.
pub const LOCAL_PLAYER_COUNT: usize = 2;
/// Default apples kept on the board while free cells remain.
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
