use crate::player::Direction;
use crate::player::Player;
use std::collections::*;

pub const SQUARES: i16 = 25;

type Point = (i16, i16);

pub struct Snake {
    pub player: Player,
    pub head: Point,
    pub dir: Point,
    pub body: VecDeque<Point>,
    pub body_set: HashSet<Point>,
    is_alive: bool,
}

impl Snake {
    pub fn new(head: Point, player: Player) -> Self {
        let dir: Point = match player.current_direction() {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        };

        let mut body = VecDeque::new();
        let mut body_set = HashSet::new();

        let starting_size = 3;

        // Head is stored separately, so body starts at i = 1.
        for i in 1..starting_size {
            let point = (
                head.0 - dir.0 * i,
                head.1 - dir.1 * i,
            );

            body.push_back(point);
            body_set.insert(point);
        }

        Snake {
            player,
            head,
            dir,
            body,
            body_set,
            is_alive: true,
        }
    }

    /// Moves the snake one square. Grows if it lands on the apple.
    /// Returns the index of the eaten apple, if any
    pub fn step(&mut self, apples: &[Point]) -> Option<usize> {
        let new_dir: Point = match self.player.current_direction() {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        };

        // Don't allow snake to immediately reverse direction
        if new_dir != (-self.dir.0, -self.dir.1) {
            self.dir = new_dir;
        }

        let new_head = (self.head.0 + self.dir.0, self.head.1 + self.dir.1);
        let eaten_index = apples.iter().position(|apple| *apple == new_head);

        // Only remove the tail if we didn't eat, so the snake grows by one
        if eaten_index.is_none() {
            if let Some(tail) = self.body.pop_back() {
                self.body_set.remove(&tail);
            }
        }

        // Die on wall or self collision
        if new_head.0 < 0 || new_head.1 < 0 || new_head.0 >= SQUARES || new_head.1 >= SQUARES
            || self.body_set.contains(&new_head)
        {
            self.is_alive = false;
            return None;
        }

        self.head = new_head;
        self.body.push_front(new_head);
        self.body_set.insert(new_head);

        eaten_index
    }

    pub fn is_alive(&self) -> bool {
        self.is_alive
    }

    pub fn body_contains(&self, point: &Point) -> bool {
        self.body_set.contains(point)
    }
}
