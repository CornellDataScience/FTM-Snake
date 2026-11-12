use std::collections::HashSet;

use rand::{Rng, seq::IteratorRandom};

use crate::{
    Board, LOCAL_PLAYER_COUNT, Point,
    config::{GameConfig, SPAWN_SPACING},
    player::Direction,
    snake::Snake,
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
    snakes: Vec<Snake>,
    apple_count: usize,
    apples: Vec<Point>,
    status: Status,
    head_on: Option<(Point, Point)>,
}

impl Game {
    /// The desktop game keeps its original two-player controls and defaults.
    pub fn new(rng: &mut impl Rng) -> Self {
        Self::with_config(GameConfig::default(), LOCAL_PLAYER_COUNT, rng)
            .expect("default game configuration is valid")
    }

    pub fn with_config(
        config: GameConfig,
        player_count: usize,
        rng: &mut impl Rng,
    ) -> Result<Self, String> {
        config.validate()?;
        if player_count < 2 || player_count > config.player_capacity() {
            return Err(format!(
                "This board supports 2 to {} players",
                config.player_capacity()
            ));
        }
        let mut game = Self {
            board: config.board,
            snakes: starting_snakes(config, player_count),
            apple_count: config.apple_count,
            apples: Vec::new(),
            status: Status::Running,
            head_on: None,
        };
        game.refill_apples(rng);
        Ok(game)
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
        if let Some(snake) = self.snakes.get_mut(player) {
            snake.set_direction(direction);
        }
    }

    /// Moves every living snake at once. The match ends when at most one
    /// snake is left alive; dead snakes remain on the board as obstacles.
    pub fn tick(&mut self, rng: &mut impl Rng) -> Status {
        if self.status != Status::Running {
            return self.status;
        }
        self.head_on = None;
        let player_count = self.snakes.len();
        let targets: Vec<_> = self
            .snakes
            .iter()
            .map(|snake| snake.is_alive().then(|| snake.next_head()))
            .collect();
        let grows: Vec<_> = targets
            .iter()
            .map(|target| target.is_some_and(|cell| self.apples.contains(&cell)))
            .collect();
        let mut dies = vec![false; player_count];
        // Snakes whose heads move into a shared cell before dying.
        let mut merges = vec![false; player_count];

        // Heads entering one cell or passing through each other kill both snakes.
        for i in 0..player_count {
            for j in i + 1..player_count {
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
            for i in 0..player_count {
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

        let mut alive = (0..player_count).filter(|&i| self.snakes[i].is_alive());
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
        targets: &[Option<Point>],
        grows: &[bool],
        dies: &[bool],
    ) -> bool {
        !self.board.contains(cell)
            || self.snakes.iter().enumerate().any(|(j, snake)| {
                // A tail moves away on a non-growing move.
                let tail_vacates = targets[j].is_some() && !grows[j] && !dies[j];
                snake.occupies(cell) && !(tail_vacates && snake.tail() == cell)
            })
    }

    fn refill_apples(&mut self, rng: &mut impl Rng) {
        let missing = self.apple_count.saturating_sub(self.apples.len());
        if missing == 0 {
            return;
        }
        let occupied: HashSet<_> = self
            .snakes
            .iter()
            .flat_map(Snake::segments)
            .chain(self.apples.iter().copied())
            .collect();
        // Scan once for all missing apples, rather than once per apple.
        let free_cells = (0..self.board.height)
            .flat_map(|y| (0..self.board.width).map(move |x| (x, y)))
            .filter(|cell| !occupied.contains(cell));
        self.apples.extend(free_cells.choose_multiple(rng, missing));
    }
}

/// Evenly spread starting areas across the board; bodies never overlap and
/// every snake has room for its first move. Larger boards create more areas.
fn starting_snakes(config: GameConfig, player_count: usize) -> Vec<Snake> {
    let board = config.board;
    // Preserve the familiar opposite-corner layout for two-player games.
    if player_count == 2 {
        return vec![
            Snake::new((2, 2), Direction::Right),
            Snake::new((board.width - 3, board.height - 3), Direction::Left),
        ];
    }
    let columns = (board.width / SPAWN_SPACING) as usize;
    (0..player_count)
        .map(|i| {
            let slot = i * config.player_capacity() / player_count;
            let column = slot % columns;
            let row = slot / columns;
            let head = (
                column as i16 * SPAWN_SPACING + 2,
                row as i16 * SPAWN_SPACING + 2,
            );
            let direction = if (column + row).is_multiple_of(2) {
                Direction::Right
            } else {
                Direction::Left
            };
            Snake::new(head, direction)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{APPLE_COUNT, snake::snake_from_segments};
    use rand::{SeedableRng, rngs::StdRng};

    fn game_with<const N: usize>(snakes: [Snake; N], apples: &[Point]) -> Game {
        Game {
            board: Board::default(),
            snakes: snakes.into(),
            apple_count: APPLE_COUNT,
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

    #[test]
    fn variable_player_counts_spawn_without_overlap_and_survive_first_tick() {
        for (width, height) in [(5, 10), (10, 5), (25, 25), (31, 17), (50, 50)] {
            let config = GameConfig {
                board: Board { width, height },
                apple_count: 7,
            };
            for count in 2..=config.player_capacity() {
                let mut game = Game::with_config(config, count, &mut rng()).unwrap();
                let cells: HashSet<_> = game.snakes().iter().flat_map(Snake::segments).collect();
                assert_eq!(cells.len(), count * 3);
                assert!(cells.iter().all(|&cell| game.board().contains(cell)));
                assert_eq!(game.apples().len(), 7);
                assert_apples_clear_of_snakes(&game);
                game.tick(&mut rng());
                assert!(game.snakes().iter().all(Snake::is_alive));
            }
        }
    }

    #[test]
    fn one_elimination_does_not_end_a_multiplayer_round() {
        let mut game = game_with(
            [
                Snake::new((24, 2), Direction::Right),
                Snake::new((10, 10), Direction::Right),
                Snake::new((15, 15), Direction::Right),
            ],
            &[],
        );
        game.set_direction(2, Direction::Down);
        assert_eq!(game.tick(&mut rng()), Status::Running);
        assert!(!game.snakes()[0].is_alive());
        assert_eq!(game.snakes()[2].head(), (15, 16));
        let dead_body: Vec<_> = game.snakes()[0].segments().collect();
        assert_eq!(game.tick(&mut rng()), Status::Running);
        assert_eq!(game.snakes()[0].segments().collect::<Vec<_>>(), dead_body);
    }

    #[test]
    fn three_heads_colliding_leave_the_fourth_player_as_winner() {
        let mut game = game_with(
            [
                Snake::new((4, 5), Direction::Right),
                Snake::new((6, 5), Direction::Left),
                Snake::new((5, 4), Direction::Down),
                Snake::new((15, 15), Direction::Right),
            ],
            &[],
        );
        assert_eq!(game.tick(&mut rng()), Status::Winner(3));
        assert!(
            game.snakes()[..3]
                .iter()
                .all(|snake| !snake.is_alive() && snake.head() == (5, 5))
        );
    }

    #[test]
    fn two_separate_head_on_collisions_are_a_four_player_draw() {
        let mut game = game_with(
            [
                Snake::new((4, 5), Direction::Right),
                Snake::new((6, 5), Direction::Left),
                Snake::new((4, 15), Direction::Right),
                Snake::new((6, 15), Direction::Left),
            ],
            &[],
        );
        assert_eq!(game.tick(&mut rng()), Status::Draw);
    }

    #[test]
    fn apple_target_handles_zero_and_a_full_board() {
        for (target, expected) in [(0, 0), (50, 44)] {
            let config = GameConfig {
                board: Board {
                    width: 5,
                    height: 10,
                },
                apple_count: target,
            };
            let mut game = Game::with_config(config, 2, &mut rng()).unwrap();
            assert_eq!(game.apples().len(), expected);
            assert_eq!(game.apples().iter().collect::<HashSet<_>>().len(), expected);
            assert_apples_clear_of_snakes(&game);
            game.tick(&mut rng());
            assert_apples_clear_of_snakes(&game);
            assert!(game.apples().len() <= target);
        }
    }

    #[test]
    fn rejects_player_counts_that_cannot_form_a_match() {
        let config = GameConfig::default();
        for count in [0, 1, config.player_capacity() + 1] {
            assert!(Game::with_config(config, count, &mut rng()).is_err());
        }
    }
}
