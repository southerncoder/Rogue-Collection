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
            // Quality gate: the stairs must be reachable from the start, or the
            // level is unwinnable (the bug the player hit on level 1).
            assert!(
                map.reachable(map.player_start, map.stairs_down.unwrap()),
                "depth {depth}: stairs unreachable from player start"
            );
        }
    }

    #[test]
    fn fixed_entrance_is_fully_connected() {
        // Every authored thing (monsters, gold, items) and the stairs must be
        // reachable from the player start — guards against off-by-one corridors.
        let cfg = GameData::bundled().unwrap().config;
        let text = std::fs::read_to_string(assets_dir().join("levels/entrance.ron")).unwrap();
        let LevelDef::Fixed(level) = ron::from_str::<LevelDef>(&text).unwrap() else {
            panic!("entrance should be a fixed level");
        };
        let mut rng = StdRng::seed_from_u64(7);
        let map = load_fixed(&level, &cfg, 1, &mut rng);
        let start = map.player_start;
        assert!(
            map.reachable(start, map.stairs_down.unwrap()),
            "stairs unreachable from start"
        );
        for s in &map.spawns {
            assert!(
                map.reachable(start, s.pos),
                "authored spawn at {:?} ({:?}) is unreachable",
                s.pos,
                s.kind
            );
        }
        // Rooms should have been detected for whole-room lighting.
        assert!(map.rooms.len() >= 3, "expected detected rooms, got {}", map.rooms.len());
    }

    #[test]
    fn fixed_levels_have_uniform_row_widths() {
        // Ragged rows shift the right wall and trailing tiles off by one
        // (the loader pads to the widest row), so every authored row in a
        // fixed level must be exactly the same length.
        for name in ["entrance.ron", "vault.ron"] {
            let text = std::fs::read_to_string(assets_dir().join("levels").join(name)).unwrap();
            let LevelDef::Fixed(level) = ron::from_str::<LevelDef>(&text).unwrap() else {
                panic!("{name} should be a fixed level");
            };
            let width = level.rows.first().map(|r| r.chars().count()).unwrap_or(0);
            for (i, row) in level.rows.iter().enumerate() {
                assert_eq!(
                    row.chars().count(),
                    width,
                    "{name} row {i} is {} wide, expected {width}",
                    row.chars().count()
                );
            }
        }
    }

    #[test]
    fn fixed_vault_spawns_are_reachable() {
        let cfg = GameData::bundled().unwrap().config;
        let text = std::fs::read_to_string(assets_dir().join("levels/vault.ron")).unwrap();
        let LevelDef::Fixed(level) = ron::from_str::<LevelDef>(&text).unwrap() else {
            panic!("vault should be a fixed level");
        };
        let mut rng = StdRng::seed_from_u64(3);
        let map = load_fixed(&level, &cfg, 2, &mut rng);
        let start = map.player_start;
        assert!(map.stairs_down.is_some(), "vault must have stairs");
        let gold = map
            .spawns
            .iter()
            .filter(|s| matches!(s.kind, crate::map::SpawnKind::Gold(_)))
            .count();
        assert!(gold >= 8, "expected authored gold piles, got {gold}");
        for s in &map.spawns {
            assert!(
                map.reachable(start, s.pos),
                "vault spawn at {:?} ({:?}) is unreachable",
                s.pos,
                s.kind
            );
        }
    }
}
