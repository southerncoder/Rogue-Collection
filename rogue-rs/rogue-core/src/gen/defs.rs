//! Serializable level/dungeon definitions used to drive generation.

use crate::map::{SpawnKind, TileKind};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn yes() -> bool {
    true
}

/// Parameters for a procedurally generated level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcParams {
    /// Override the depth used for difficulty scaling (defaults to the real depth).
    #[serde(default)]
    pub depth: Option<i32>,
    /// Allow rooms to be dark at sufficient depth.
    #[serde(default = "yes")]
    pub allow_dark: bool,
    /// Allow some rooms to be replaced by corridor junctions ("gone" rooms).
    #[serde(default = "yes")]
    pub allow_gone_rooms: bool,
}

impl Default for ProcParams {
    fn default() -> Self {
        ProcParams {
            depth: None,
            allow_dark: true,
            allow_gone_rooms: true,
        }
    }
}

/// A tile that a legend character maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileSpec {
    Empty,
    Floor,
    Wall,
    Passage,
    Door,
    StairsDown,
    StairsUp,
    Trap,
}

impl From<TileSpec> for TileKind {
    fn from(t: TileSpec) -> Self {
        match t {
            TileSpec::Empty => TileKind::Empty,
            TileSpec::Floor => TileKind::Floor,
            TileSpec::Wall => TileKind::Wall,
            TileSpec::Passage => TileKind::Passage,
            TileSpec::Door => TileKind::Door,
            TileSpec::StairsDown => TileKind::StairsDown,
            TileSpec::StairsUp => TileKind::StairsUp,
            TileSpec::Trap => TileKind::Trap,
        }
    }
}

/// What a legend character spawns, in addition to (or instead of) a tile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpawnSpec {
    /// A monster by glyph, or `None` to pick a random depth-appropriate one.
    Monster(Option<char>),
    /// A random floor item.
    Item,
    /// Gold of an explicit value (or `None` to roll by depth).
    Gold(Option<i32>),
    /// The player's starting position.
    Player,
    /// The down staircase.
    StairsDown,
}

/// A single character mapping for a fixed map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegendEntry {
    pub ch: char,
    #[serde(default)]
    pub tile: Option<TileSpec>,
    #[serde(default)]
    pub spawn: Option<SpawnSpec>,
}

/// A hand-authored, fixed dungeon level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedLevel {
    /// Rows of map text, top to bottom.
    pub rows: Vec<String>,
    /// Optional extra/overriding character mappings.
    #[serde(default)]
    pub legend: Vec<LegendEntry>,
    /// If true, the whole level starts dark (revealed only as explored).
    #[serde(default)]
    pub dark: bool,
}

/// One level's definition (in-memory, fully resolved).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelDef {
    Procedural(ProcParams),
    Fixed(FixedLevel),
}

/// A reference to a level inside a dungeon file: either inline or an external file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelRef {
    Procedural(ProcParams),
    Fixed(FixedLevel),
    /// Load the level from `levels/<name>` relative to the dungeon file.
    File(String),
}

/// The on-disk `dungeon.ron` description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DungeonFile {
    pub name: String,
    pub levels: Vec<LevelRef>,
    #[serde(default = "yes")]
    pub repeat_last: bool,
}

/// A fully resolved dungeon ready for generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dungeon {
    pub name: String,
    pub levels: Vec<LevelDef>,
    pub repeat_last: bool,
}

// Dungeon + level files embedded at compile time so they are available on
// WASM (which has no filesystem access) and as a fallback on native.
const DUNGEON_RON:  &str = include_str!("../../../assets/dungeon.ron");
const LEVEL_ROGUE:    &str = include_str!("../../../assets/levels/rogue.ron");
const LEVEL_ENTRANCE: &str = include_str!("../../../assets/levels/entrance.ron");
const LEVEL_VAULT:    &str = include_str!("../../../assets/levels/vault.ron");

fn bundled_level_text(name: &str) -> Option<&'static str> {
    match name {
        "rogue.ron"    => Some(LEVEL_ROGUE),
        "entrance.ron" => Some(LEVEL_ENTRANCE),
        "vault.ron"    => Some(LEVEL_VAULT),
        _              => None,
    }
}

impl Dungeon {
    /// A purely procedural dungeon of `depth` levels (used as the default).
    pub fn default_procedural(depth: i32) -> Dungeon {
        Dungeon {
            name: "The Dungeons of Doom".to_string(),
            levels: (0..depth)
                .map(|_| LevelDef::Procedural(ProcParams::default()))
                .collect(),
            repeat_last: true,
        }
    }

    /// Parse the dungeon from assets embedded in the binary at compile time.
    /// This is the only source available on WASM and a fallback on native.
    pub fn bundled() -> Dungeon {
        let file: DungeonFile = ron::from_str(DUNGEON_RON)
            .expect("bundled dungeon.ron must parse");
        let mut levels = Vec::with_capacity(file.levels.len());
        for r in file.levels {
            let def = match r {
                LevelRef::Procedural(p) => LevelDef::Procedural(p),
                LevelRef::Fixed(f)      => LevelDef::Fixed(f),
                LevelRef::File(ref name) => {
                    if let Some(text) = bundled_level_text(name) {
                        ron::from_str::<LevelDef>(text)
                            .unwrap_or(LevelDef::Procedural(ProcParams::default()))
                    } else {
                        LevelDef::Procedural(ProcParams::default())
                    }
                }
            };
            levels.push(def);
        }
        Dungeon { name: file.name, levels, repeat_last: file.repeat_last }
    }

    /// Resolve a `dungeon.ron` in `dir`, loading any `File` references from
    /// `dir/levels/`. Returns `None` if the file is missing or invalid.
    pub fn load(dir: impl AsRef<Path>) -> Option<Dungeon> {
        let dir = dir.as_ref();
        let text = std::fs::read_to_string(dir.join("dungeon.ron")).ok()?;
        let file: DungeonFile = ron::from_str(&text).ok()?;
        Dungeon::from_file(file, dir).ok()
    }

    /// Resolve a [`DungeonFile`], loading any `File` references from `base_dir`.
    pub fn from_file(file: DungeonFile, base_dir: impl AsRef<Path>) -> Result<Dungeon, String> {
        let base = base_dir.as_ref();
        let mut levels = Vec::with_capacity(file.levels.len());
        for r in file.levels {
            let def = match r {
                LevelRef::Procedural(p) => LevelDef::Procedural(p),
                LevelRef::Fixed(f) => LevelDef::Fixed(f),
                LevelRef::File(name) => {
                    let path = base.join("levels").join(&name);
                    let text = std::fs::read_to_string(&path)
                        .map_err(|e| format!("reading {}: {e}", path.display()))?;
                    ron::from_str::<LevelDef>(&text)
                        .map_err(|e| format!("parsing {}: {e}", path.display()))?
                }
            };
            levels.push(def);
        }
        Ok(Dungeon {
            name: file.name,
            levels,
            repeat_last: file.repeat_last,
        })
    }
}

/// Resolve a legend character to a tile and/or spawn using built-in defaults
/// plus any custom entries (custom entries win).
pub(crate) fn resolve_char(
    ch: char,
    legend: &[LegendEntry],
) -> (TileKind, Option<SpawnKind>) {
    if let Some(e) = legend.iter().find(|e| e.ch == ch) {
        let tile = e.tile.map(TileKind::from).unwrap_or(TileKind::Floor);
        let spawn = e.spawn.as_ref().map(spawn_from_spec);
        return (tile, spawn);
    }
    default_char(ch)
}

fn spawn_from_spec(s: &SpawnSpec) -> SpawnKind {
    match s {
        SpawnSpec::Monster(m) => SpawnKind::Monster(*m),
        SpawnSpec::Item => SpawnKind::Item,
        SpawnSpec::Gold(v) => SpawnKind::Gold(v.unwrap_or(0)),
        // Player / StairsDown are handled by the loader as map metadata, but we
        // still return a placeholder so the tile becomes floor/stairs.
        SpawnSpec::Player => SpawnKind::Item,
        SpawnSpec::StairsDown => SpawnKind::Item,
    }
}

/// The built-in fixed-map character vocabulary.
fn default_char(ch: char) -> (TileKind, Option<SpawnKind>) {
    match ch {
        ' ' => (TileKind::Empty, None),
        '.' => (TileKind::Floor, None),
        '#' => (TileKind::Passage, None),
        '|' | '-' => (TileKind::Wall, None),
        '+' => (TileKind::Door, None),
        '>' => (TileKind::StairsDown, None),
        '<' => (TileKind::StairsUp, None),
        '^' => (TileKind::Trap, None),
        '@' => (TileKind::Floor, None), // player start, handled specially
        '$' => (TileKind::Floor, Some(SpawnKind::Gold(0))),
        '*' => (TileKind::Floor, Some(SpawnKind::Item)),
        'A'..='Z' => (TileKind::Floor, Some(SpawnKind::Monster(Some(ch)))),
        _ => (TileKind::Empty, None),
    }
}
