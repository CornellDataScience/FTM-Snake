use crate::{APPLE_COUNT, Board};

/// Each starting snake gets a square with room for its body and first move.
pub(crate) const SPAWN_SPACING: i16 = 5;

/// Rules chosen once at server startup and reused for every round.
#[derive(Debug, Clone, Copy)]
pub struct GameConfig {
    pub board: Board,
    pub apple_count: usize,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            board: Board::default(),
            apple_count: APPLE_COUNT,
        }
    }
}

impl GameConfig {
    /// Capacity comes from available starting areas, not a fixed player limit.
    pub fn player_capacity(&self) -> usize {
        (self.board.width.max(0) / SPAWN_SPACING) as usize
            * (self.board.height.max(0) / SPAWN_SPACING) as usize
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.board.width < SPAWN_SPACING || self.board.height < SPAWN_SPACING {
            return Err(format!(
                "Board width and height must each be at least {SPAWN_SPACING}"
            ));
        }
        if self.player_capacity() < 2 {
            return Err(format!(
                "The board needs at least two {SPAWN_SPACING} x {SPAWN_SPACING} starting areas"
            ));
        }
        let cells = self.board.width as usize * self.board.height as usize;
        if self.apple_count > cells {
            return Err("Apple count cannot exceed the number of board cells".into());
        }
        Ok(())
    }
}
