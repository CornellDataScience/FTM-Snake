use apple_logic::{Point, game::Game};
use macroquad::prelude::*;

const BOARD_MARGIN: f32 = 10.0;

pub fn draw_game(game: &Game) {
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
    for cell in game.snake().segments() {
        draw_cell(cell, origin, cell_size, LIME);
    }
    draw_cell(game.snake().head(), origin, cell_size, DARKGREEN);
    for &cell in game.apples() {
        draw_cell(cell, origin, cell_size, RED);
    }
}

fn draw_cell((x, y): Point, origin: Vec2, size: f32, color: Color) {
    draw_rectangle(
        origin.x + f32::from(x) * size,
        origin.y + f32::from(y) * size,
        size,
        size,
        color,
    );
}
