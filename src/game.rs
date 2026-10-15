use rand::Rng;

use crate::{
    APPLE_COUNT, Board, PLAYER_COUNT, Point, apple::spawn_apple, player::Direction, snake::Snake,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    /// Index of the last snake left alive.
    Winner(usize),
    /// Every remaining snake died on the same tick, including head-on collisions.
    Draw,
}

/// Owns game state without depending on a window, keyboard, or wall clock.
pub struct Game {
    board: Board,
    snakes: [Snake; PLAYER_COUNT],
    apples: Vec<Point>,
    status: Status,
    head_on: Option<(Point, Point)>,
}

impl Game {
    pub fn new(rng: &mut impl Rng) -> Self {
        let board = Board::default();
        let mut game = Self {
            board,
            snakes: [
                Snake::new((2, 2), Direction::Right),
                Snake::new((board.width - 3, board.height - 3), Direction::Left),
            ],
            apples: Vec::new(),
            status: Status::Running,
            head_on: None,
        };
        game.refill_apples(rng);
        game
    }

    pub fn board(&self) -> Board {
        self.board
    }
    pub fn snakes(&self) -> &[Snake] {
        &self.snakes
    }
    pub fn apples(&self) -> &[Point] {
        &self.apples
    }
    pub fn status(&self) -> Status {
        self.status
    }
    /// Where the two heads ended up after a head-on collision: the same cell
    /// when they met there, or adjacent cells when they tried to pass through
    /// each other.
    pub fn head_on(&self) -> Option<(Point, Point)> {
        self.head_on
    }

    pub fn set_direction(&mut self, player: usize, direction: Direction) {
        self.snakes[player].set_direction(direction);
    }

    /// Moves every living snake at once. The match ends when at most one
    /// snake is left alive; dead snakes remain on the board as obstacles.
    pub fn tick(&mut self, rng: &mut impl Rng) -> Status {
        if self.status != Status::Running {
            return self.status;
        }
        let targets: [Option<Point>; PLAYER_COUNT] = std::array::from_fn(|i| {
            self.snakes[i]
                .is_alive()
                .then(|| self.snakes[i].next_head())
        });
        let grows = targets.map(|target| target.is_some_and(|cell| self.apples.contains(&cell)));
        let mut dies = [false; PLAYER_COUNT];
        // Snakes whose heads move into a shared cell before dying.
        let mut merges = [false; PLAYER_COUNT];

        // Heads entering one cell or passing through each other kill both snakes.
        for i in 0..PLAYER_COUNT {
            for j in i + 1..PLAYER_COUNT {
                let (Some(a), Some(b)) = (targets[i], targets[j]) else {
                    continue;
                };
                let (head_i, head_j) = (self.snakes[i].head(), self.snakes[j].head());
                if a == b && self.board.contains(a) {
                    merges[i] = true;
                    merges[j] = true;
                    self.head_on = Some((a, b));
                } else if a == head_j && b == head_i {
                    self.head_on = Some((head_i, head_j));
                } else {
                    continue;
                }
                dies[i] = true;
                dies[j] = true;
            }
        }
        // A dying snake does not vacate its tail, which can block another
        // snake, so repeat until no new deaths appear.
        loop {
            let mut changed = false;
            for i in 0..PLAYER_COUNT {
                let Some(target) = targets[i] else { continue };
                if !dies[i] && self.is_blocked(target, &targets, &grows, &dies) {
                    dies[i] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        for (i, snake) in self.snakes.iter_mut().enumerate() {
            match (targets[i], dies[i]) {
                (None, _) => {}
                (Some(target), true) => {
                    if merges[i] {
                        snake.advance(false);
                        self.apples.retain(|&apple| apple != target);
                    }
                    snake.kill();
                }
                (Some(target), false) => {
                    snake.advance(grows[i]);
                    if grows[i] {
                        self.apples.retain(|&apple| apple != target);
                    }
                }
            }
        }
        self.refill_apples(rng);

        let mut alive = (0..PLAYER_COUNT).filter(|&i| self.snakes[i].is_alive());
        self.status = match (alive.next(), alive.next()) {
            (None, _) => Status::Draw,
            (Some(player), None) => Status::Winner(player),
            _ => Status::Running,
        };
        self.status
    }

    /// Whether entering `cell` is fatal, given every snake's planned move.
    fn is_blocked(
        &self,
        cell: Point,
        targets: &[Option<Point>; PLAYER_COUNT],
        grows: &[bool; PLAYER_COUNT],
        dies: &[bool; PLAYER_COUNT],
    ) -> bool {
        !self.board.contains(cell)
            || self.snakes.iter().enumerate().any(|(j, snake)| {
                // A tail moves away on a non-growing move.
                let tail_vacates = targets[j].is_some() && !grows[j] && !dies[j];
                snake.occupies(cell) && !(tail_vacates && snake.tail() == cell)
            })
    }

    fn refill_apples(&mut self, rng: &mut impl Rng) {
        while self.apples.len() < APPLE_COUNT {
            let occupied = self
                .snakes
                .iter()
                .flat_map(Snake::segments)
                .chain(self.apples.iter().copied());
            let Some(apple) = spawn_apple(self.board.width, self.board.height, occupied, rng)
            else {
                break;
            };
            self.apples.push(apple);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snake::snake_from_segments;
    use rand::{SeedableRng, rngs::StdRng};

    fn game_with(snakes: [Snake; PLAYER_COUNT], apples: &[Point]) -> Game {
        Game {
            board: Board::default(),
            snakes,
            apples: apples.to_vec(),
            status: Status::Running,
            head_on: None,
        }
    }

    fn rng() -> StdRng {
        StdRng::seed_from_u64(11)
    }

    fn assert_apples_clear_of_snakes(game: &Game) {
        assert!(
            !game
                .snakes()
                .iter()
                .flat_map(Snake::segments)
                .any(|cell| game.apples().contains(&cell))
        );
    }

    #[test]
    fn initial_apples_exclude_every_snake_segment() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..1000 {
            let game = Game::new(&mut rng);
            assert_eq!(game.apples().len(), APPLE_COUNT);
            assert_apples_clear_of_snakes(&game);
        }
    }

    #[test]
    fn eating_replaces_apple_outside_both_snakes() {
        let mut rng = rng();
        let mut game = Game::new(&mut rng);
        game.apples = vec![(3, 2)];
        game.tick(&mut rng);
        assert_eq!(game.snakes()[0].length(), 4);
        assert_eq!(game.apples().len(), APPLE_COUNT);
        assert!(!game.apples().contains(&(3, 2)));
        assert_apples_clear_of_snakes(&game);
    }

    #[test]
    fn players_steer_their_own_snakes() {
        let mut rng = rng();
        let mut game = Game::new(&mut rng);
        game.apples.clear();
        game.set_direction(0, Direction::Down);
        game.set_direction(1, Direction::Up);
        game.tick(&mut rng);
        assert_eq!(game.snakes()[0].head(), (2, 3));
        assert_eq!(game.snakes()[1].head(), (22, 21));
    }

    #[test]
    fn wall_collision_preserves_segments_and_the_survivor_wins() {
        let mut rng = rng();
        let mut game = game_with(
            [
                snake_from_segments(&[(24, 2), (23, 2), (22, 2), (21, 2)], Direction::Right),
                Snake::new((10, 10), Direction::Right),
            ],
            &[],
        );
        let before: Vec<_> = game.snakes()[0].segments().collect();
        // The survivor wins even though it is shorter.
        assert_eq!(game.tick(&mut rng), Status::Winner(1));
        assert_eq!(game.tick(&mut rng), Status::Winner(1));
        assert_eq!(game.snakes()[0].segments().collect::<Vec<_>>(), before);
        assert_eq!(game.head_on(), None);
    }

    #[test]
    fn head_on_collision_moves_both_heads_into_the_cell_and_is_a_draw() {
        let mut rng = rng();
        let mut game = game_with(
            [
                snake_from_segments(&[(5, 5), (4, 5), (3, 5), (2, 5)], Direction::Right),
                Snake::new((7, 5), Direction::Left),
            ],
            &[(6, 5)],
        );
        assert_eq!(game.tick(&mut rng), Status::Draw);
        assert_eq!(game.head_on(), Some(((6, 5), (6, 5))));
        assert!(game.snakes().iter().all(|snake| snake.head() == (6, 5)));
        assert_eq!(game.snakes()[0].length(), 4);
        assert!(!game.apples().contains(&(6, 5)));
    }

    #[test]
    fn heads_passing_through_each_other_is_a_draw() {
        let mut rng = rng();
        let mut game = game_with(
            [
                Snake::new((5, 5), Direction::Right),
                Snake::new((6, 5), Direction::Left),
            ],
            &[],
        );
        assert_eq!(game.tick(&mut rng), Status::Draw);
        assert_eq!(game.head_on(), Some(((5, 5), (6, 5))));
    }

    #[test]
    fn hitting_the_other_snakes_body_loses() {
        let mut rng = rng();
        let mut game = game_with(
            [
                Snake::new((5, 4), Direction::Down),
                Snake::new((6, 5), Direction::Right),
            ],
            &[],
        );
        assert_eq!(game.tick(&mut rng), Status::Winner(1));
    }

    #[test]
    fn entering_a_vacating_tail_is_allowed_unless_its_snake_grows() {
        // Snake 0 moves into snake 1's tail at (5, 5) as snake 1 moves away.
        let snakes = || {
            [
                Snake::new((5, 4), Direction::Down),
                Snake::new((7, 5), Direction::Right),
            ]
        };
        let mut rng = rng();
        let mut game = game_with(snakes(), &[]);
        game.tick(&mut rng);
        assert!(game.snakes().iter().all(Snake::is_alive));

        let mut growing = game_with(snakes(), &[(8, 5)]);
        growing.tick(&mut rng);
        assert!(!growing.snakes()[0].is_alive());
    }

    #[test]
    fn tail_stays_blocking_when_its_snake_dies() {
        // Snake 1 hits the wall, so its tail at (22, 5) does not vacate.
        let mut rng = rng();
        let mut game = game_with(
            [
                Snake::new((22, 4), Direction::Down),
                Snake::new((24, 5), Direction::Right),
            ],
            &[],
        );
        game.tick(&mut rng);
        assert!(game.snakes().iter().all(|snake| !snake.is_alive()));
    }

    #[test]
    fn self_collision_kills_but_entering_own_vacating_tail_is_allowed() {
        let curled = || snake_from_segments(&[(2, 2), (2, 3), (1, 3), (1, 2)], Direction::Up);
        let mut rng = rng();
        let mut game = game_with([curled(), Snake::new((10, 10), Direction::Right)], &[]);
        game.set_direction(0, Direction::Left);
        game.tick(&mut rng);
        assert!(game.snakes()[0].is_alive());
        // Growing keeps the tail occupied, making the same move fatal.
        let mut growing = game_with(
            [curled(), Snake::new((10, 10), Direction::Right)],
            &[(1, 2)],
        );
        growing.set_direction(0, Direction::Left);
        growing.tick(&mut rng);
        assert!(!growing.snakes()[0].is_alive());
    }

    #[test]
    fn simultaneous_deaths_are_a_draw() {
        let mut rng = rng();
        let mut game = game_with(
            [
                snake_from_segments(&[(24, 2), (23, 2), (22, 2), (21, 2)], Direction::Right),
                Snake::new((24, 10), Direction::Right),
            ],
            &[],
        );
        assert_eq!(game.tick(&mut rng), Status::Draw);
        assert_eq!(game.head_on(), None);
    }
}
