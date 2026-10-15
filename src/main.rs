mod input;
mod render;

use apple_logic::game::{Game, Status};
use macroquad::prelude::{get_time, next_frame};

const MOVE_INTERVAL_SECONDS: f64 = 0.10;

#[macroquad::main("FTM Snake")]
async fn main() {
    let mut rng = rand::thread_rng();
    let mut game = Game::new(&mut rng);
    let mut last_move_time = get_time();
    let mut ended_at = None;

    loop {
        for (player, direction) in input::read_directions().into_iter().enumerate() {
            if let Some(direction) = direction {
                game.set_direction(player, direction);
            }
        }
        if game.status() != Status::Running && input::restart_pressed() {
            game = Game::new(&mut rng);
            last_move_time = get_time();
            ended_at = None;
        }
        let now = get_time();
        if now - last_move_time >= MOVE_INTERVAL_SECONDS {
            if game.tick(&mut rng) != Status::Running && ended_at.is_none() {
                ended_at = Some(now);
            }
            last_move_time = now;
        }
        let since_end = ended_at.map(|time| (get_time() - time) as f32);
        render::draw_game(&game, since_end);
        next_frame().await;
    }
}
