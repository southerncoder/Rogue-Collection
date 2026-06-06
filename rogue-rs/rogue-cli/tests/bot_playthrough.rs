//! Integration test: the autopilot bot must actually be able to play through
//! the first (hand-authored) level and descend — proving the level the player
//! reported being "blocked" on is winnable.
//!
//! These tests drive the real game headlessly (no window): they load the real
//! `assets/` dungeon, then repeatedly call the public autopilot API.

use std::path::PathBuf;

use rogue_cli::game::Game;
use rogue_core::Dungeon;

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("assets")
}

fn load_real_dungeon() -> Dungeon {
    Dungeon::load(assets_dir()).expect("real assets/dungeon.ron should load")
}

#[test]
fn bot_descends_from_the_first_level() {
    // Deterministic seed so the run is reproducible in CI.
    let mut game = Game::with_dungeon_seeded(load_real_dungeon(), 12345);
    game.begin_playing();
    assert_eq!(game.depth(), 1, "should start on level 1");

    // Run the bot for a bounded number of turns; it should leave level 1.
    let mut turns = 0;
    while game.depth() < 2 && !game.is_dead() && !game.is_won() && turns < 5000 {
        if !game.auto_turn() {
            break; // no path / nothing to do
        }
        turns += 1;
    }

    assert!(
        game.depth() >= 2,
        "bot failed to descend from level 1 (depth={}, dead={}, won={}, turns={})",
        game.depth(),
        game.is_dead(),
        game.is_won(),
        turns
    );
}

#[test]
fn bot_makes_progress_across_several_levels() {
    let mut game = Game::with_dungeon_seeded(load_real_dungeon(), 2024);
    game.begin_playing();

    let mut turns = 0;
    let mut deepest = game.depth();
    while deepest < 4 && !game.is_dead() && !game.is_won() && turns < 20000 {
        if !game.auto_turn() {
            break;
        }
        deepest = deepest.max(game.depth());
        turns += 1;
    }

    // The bot should make real progress (several levels deep) before any death.
    assert!(
        deepest >= 3 || game.is_won(),
        "bot only reached depth {deepest} (dead={}, won={})",
        game.is_dead(),
        game.is_won()
    );
}
