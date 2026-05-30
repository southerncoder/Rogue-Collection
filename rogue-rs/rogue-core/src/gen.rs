//! Dungeon generation: faithful procedural layouts plus hand-authored fixed maps.
//!
//! Two ways to define a level (see [`LevelDef`]):
//! * [`LevelDef::Procedural`] — generated using the classic Rogue v5.4.2 algorithm
//!   (3x3 grid of rooms, MST-connected L-shaped corridors, dark/gone rooms, traps,
//!   gold, items and a down staircase).
//! * [`LevelDef::Fixed`] — an ASCII map loaded verbatim, with a character legend.
//!
//! A whole dungeon is an ordered list of level defs ([`Dungeon`]). Adding a level
//! is just adding an entry — no code changes required.

mod defs;
mod fixed;
mod procedural;

pub use defs::{Dungeon, DungeonFile, FixedLevel, LegendEntry, LevelDef, ProcParams};
pub use fixed::load_fixed;
pub use procedural::generate_procedural;

use crate::data::Config;
use crate::map::Map;
use crate::rng::RogueRng;

impl Dungeon {
    /// Build the map for a given dungeon depth (1-based).
    ///
    /// If the depth is beyond the explicitly defined levels and `repeat_last` is
    /// set, the trailing procedural parameters are reused so the dungeon can go
    /// arbitrarily deep.
    pub fn build_level(&self, depth: i32, cfg: &Config, rng: &mut impl RogueRng) -> Map {
        let def = self.def_for_depth(depth);
        match def {
            LevelDef::Procedural(params) => {
                let effective_depth = params.depth.unwrap_or(depth);
                generate_procedural(cfg, effective_depth, params, rng)
            }
            LevelDef::Fixed(level) => load_fixed(level, cfg, depth, rng),
        }
    }

    fn def_for_depth(&self, depth: i32) -> &LevelDef {
        let idx = (depth - 1).max(0) as usize;
        if idx < self.levels.len() {
            &self.levels[idx]
        } else if self.repeat_last {
            // Find the last procedural def to repeat; fall back to a default one.
            self.levels
                .iter()
                .rev()
                .find(|d| matches!(d, LevelDef::Procedural(_)))
                .unwrap_or(&DEFAULT_PROC)
        } else {
            self.levels.last().unwrap_or(&DEFAULT_PROC)
        }
    }
}

static DEFAULT_PROC: LevelDef = LevelDef::Procedural(ProcParams {
    depth: None,
    allow_dark: true,
    allow_gone_rooms: true,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets")
    }

    #[test]
    fn dungeon_file_parses_and_resolves() {
        let text = std::fs::read_to_string(assets_dir().join("dungeon.ron")).unwrap();
        let file: DungeonFile = ron::from_str(&text).expect("dungeon.ron should parse");
        // `Procedural(())` must deserialize to defaults.
        let dungeon = Dungeon::from_file(file, assets_dir()).expect("resolve File refs");
        assert!(dungeon.levels.len() >= 4);
        // First level is the hand-authored entrance.
        assert!(matches!(dungeon.levels[0], LevelDef::Fixed(_)));
    }

    #[test]
    fn builds_every_defined_and_repeated_level() {
        let cfg = GameData::bundled().unwrap().config;
        let text = std::fs::read_to_string(assets_dir().join("dungeon.ron")).unwrap();
        let file: DungeonFile = ron::from_str(&text).unwrap();
        let dungeon = Dungeon::from_file(file, assets_dir()).unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        // Build several levels including some past the defined list (repeat_last).
        for depth in 1..=8 {
            let map = dungeon.build_level(depth, &cfg, &mut rng);
            assert!(map.stairs_down.is_some(), "depth {depth} has no stairs");
            assert!(map.is_walkable(map.player_start));
        }
    }

    #[test]
    fn fixed_entrance_has_player_stairs_and_monsters() {
        let cfg = GameData::bundled().unwrap().config;
        let text = std::fs::read_to_string(assets_dir().join("levels/entrance.ron")).unwrap();
        let def: LevelDef = ron::from_str(&text).unwrap();
        let LevelDef::Fixed(level) = def else {
            panic!("entrance should be a fixed level");
        };
        let mut rng = StdRng::seed_from_u64(1);
        let map = load_fixed(&level, &cfg, 1, &mut rng);
        assert!(map.stairs_down.is_some(), "entrance must have stairs");
        assert_eq!(map.tile(map.player_start), crate::map::TileKind::Floor);
        // K, B, H monsters plus gold and an item are authored in.
        let monsters = map
            .spawns
            .iter()
            .filter(|s| matches!(s.kind, crate::map::SpawnKind::Monster(_)))
            .count();
        assert!(monsters >= 3, "expected authored monsters, got {monsters}");
    }
}
