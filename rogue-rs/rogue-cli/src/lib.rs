//! Library surface for the bracket-lib Rogue frontend.
//!
//! The playable binary lives in `main.rs`; exposing the game as a library lets
//! integration tests (and the headless autopilot bot) drive it without opening
//! a window.

pub mod components;
pub mod game;
pub mod scores;
