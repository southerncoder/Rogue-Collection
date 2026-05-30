//! rogue-core: engine-agnostic Rogue rules, map, entities, generation and data.
//!
//! This crate contains everything that does not depend on a specific rendering
//! backend so it can be reused by the bracket-lib frontend, tests, or a future
//! WASM/web build.

pub mod data;
pub mod dice;
pub mod geometry;

pub use geometry::Point;
