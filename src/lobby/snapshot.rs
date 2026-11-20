//! Builds the browser's view of a round.
use crate::{
    protocol::{GameState, SnakeState},
    round::Round,
};

use super::Player;

pub(super) fn snapshot(round: &Round, players: &[Player]) -> GameState {
    let game = round.game();
    let board = game.board();
    let snakes = game.snakes().iter().zip(round.owners());
    GameState {
        width: board.width,
        height: board.height,
        snakes: snakes
            .map(|(snake, id)| {
                let player = players.iter().find(|p| p.id == *id);
                SnakeState {
                    id: id.clone(),
                    name: player.map(|p| p.name.clone()).unwrap_or_default(),
                    segments: snake.segments().collect(),
                    alive: snake.is_alive(),
                    score: snake.length() as u32,
                    disconnected: player.is_none_or(|p| p.disconnected),
                }
            })
            .collect(),
        apples: game.apples().to_vec(),
    }
}
