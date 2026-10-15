use rand::Rng;

use crate::{
    Board, Point,
    apple::spawn_apple,
    player::Direction,
    snake::{Snake, StepOutcome},
};

/// Owns game state without depending on a window, keyboard, or wall clock.
pub struct Game {
    board: Board,
    snake: Snake,
    apples: Vec<Point>,
}

impl Game {
    pub fn new(rng: &mut impl Rng) -> Self {
        let mut game = Self {
            board: Board::default(),
            snake: Snake::new((2, 2)),
            apples: Vec::new(),
        };
        game.spawn_apple(rng);
        game
    }

    pub fn board(&self) -> Board {
        self.board
    }
    pub fn snake(&self) -> &Snake {
        &self.snake
    }
    pub fn apples(&self) -> &[Point] {
        &self.apples
    }

    pub fn set_direction(&mut self, direction: Direction) {
        self.snake.set_direction(direction);
    }

    pub fn tick(&mut self, rng: &mut impl Rng) -> StepOutcome {
        let outcome = self.snake.step(self.board, &self.apples);
        if let StepOutcome::AteApple(index) = outcome {
            self.apples.remove(index);
            self.spawn_apple(rng);
        }
        outcome
    }

    fn spawn_apple(&mut self, rng: &mut impl Rng) {
        let occupied = self.snake.segments().chain(self.apples.iter().copied());
        if let Some(apple) = spawn_apple(self.board.width, self.board.height, occupied, rng) {
            self.apples.push(apple);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn initial_apple_excludes_every_snake_segment_including_head() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..1000 {
            let game = Game::new(&mut rng);
            assert_eq!(game.apples().len(), 1);
            assert!(
                !game
                    .snake()
                    .segments()
                    .any(|cell| game.apples().contains(&cell))
            );
        }
    }

    #[test]
    fn eating_replaces_apple_outside_the_grown_snake() {
        let mut rng = StdRng::seed_from_u64(11);
        let mut game = Game::new(&mut rng);
        game.apples = vec![(3, 2)];
        assert_eq!(game.tick(&mut rng), StepOutcome::AteApple(0));
        assert_eq!(game.apples().len(), 1);
        assert!(
            !game
                .snake()
                .segments()
                .any(|cell| game.apples().contains(&cell))
        );
    }
}
