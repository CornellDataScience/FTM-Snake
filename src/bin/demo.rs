//! Live demo of the current two-player game on a fixed-rate tick clock.
//! Player 1 steers with WASD, player 2 with the arrow keys; Space restarts.
#[path = "../input.rs"]
mod input;
#[path = "../render.rs"]
mod render;

use apple_logic::game::{Game, Status};
use macroquad::prelude::{get_frame_time, get_time, next_frame};

const TICK_SECONDS: f64 = 0.10;
/// Longest frame gap credited to the tick clock, so a stalled window does not
/// replay a burst of moves when it resumes.
const MAX_FRAME_SECONDS: f64 = 0.25;

#[macroquad::main("FTM Snake demo")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_frames_carry_over_until_a_tick_is_due() {
        let mut pending = 0.0;
        assert_eq!(due_ticks(&mut pending, 0.06), 0);
        assert_eq!(due_ticks(&mut pending, 0.06), 1);
        assert!((pending - 0.02).abs() < 1e-9);
    }

    #[test]
    fn ticks_keep_their_rate_regardless_of_frame_rate() {
        let (mut fast, mut slow) = (0.0, 0.0);
        // About 1.05 seconds each, clear of a tick boundary.
        let fast_ticks: u32 = (0..126).map(|_| due_ticks(&mut fast, 1.0 / 120.0)).sum();
        let slow_ticks: u32 = (0..32).map(|_| due_ticks(&mut slow, 1.0 / 30.0)).sum();
        assert_eq!(fast_ticks, 10);
        assert_eq!(slow_ticks, 10);
    }

    #[test]
    fn a_stalled_frame_is_capped() {
        let mut pending = 0.0;
        assert_eq!(due_ticks(&mut pending, 5.0), 2);
        assert!(pending < TICK_SECONDS);
    }
}
