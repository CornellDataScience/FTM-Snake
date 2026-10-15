//! Game rules shared by the desktop client and future server.
pub mod apple;
pub mod game;
pub mod player;
pub mod snake;

pub type Point = (i16, i16);

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
