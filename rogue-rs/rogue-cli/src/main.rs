//! bracket-lib terminal frontend for the modern Rust Rogue.

use bracket_lib::prelude::*;

mod components;
mod game;

use game::{Game, SCREEN_HEIGHT, SCREEN_WIDTH};

fn main() -> BError {
    // The default 8x8 font makes the window tiny on modern displays. Render each
    // tile at 16x16 px so the window opens at a comfortable size (and set an
    // explicit pixel size to match: 80*16 x 28*16).
    let context = BTermBuilder::simple(SCREEN_WIDTH, SCREEN_HEIGHT)?
        .with_title("Rogue — Rust edition")
        .with_tile_dimensions(16, 16)
        .with_dimensions(SCREEN_WIDTH, SCREEN_HEIGHT)
        .build()?;
    main_loop(context, Game::new())
}
