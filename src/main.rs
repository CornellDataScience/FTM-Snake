//! Two-player desktop game on a fixed-rate tick clock.
//! Player 1 steers with WASD, player 2 with the arrow keys; Space restarts.
mod input;
mod render;

use ftm_snake::game::{Game, Status};
use macroquad::prelude::{get_frame_time, get_time, next_frame};

const TICK_SECONDS: f64 = 0.10;
/// Longest frame gap credited to the tick clock, so a stalled window does not
/// replay a burst of moves when it resumes.
const MAX_FRAME_SECONDS: f64 = 0.25;

#[macroquad::main("FTM Snake")]
async fn main() {
    let mut rng = rand::thread_rng();
    let mut game = Game::new(&mut rng);
    let mut pending = 0.0;
    let mut ended_at = None;

    loop {
        if input::restart_pressed() {
            game = Game::new(&mut rng);
            pending = 0.0;
            ended_at = None;
        }
        for (player, direction) in input::read_directions().into_iter().enumerate() {
            if let Some(direction) = direction {
                game.set_direction(player, direction);
            }
        }
        for _ in 0..due_ticks(&mut pending, f64::from(get_frame_time())) {
            if game.tick(&mut rng) != Status::Running && ended_at.is_none() {
                ended_at = Some(get_time());
            }
        }
        let since_end = ended_at.map(|time| (get_time() - time) as f32);
        render::draw_game(&game, since_end);
        next_frame().await;
    }
}

/// Adds one frame's duration to the tick clock and returns how many ticks are due.
fn due_ticks(pending: &mut f64, frame_seconds: f64) -> u32 {
    *pending += frame_seconds.clamp(0.0, MAX_FRAME_SECONDS);
    let mut ticks = 0;
    while *pending >= TICK_SECONDS {
        *pending -= TICK_SECONDS;
        ticks += 1;
    }
    ticks
}