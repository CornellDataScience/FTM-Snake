use std::collections::{HashSet, VecDeque};

use crate::{Board, Point, player::Direction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Moved,
    AteApple(usize),
    Died,
}

pub struct Snake {
    // All segments include the head at the front, followed by the body and tail.
    segments: VecDeque<Point>,
    occupied: HashSet<Point>,
    direction: Direction,
    pending_direction: Option<Direction>,
    alive: bool,
}

impl Snake {
    /// Creates a three-cell snake facing right. The caller must provide room
    /// for the two body cells to the left of the head.
    pub fn new(head: Point) -> Self {
        let segments = VecDeque::from([head, (head.0 - 1, head.1), (head.0 - 2, head.1)]);
        let occupied = segments.iter().copied().collect();
        Self {
            segments,
            occupied,
            direction: Direction::Right,
            pending_direction: None,
            alive: true,
        }
    }

    pub fn head(&self) -> Point {
        self.segments[0]
    }

    pub fn segments(&self) -> impl Iterator<Item = Point> + '_ {
        self.segments.iter().copied()
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    /// Accepts at most one turn per tick, relative to the last actual movement.
    pub fn set_direction(&mut self, direction: Direction) {
        if self.alive
            && self.pending_direction.is_none()
            && direction != self.direction
            && !self.direction.is_opposite(direction)
        {
            self.pending_direction = Some(direction);
        }
    }

    pub fn step(&mut self, board: Board, apples: &[Point]) -> StepOutcome {
        if !self.alive {
            return StepOutcome::Died;
        }
        if let Some(direction) = self.pending_direction.take() {
            self.direction = direction;
        }
        let (dx, dy) = self.direction.offset();
        let head = self.head();
        let next_head = (head.0 + dx, head.1 + dy);
        let apple_index = apples.iter().position(|apple| *apple == next_head);
        // The tail vacates its cell on a non-growing move.
        let entering_vacating_tail =
            apple_index.is_none() && self.segments.back() == Some(&next_head);
        if !board.contains(next_head)
            || (self.occupied.contains(&next_head) && !entering_vacating_tail)
        {
            self.alive = false;
            return StepOutcome::Died;
        }
        if apple_index.is_none() {
            let tail = self.segments.pop_back().expect("a live snake has a tail");
            self.occupied.remove(&tail);
        }
        self.segments.push_front(next_head);
        self.occupied.insert(next_head);
        match apple_index {
            Some(index) => StepOutcome::AteApple(index),
            None => StepOutcome::Moved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_preserves_length_and_connected_segments() {
        let mut snake = Snake::new((2, 2));
        assert_eq!(snake.step(Board::default(), &[]), StepOutcome::Moved);
        assert_eq!(
            snake.segments().collect::<Vec<_>>(),
            [(3, 2), (2, 2), (1, 2)]
        );
    }

    #[test]
    fn eating_preserves_tail_and_adds_one_segment() {
        let mut snake = Snake::new((2, 2));
        assert_eq!(
            snake.step(Board::default(), &[(3, 2)]),
            StepOutcome::AteApple(0)
        );
        assert_eq!(
            snake.segments().collect::<Vec<_>>(),
            [(3, 2), (2, 2), (1, 2), (0, 2)]
        );
    }

    #[test]
    fn rapid_inputs_cannot_reverse_or_replace_the_pending_turn() {
        let mut snake = Snake::new((2, 2));
        snake.set_direction(Direction::Left);
        snake.set_direction(Direction::Up);
        snake.set_direction(Direction::Left);
        snake.step(Board::default(), &[]);
        assert_eq!(snake.head(), (2, 1));
    }

    #[test]
    fn wall_collision_preserves_segments_and_dead_snakes_do_not_move() {
        let mut snake = Snake::new((24, 2));
        let before: Vec<_> = snake.segments().collect();
        assert_eq!(snake.step(Board::default(), &[]), StepOutcome::Died);
        assert!(!snake.is_alive());
        assert_eq!(snake.step(Board::default(), &[]), StepOutcome::Died);
        assert_eq!(snake.segments().collect::<Vec<_>>(), before);
    }

    #[test]
    fn self_collision_kills_but_entering_a_vacating_tail_is_allowed() {
        let segments = VecDeque::from([(2, 2), (2, 3), (1, 3), (1, 2)]);
        let mut snake = Snake {
            occupied: segments.iter().copied().collect(),
            segments,
            direction: Direction::Up,
            pending_direction: None,
            alive: true,
        };
        snake.set_direction(Direction::Left);
        assert_eq!(snake.step(Board::default(), &[]), StepOutcome::Moved);
        // Growing keeps the tail occupied, making the same move fatal.
        let mut growing = Snake {
            segments: VecDeque::from([(2, 2), (2, 3), (1, 3), (1, 2)]),
            occupied: [(2, 2), (2, 3), (1, 3), (1, 2)].into_iter().collect(),
            direction: Direction::Up,
            pending_direction: None,
            alive: true,
        };
        growing.set_direction(Direction::Left);
        assert_eq!(growing.step(Board::default(), &[(1, 2)]), StepOutcome::Died);
    }
}
