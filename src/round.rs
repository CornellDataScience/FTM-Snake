//! One match between a fixed, ordered roster of players. Shared by the
//! standalone lobby and the consensus game adapter, so both map player IDs to
//! snakes and draw randomness the same way.
use std::collections::HashSet;

use rand::{SeedableRng, rngs::StdRng};

use crate::{
    config::GameConfig,
    game::{Game, Status},
    player::Direction,
};

pub struct Round {
    game: Game,
    /// Fixed for the round, in the same order as the game's snakes.
    owners: Vec<String>,
    rng: StdRng,
}

impl Round {
    /// The same config, roster order and seed always produce the same round.
    pub fn new(config: GameConfig, owners: Vec<String>, seed: u64) -> Result<Self, String> {
        if owners.iter().collect::<HashSet<_>>().len() != owners.len() {
            return Err("Player IDs must be unique".into());
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let game = Game::with_config(config, owners.len(), &mut rng)?;
        Ok(Self { game, owners, rng })
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn owners(&self) -> &[String] {
        &self.owners
    }

    /// The snake index of `player_id`, if they are in this round.
    pub fn snake_of(&self, player_id: &str) -> Option<usize> {
        self.owners.iter().position(|id| id == player_id)
    }

    /// Ignores players who are not in this round.
    pub fn set_direction(&mut self, player_id: &str, direction: Direction) {
        if let Some(snake) = self.snake_of(player_id) {
            self.game.set_direction(snake, direction);
        }
    }

    pub fn tick(&mut self) -> Status {
        self.game.tick(&mut self.rng)
    }

    /// The winning player's ID, or `None` for a draw or a running game.
    pub fn winner(&self) -> Option<&str> {
        match self.game.status() {
            Status::Winner(snake) => Some(&self.owners[snake]),
            Status::Running | Status::Draw => None,
        }
    }
}
