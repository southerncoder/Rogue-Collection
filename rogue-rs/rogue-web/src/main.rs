//! macroquad frontend for rogue-engine — runs natively and in the browser via WASM.
//!
//! Build native:    `cargo run -p rogue-web`
//! Build WASM:      `cargo build -p rogue-web --target wasm32-unknown-unknown`
//! Serve (any):     serve `rogue-web/web/` with a static-file server and open index.html

use macroquad::prelude::*;
use rogue_engine::framebuffer::FrameBuffer;
use rogue_engine::game::{Game, SCREEN_HEIGHT, SCREEN_WIDTH};

mod keys;

/// How many screen pixels each cell occupies.
const CELL_W: f32 = 10.0;
const CELL_H: f32 = 16.0;

#[macroquad::main(window_conf)]
async fn main() {
    let mut game = Game::new();
    loop {
        let action = keys::poll_action();
        let fb = game.tick(action);
        clear_background(BLACK);
        render_framebuffer(fb);
        if fb.wants_quit {
            break;
        }
        next_frame().await;
    }
}

/// Blit the framebuffer to the macroquad canvas using the default font.
fn render_framebuffer(fb: &FrameBuffer) {
    for y in 0..fb.height {
        for x in 0..fb.width {
            let cell = &fb.cells[(y * fb.width + x) as usize];
            let sx = x as f32 * CELL_W;
            let sy = y as f32 * CELL_H;
            // Background rect
            draw_rectangle(sx, sy, CELL_W, CELL_H, color_of(cell.bg));
            // Character
            let ch = match cell.cp437 {
                Some(code) => cp437_to_char(code),
                None       => cell.glyph,
            };
            if ch != ' ' {
                draw_text(
                    &ch.to_string(),
                    sx,
                    sy + CELL_H - 2.0,
                    CELL_H,
                    color_of(cell.fg),
                );
            }
        }
    }
}

fn color_of(c: (u8, u8, u8)) -> Color {
    Color::from_rgba(c.0, c.1, c.2, 255)
}

/// Minimal CP437 → Unicode mapping for the glyphs we actually emit.
/// Full ASCII 0-127 is identity; we only need a handful of extended chars.
fn cp437_to_char(cp: u16) -> char {
    // Standard ASCII range — CP437 is identity for 0x20-0x7E.
    if cp < 128 {
        return cp as u8 as char;
    }
    // For sprite indices 128+ (tiled theme) we show a placeholder because
    // macroquad renders text with a Unicode font rather than a CP437 bitmap.
    '?'
}

/// Compute the total pixel size needed for the window.
pub fn window_conf() -> Conf {
    Conf {
        window_title: "Rogue-rs".to_string(),
        window_width:  (SCREEN_WIDTH  as f32 * CELL_W) as i32,
        window_height: (SCREEN_HEIGHT as f32 * CELL_H) as i32,
        ..Default::default()
    }
}
