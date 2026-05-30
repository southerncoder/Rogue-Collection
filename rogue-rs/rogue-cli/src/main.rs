//! bracket-lib terminal frontend for the modern Rust Rogue.
//!
//! Phase 1 scaffold: opens an 80x24 terminal and renders a title screen so the
//! workspace is runnable end-to-end. The real game loop arrives in later phases.

use bracket_lib::prelude::*;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 25; // 24 map rows + 1 status line

struct State;

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        ctx.cls();
        ctx.print_centered(10, "Rogue (Rust edition)");
        ctx.print_centered(12, "Phase 1 scaffold — press Q to quit");
        if let Some(VirtualKeyCode::Q) = ctx.key {
            ctx.quitting = true;
        }
    }
}

fn main() -> BError {
    let context = BTermBuilder::simple(SCREEN_WIDTH, SCREEN_HEIGHT)?
        .with_title("Rogue — Rust edition")
        .build()?;
    main_loop(context, State)
}
