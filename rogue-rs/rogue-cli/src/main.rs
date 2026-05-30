//! bracket-lib terminal frontend for the modern Rust Rogue.

use bracket_lib::prelude::*;

mod components;
mod game;

use game::{Game, SCREEN_HEIGHT, SCREEN_WIDTH};

fn main() -> BError {
    let context = BTermBuilder::simple(SCREEN_WIDTH, SCREEN_HEIGHT)?
        .with_title("Rogue — Rust edition")
        .build()?;
    main_loop(context, Game::new())
}
