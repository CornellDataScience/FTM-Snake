mod player;
mod snake;
mod apple;
mod lib;
use lib::*;
use macroquad::prelude::*;
use std::collections::*;
use player::*;
use snake::Snake;
use apple::*;
use ::rand::Rng;

fn draw_apple(width: i16,
    height: i16,
    snake: &Snake,
    rng: &mut impl Rng) -> (i16, i16) {
        spawn_apple(width, height, snake.body.iter().chain(snake.body.iter()).copied(), rng).unwrap()
    }

fn remove_apple(apples: &mut Vec<(i16, i16)>,
    snake: &mut Snake,
    index: usize,) {
    apples.remove(index);
    let mut rng = ::rand::thread_rng();
    if let Some(new_apple) = spawn_apple(
        SQUARES,
        SQUARES,
        snake.body.iter().chain(std::iter::once(&snake.head)).chain(apples.iter()).copied(),
        &mut rng) {
            apples.push(new_apple);
        }
}
#[macroquad::main("MyGame")]
async fn main() {
    let mut snake = Snake::new((2, 2), Player::new());
    let mut rng = ::rand::thread_rng();

    let mut apples = vec![
        draw_apple(
            SQUARES,
            SQUARES,
            &snake,
            &mut rng,
        ),
    ];
    let mut last_move_time = get_time();
    let move_interval = 0.15;

    loop {

        snake.player.handle_input();

        if get_time() - last_move_time >= move_interval && snake.is_alive() {

            if let Some(index) = snake.step(&apples){
                remove_apple(&mut apples, &mut snake, index);
                //
            }
            last_move_time = get_time();
        }

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
        for &(x, y) in &apples {
            draw_rectangle(
                offset_x + x as f32 * sq_size,
                offset_y + y as f32 * sq_size,
                sq_size,
                sq_size,
                RED,
            );
        }
        next_frame().await
    }
}