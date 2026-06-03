//! bracket-lib terminal frontend for the modern Rust Rogue.

use bracket_lib::prelude::*;

use rogue_cli::game::{Game, SCREEN_HEIGHT, SCREEN_WIDTH};
use rogue_cli::theme::Theme;

fn main() -> BError {
    // The default 8x8 font makes the window tiny on modern displays. Render each
    // tile at 16x16 px so the window opens at a comfortable size (and set an
    // explicit pixel size to match: 80*16 x 28*16).
    let context = BTermBuilder::simple(SCREEN_WIDTH, SCREEN_HEIGHT)?
        .with_title("Rogue — Rust edition")
        .with_tile_dimensions(16, 16)
        .with_dimensions(SCREEN_WIDTH, SCREEN_HEIGHT)
        .build()?;

    let mut game = Game::new();

    let args: Vec<String> = std::env::args().collect();
    // `--demo` boots straight into a watchable autopilot run.
    if args.iter().any(|a| a == "--demo") {
        game.start_demo();
    }
    // `--theme <name>` selects a color theme (classic / amber / green).
    if let Some(pos) = args.iter().position(|a| a == "--theme") {
        if let Some(name) = args.get(pos + 1) {
            game.theme = Theme::from_name(name);
        }
    }

    main_loop(context, game)
}
