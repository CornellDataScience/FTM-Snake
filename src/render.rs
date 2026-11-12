use apple_logic::{
    LOCAL_PLAYER_COUNT, Point,
    game::{Game, Status},
};
use macroquad::prelude::*;

const BOARD_MARGIN: f32 = 10.0;
const FONT_SIZE: f32 = 32.0;
/// Body and head colors per player.
const SNAKE_COLORS: [(Color, Color); LOCAL_PLAYER_COUNT] = [(LIME, DARKGREEN), (SKYBLUE, DARKBLUE)];
const DEAD_COLORS: (Color, Color) = (GRAY, DARKGRAY);
/// Particles spiral in for `COLLAPSE_SECONDS`, then a flash lasts until
/// `IMPLOSION_SECONDS`, after which the result is shown.
const COLLAPSE_SECONDS: f32 = 0.6;
const IMPLOSION_SECONDS: f32 = 0.9;
const IMPLOSION_PARTICLES: usize = 16;
/// Starting distance of the particles from the crash, in cells.
const IMPLOSION_RADIUS: f32 = 4.0;

/// `since_end` is the time in seconds since the match ended, if it has.
pub fn draw_game(game: &Game, since_end: Option<f32>) {
    clear_background(BLACK);
    let board = game.board();
    let cell_size = ((screen_width() - 2.0 * BOARD_MARGIN) / f32::from(board.width))
        .min((screen_height() - 2.0 * BOARD_MARGIN) / f32::from(board.height))
        .max(0.0);
    let width = cell_size * f32::from(board.width);
    let height = cell_size * f32::from(board.height);
    let origin = vec2(
        (screen_width() - width) / 2.0,
        (screen_height() - height) / 2.0,
    );

    draw_rectangle(origin.x, origin.y, width, height, WHITE);
    for column in 1..board.width {
        let x = origin.x + f32::from(column) * cell_size;
        draw_line(x, origin.y, x, origin.y + height, 2.0, LIGHTGRAY);
    }
    for row in 1..board.height {
        let y = origin.y + f32::from(row) * cell_size;
        draw_line(origin.x, y, origin.x + width, y, 2.0, LIGHTGRAY);
    }
    for (snake, colors) in game.snakes().iter().zip(SNAKE_COLORS) {
        let (body, head) = if snake.is_alive() {
            colors
        } else {
            DEAD_COLORS
        };
        for cell in snake.segments() {
            draw_cell(cell, origin, cell_size, body);
        }
        draw_cell(snake.head(), origin, cell_size, head);
    }
    for &cell in game.apples() {
        draw_cell(cell, origin, cell_size, RED);
    }
    if let Some(heads) = game.head_on() {
        draw_head_on(heads, origin, cell_size, since_end.unwrap_or(0.0));
    }
    draw_scores(game, origin);
    let result_delay = if game.head_on().is_some() {
        IMPLOSION_SECONDS
    } else {
        0.0
    };
    if since_end.is_some_and(|elapsed| elapsed >= result_delay) {
        draw_result(game.status());
    }
}

/// Draws both heads in their player colors (split diagonally when they share
/// a cell) and an implosion collapsing onto the point between them.
fn draw_head_on((a, b): (Point, Point), origin: Vec2, size: f32, elapsed: f32) {
    let [(_, color_a), (_, color_b)] = SNAKE_COLORS;
    if a == b {
        let corner = cell_corner(a, origin, size);
        let top_right = corner + vec2(size, 0.0);
        let bottom_left = corner + vec2(0.0, size);
        draw_triangle(corner, top_right, bottom_left, color_a);
        draw_triangle(corner + vec2(size, size), top_right, bottom_left, color_b);
    } else {
        draw_cell(a, origin, size, color_a);
        draw_cell(b, origin, size, color_b);
    }

    let center = (cell_corner(a, origin, size) + cell_corner(b, origin, size)) / 2.0
        + vec2(size, size) / 2.0;
    if elapsed < COLLAPSE_SECONDS {
        let progress = elapsed / COLLAPSE_SECONDS;
        // Accelerates inward while spinning half a turn.
        let radius = size * IMPLOSION_RADIUS * (1.0 - progress * progress);
        let spin = progress * std::f32::consts::PI;
        for particle in 0..IMPLOSION_PARTICLES {
            let angle = particle as f32 / IMPLOSION_PARTICLES as f32 * std::f32::consts::TAU + spin;
            let color = if particle % 2 == 0 { color_a } else { color_b };
            draw_circle(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
                size * 0.25 * (1.0 - progress / 2.0),
                color,
            );
        }
        draw_circle_lines(center.x, center.y, radius, 3.0, ORANGE);
    } else if elapsed < IMPLOSION_SECONDS {
        let progress = (elapsed - COLLAPSE_SECONDS) / (IMPLOSION_SECONDS - COLLAPSE_SECONDS);
        draw_circle(
            center.x,
            center.y,
            size * 1.5 * progress,
            Color::new(1.0, 0.9, 0.4, 1.0 - progress),
        );
    }
}

fn draw_scores(game: &Game, origin: Vec2) {
    let mut x = origin.x + 8.0;
    for (player, (snake, (_, color))) in game.snakes().iter().zip(SNAKE_COLORS).enumerate() {
        let text = format!("P{}: {}", player + 1, snake.length());
        draw_text(&text, x, origin.y + FONT_SIZE, FONT_SIZE, color);
        x += measure_text(&text, None, FONT_SIZE as u16, 1.0).width + 24.0;
    }
}

fn draw_result(status: Status) {
    let headline = match status {
        Status::Running => return,
        Status::Winner(player) => format!("Player {} wins!", player + 1),
        Status::Draw => "Draw!".to_string(),
    };
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.6),
    );
    let center_y = screen_height() / 2.0;
    draw_centered_text(&headline, center_y, FONT_SIZE * 2.0);
    draw_centered_text(
        "Press Space to restart",
        center_y + FONT_SIZE * 1.5,
        FONT_SIZE,
    );
}

fn draw_centered_text(text: &str, y: f32, size: f32) {
    let width = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, (screen_width() - width) / 2.0, y, size, WHITE);
}

fn cell_corner((x, y): Point, origin: Vec2, size: f32) -> Vec2 {
    origin + vec2(f32::from(x), f32::from(y)) * size
}

fn draw_cell(cell: Point, origin: Vec2, size: f32, color: Color) {
    let corner = cell_corner(cell, origin, size);
    draw_rectangle(corner.x, corner.y, size, size, color);
}
