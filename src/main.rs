mod input;
mod render;

use apple_logic::game::Game;
use macroquad::prelude::{get_time, next_frame};

const MOVE_INTERVAL_SECONDS: f64 = 0.10;

#[macroquad::main("FTM Snake")]
async fn main() {
    let mut rng = rand::thread_rng();
    let mut game = Game::new(&mut rng);
    let mut last_move_time = get_time();

    loop {
        if let Some(direction) = input::read_direction() {
            game.set_direction(direction);
        }
        let now = get_time();
        if now - last_move_time >= MOVE_INTERVAL_SECONDS {
            game.tick(&mut rng);
            last_move_time = now;
        }
        render::draw_game(&game);
        next_frame().await;
    }
}
