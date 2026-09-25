use macroquad::prelude::*;
use std::collections::*;
use Player;
type Point = (i16, i16);

const SQUARES: i16 = 25;
const SQUARE_SIZE: f32 = 20.;

//PLACEHOLDER FOR ELLA AND WONJIN LATER


struct Snake {
    player: Player,
    head: Point,
    dir: Point,
    body: VecDeque<Point>,
    body_set: HashSet<Point>,
    is_alive: bool
}

impl Snake {
    fn new(head: Point, player: Player) -> Self {
        // Fix intial direction to be inline with player, and initlize tail
        let dir: Point = match player.current_direction() {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        };
        //continue here

        
        let mut body = VecDeque::new();
        let mut body_set = HashSet::new();

        body.push_front(head);
        body_set.insert(head);

        Snake {
            head,
            dir: (1, 0),
            body,
            body_set,
            is_alive: true,
        }
    }

    fn move_snake(&mut self) {
        // Get player's most recent direction
        let new_dir = self.player.current_direction;

        // Don't allow snake to immediately reverse direction
        if new_dir != (-self.dir.0, -self.dir.1) {
            self.dir = new_dir;
        }

        // Calculate new head
        let new_head = (
            self.head.0 + self.dir.0,
            self.head.1 + self.dir.1,
        );

        // Add new head
        self.head = new_head;
        self.body.push_front(new_head);
        self.body_set.insert(new_head);

        // Remove tail
        if let Some(tail) = self.body.pop_back() {
            self.body_set.remove(&tail);
        }
    }

    fn grow(&mut self, player: &Player) {
        let new_dir = player.current_direction;

        if new_dir != (-self.dir.0, -self.dir.1) {
            self.dir = new_dir;
        }

        let new_head = (
            self.head.0 + self.dir.0,
            self.head.1 + self.dir.1,
        );

        self.head = new_head;
        self.body.push_front(new_head);
        self.body_set.insert(new_head);

        // Don't remove tail -> snake grows
    }

    fn hit_self(&self) -> bool {
        self.body
            .iter()
            .skip(1)
            .any(|point| *point == self.head)
    }

    fn out_of_bounds(&self) -> bool {
        self.head.0 < 0
            || self.head.1 < 0
            || self.head.0 >= SQUARES
            || self.head.1 >= SQUARES
    }

    fn body_contains(&self, point: &Point) -> bool {
        self.body_set.contains(point)
    }
}

#[macroquad::main("MyGame")]
async fn main() {
    let snake = Snake::new((2,2));

    loop {

        let game_size = screen_width().min(screen_height());
        let offset_x = (screen_width() - game_size) / 2. + 10.;
        let offset_y = (screen_height() - game_size) / 2. + 10.;
        let sq_size = (screen_height() - offset_y * 2.) / SQUARES as f32;

        draw_rectangle(offset_x, offset_y, game_size - 20., game_size - 20., WHITE);
        
        for i in 1..SQUARES {
                draw_line(
                    offset_x,
                    offset_y + sq_size * i as f32,
                    screen_width() - offset_x,
                    offset_y + sq_size * i as f32,
                    2.,
                    LIGHTGRAY,
            );
        }

        for i in 1..SQUARES {
            draw_line(
                offset_x + sq_size * i as f32,
                offset_y,
                offset_x + sq_size * i as f32,
                screen_height() - offset_y,
                2.,
                LIGHTGRAY,
            );
        }

        for (x, y) in &snake.body {
            draw_rectangle(
                offset_x + *x as f32 * sq_size,
                offset_y + *y as f32 * sq_size,
                sq_size,
                sq_size,
                LIME,
            );
        }

        draw_rectangle(
            offset_x + snake.head.0 as f32 * sq_size,
            offset_y + snake.head.1 as f32 * sq_size,
            sq_size,
            sq_size,
            DARKGREEN,
        );

        next_frame().await
    }
}