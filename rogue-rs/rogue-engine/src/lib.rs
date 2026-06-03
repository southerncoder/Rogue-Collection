//! Platform-neutral game engine for the modern Rust Rogue reimplementation.
//!
//! This crate contains the complete game loop, state machine, ECS, rendering
//! abstraction, and supporting types.  It has **no** dependency on bracket-lib
//! or any other windowing/input library so it can compile to any target,
//! including wasm32-unknown-unknown.
//!
//! # Entry point
//! The primary surface is [`game::Game`]:
//!
//! ```rust,no_run
//! use rogue_engine::{game::Game, action::GameAction};
//!
//! let mut g = Game::new();
//! // Each tick: convert your platform's key event into an optional GameAction,
//! // then call tick() to advance the simulation and get the new frame.
//! let fb = g.tick(None);
//! ```

pub mod action;
pub mod components;
pub mod framebuffer;
pub mod game;
pub mod scores;
pub mod theme;
