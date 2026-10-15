use std::collections::{HashSet, VecDeque};

use crate::{Point, player::Direction};

/// Segment storage and turning. Collision rules live in `Game`, which sees
/// every snake's planned move before any of them advance.
pub struct Snake {
    // All segments include the head at the front, followed by the body and tail.
    segments: VecDeque<Point>,
    occupied: HashSet<Point>,
    direction: Direction,
    pending_direction: Option<Direction>,
    alive: bool,
}

impl Snake {
    /// Creates a three-cell snake facing `direction`. The caller must provide
    /// room for the two body cells behind the head.
    pub fn new(head: Point, direction: Direction) -> Self {
        let (dx, dy) = direction.offset();
        Self::from_segments(
            VecDeque::from([
                head,
                (head.0 - dx, head.1 - dy),
                (head.0 - 2 * dx, head.1 - 2 * dy),
            ]),
            direction,
        )
    }

    fn from_segments(segments: VecDeque<Point>, direction: Direction) -> Self {
        let occupied = segments.iter().copied().collect();
        Self {
            segments,
            occupied,
            direction,
            pending_direction: None,
            alive: true,
        }
    }

    pub fn head(&self) -> Point {
        self.segments[0]
    }

    pub fn tail(&self) -> Point {
        *self.segments.back().expect("a snake has a tail")
    }

    pub fn length(&self) -> usize {
        self.segments.len()
    }

    pub fn segments(&self) -> impl Iterator<Item = Point> + '_ {
        self.segments.iter().copied()
    }

    pub fn occupies(&self, cell: Point) -> bool {
        self.occupied.contains(&cell)
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

    /// The cell the head will enter on the next advance, including any pending turn.
    pub fn next_head(&self) -> Point {
        let (dx, dy) = self.pending_direction.unwrap_or(self.direction).offset();
        let head = self.head();
        (head.0 + dx, head.1 + dy)
    }

    /// Moves one cell. A growing move keeps the tail in place.
    pub fn advance(&mut self, grow: bool) {
        let next_head = self.next_head();
        if let Some(direction) = self.pending_direction.take() {
            self.direction = direction;
        }
        if !grow {
            let tail = self.segments.pop_back().expect("a snake has a tail");
            self.occupied.remove(&tail);
        }
        self.segments.push_front(next_head);
        self.occupied.insert(next_head);
    }

    /// Stops the snake. Its segments stay where they were.
    pub fn kill(&mut self) {
        self.alive = false;
        self.pending_direction = None;
    }
}

#[cfg(test)]
pub(crate) fn snake_from_segments(segments: &[Point], direction: Direction) -> Snake {
    Snake::from_segments(segments.iter().copied().collect(), direction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_snake_extends_behind_its_direction() {
        let snake = Snake::new((22, 22), Direction::Left);
        assert_eq!(
            snake.segments().collect::<Vec<_>>(),
            [(22, 22), (23, 22), (24, 22)]
        );
    }

    #[test]
    fn movement_preserves_length_and_connected_segments() {
        let mut snake = Snake::new((2, 2), Direction::Right);
        snake.advance(false);
        assert_eq!(
            snake.segments().collect::<Vec<_>>(),
            [(3, 2), (2, 2), (1, 2)]
        );
        assert!(!snake.occupies((0, 2)));
    }

    #[test]
    fn eating_preserves_tail_and_adds_one_segment() {
        let mut snake = Snake::new((2, 2), Direction::Right);
        snake.advance(true);
        assert_eq!(
            snake.segments().collect::<Vec<_>>(),
            [(3, 2), (2, 2), (1, 2), (0, 2)]
        );
    }

    #[test]
    fn rapid_inputs_cannot_reverse_or_replace_the_pending_turn() {
        let mut snake = Snake::new((2, 2), Direction::Right);
        snake.set_direction(Direction::Left);
        snake.set_direction(Direction::Up);
        snake.set_direction(Direction::Left);
        assert_eq!(snake.next_head(), (2, 1));
        snake.advance(false);
        assert_eq!(snake.head(), (2, 1));
    }

    #[test]
    fn dead_snakes_ignore_turns() {
        let mut snake = Snake::new((2, 2), Direction::Right);
        snake.kill();
        snake.set_direction(Direction::Up);
        assert!(!snake.is_alive());
        assert_eq!(snake.next_head(), (3, 2));
    }
}
