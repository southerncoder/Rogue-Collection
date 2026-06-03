//! bracket-lib terminal frontend for the modern Rust Rogue.

use bracket_lib::prelude::*;

use rogue_cli::game::{Game, SCREEN_HEIGHT, SCREEN_WIDTH};
use rogue_cli::theme::Theme;

embedded_resource!(TILE_FONT, "../resources/rogue_tiles.png");

fn main() -> BError {
    let args: Vec<String> = std::env::args().collect();
    let theme_name = args.iter().position(|a| a == "--theme")
        .and_then(|pos| args.get(pos + 1))
        .map(|s| s.as_str())
        .unwrap_or("classic");

    // Register the tiled font from embedded bytes so it works regardless of CWD.
    link_resource!(TILE_FONT, "resources/rogue_tiles.png");

    let context = if theme_name == "tiled" {
        // 16×32 pixel tiles: window is 80×28 tiles = 1280×896 px.
        BTermBuilder::new()
            .with_title("Rogue — Rust edition")
            .with_resource_path("resources/")
            .with_font("rogue_tiles.png", 16, 32)
            .with_simple_console(SCREEN_WIDTH, SCREEN_HEIGHT, "rogue_tiles.png")
            .with_dimensions(SCREEN_WIDTH, SCREEN_HEIGHT)
            .with_tile_dimensions(16, 32)
            .build()?
    } else {
        // The default 8×8 font makes the window tiny on modern displays.  Render
        // each tile at 16×16 px so the window opens at a comfortable size.
        BTermBuilder::simple(SCREEN_WIDTH, SCREEN_HEIGHT)?
            .with_title("Rogue — Rust edition")
            .with_tile_dimensions(16, 16)
            .with_dimensions(SCREEN_WIDTH, SCREEN_HEIGHT)
            .build()?
    };

    let mut game = Game::new();

    // `--demo` boots straight into a watchable autopilot run.
    if args.iter().any(|a| a == "--demo") {
        game.start_demo();
    }
    game.theme = Theme::from_name(theme_name);

    main_loop(context, game)
}
