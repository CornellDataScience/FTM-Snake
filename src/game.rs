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
        let mut plan = MovePlan::new(self);
        self.head_on = self.mark_head_on_collisions(&mut plan);
        self.mark_blocked_moves(&mut plan);
        self.apply(&plan);
        self.refill_apples(rng);
        self.status = self.result();
        self.status
    }

    /// Heads entering one cell or passing through each other kill both snakes.
    /// Returns where the colliding heads end up, for the renderer.
    fn mark_head_on_collisions(&self, plan: &mut MovePlan) -> Option<(Point, Point)> {
        let mut head_on = None;
        for i in 0..self.snakes.len() {
            for j in i + 1..self.snakes.len() {
                let (Some(a), Some(b)) = (plan.targets[i], plan.targets[j]) else {
                    continue;
                };
                let (head_i, head_j) = (self.snakes[i].head(), self.snakes[j].head());
                if a == b && self.board.contains(a) {
                    plan.merges[i] = true;
                    plan.merges[j] = true;
                    head_on = Some((a, b));
                } else if a == head_j && b == head_i {
                    head_on = Some((head_i, head_j));
                } else {
                    continue;
                }
                plan.dies[i] = true;
                plan.dies[j] = true;
            }
        }
        head_on
    }

    /// A dying snake does not vacate its tail, which can block another snake,
    /// so repeat until no new deaths appear.
    fn mark_blocked_moves(&self, plan: &mut MovePlan) {
        loop {
            let mut changed = false;
            for i in 0..self.snakes.len() {
                let Some(target) = plan.targets[i] else {
                    continue;
                };
                if !plan.dies[i] && self.is_blocked(target, plan) {
                    plan.dies[i] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Whether entering `cell` is fatal, given every snake's planned move.
    fn is_blocked(&self, cell: Point, plan: &MovePlan) -> bool {
        !self.board.contains(cell)
            || self.snakes.iter().enumerate().any(|(j, snake)| {
                // A tail moves away on a non-growing move.
                let tail_vacates = plan.targets[j].is_some() && !plan.grows[j] && !plan.dies[j];
                snake.occupies(cell) && !(tail_vacates && snake.tail() == cell)
            })
    }

    fn apply(&mut self, plan: &MovePlan) {
        for (i, snake) in self.snakes.iter_mut().enumerate() {
            let Some(target) = plan.targets[i] else {
                continue;
            };
            if plan.dies[i] {
                // Head-on snakes move into the shared cell before dying.
                if plan.merges[i] {
                    snake.advance(false);
                    self.apples.retain(|&apple| apple != target);
                }
                snake.kill();
            } else {
                snake.advance(plan.grows[i]);
                if plan.grows[i] {
                    self.apples.retain(|&apple| apple != target);
                }
            }
        }
    }

    fn result(&self) -> Status {
        let mut alive = (0..self.snakes.len()).filter(|&i| self.snakes[i].is_alive());
        match (alive.next(), alive.next()) {
            (None, _) => Status::Draw,
            (Some(player), None) => Status::Winner(player),
            _ => Status::Running,
        }
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

/// Every snake's move for one tick, indexed like `Game::snakes`. Decided in
/// full before any snake advances.
struct MovePlan {
    /// The cell each living snake's head will enter; `None` for dead snakes.
    targets: Vec<Option<Point>>,
    grows: Vec<bool>,
    dies: Vec<bool>,
    /// Snakes whose heads move into a shared cell before dying.
    merges: Vec<bool>,
}

impl MovePlan {
    fn new(game: &Game) -> Self {
        let targets: Vec<_> = game
            .snakes
            .iter()
            .map(|snake| snake.is_alive().then(|| snake.next_head()))
            .collect();
        let grows = targets
            .iter()
            .map(|target| target.is_some_and(|cell| game.apples.contains(&cell)))
            .collect();
        let count = targets.len();
        Self {
            targets,
            grows,
            dies: vec![false; count],
            merges: vec![false; count],
        }
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
