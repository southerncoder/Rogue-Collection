//! rogue-core: engine-agnostic Rogue rules, map, entities, generation and data.
//!
//! This crate contains everything that does not depend on a specific rendering
//! backend so it can be reused by the bracket-lib frontend, tests, or a future
//! WASM/web build.

pub mod combat;
pub mod data;
pub mod dice;
pub mod gen;
pub mod geometry;
pub mod map;
pub mod rng;

pub use combat::{roll_attack, Attacker};
pub use data::GameData;
pub use dice::{DamageRoll, Dice};
pub use gen::{Dungeon, LevelDef};
pub use geometry::{Point, Rect};
pub use map::{Map, Spawn, SpawnKind, TileKind};
pub use rng::RogueRng;
