//! Data-driven game content: monsters, item tables, and tunable config.
//!
//! All values derive `serde` so the content lives in RON files under
//! `rogue-rs/assets/`. The canonical numbers are lifted verbatim from the legacy
//! Unix Rogue v5.4.2 tables (`legacy/src/RogueVersions/Rogue_5_4_2/extern.c` and
//! `weapons.c`). The default content is embedded in the binary via `include_str!`
//! so the game runs with no external files, while [`GameData::load_from_dir`]
//! allows overriding from disk.

use crate::dice::DamageRoll;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Behavioural flags for a monster (mirrors the legacy `IS*` bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterFlag {
    /// Attacks the player on sight.
    Mean,
    /// Flies (moves erratically / over some terrain).
    Fly,
    /// Regenerates hit points over time.
    Regen,
    /// Steals gold / is greedy for treasure.
    Greed,
    /// Invisible unless the player can see invisible.
    Invis,
}

impl MonsterFlag {
    pub fn is(flags: &[MonsterFlag], f: MonsterFlag) -> bool {
        flags.contains(&f)
    }
}

/// A monster archetype (one of the 26 letters `A`..=`Z` in classic Rogue).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonsterDef {
    pub name: String,
    /// Glyph used on the map.
    pub symbol: char,
    /// Percent chance to be carrying treasure when killed.
    pub carry: i32,
    pub flags: Vec<MonsterFlag>,
    /// Base strength.
    pub strength: i32,
    /// Experience awarded for the kill.
    pub experience: i32,
    /// Monster level (drives hit-point dice: `level d8`).
    pub level: i32,
    /// Armor class — lower is harder to hit, matching Rogue.
    pub armor: i32,
    /// Attack damage, possibly multiple attacks separated by `/`.
    pub damage: DamageRoll,
}

/// The seven kinds of item that can appear in the dungeon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemCategory {
    Potion,
    Scroll,
    Food,
    Weapon,
    Armor,
    Ring,
    Stick,
}

/// Relative spawn weight for one of the seven top-level item categories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryProb {
    pub category: ItemCategory,
    pub prob: i32,
}

/// A simple named item with a spawn weight and gold value (potions, scrolls,
/// rings, wands/staves).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedItem {
    pub name: String,
    pub prob: i32,
    pub worth: i32,
}

/// An armor type. `base_ac` is the unenchanted armor class (lower is better).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmorDef {
    pub name: String,
    pub prob: i32,
    pub worth: i32,
    pub base_ac: i32,
}

/// A weapon type with melee and thrown damage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaponDef {
    pub name: String,
    pub prob: i32,
    pub worth: i32,
    /// Damage when wielded in melee.
    pub damage: DamageRoll,
    /// Damage when thrown.
    pub throw_damage: DamageRoll,
    /// Name of the weapon that launches this one (e.g. arrow -> "short bow").
    #[serde(default)]
    pub launched_by: Option<String>,
    /// True for stackable ammunition / thrown weapons (arrows, darts...).
    #[serde(default)]
    pub stacks: bool,
}

/// All item tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemTables {
    pub categories: Vec<CategoryProb>,
    pub potions: Vec<NamedItem>,
    pub scrolls: Vec<NamedItem>,
    pub rings: Vec<NamedItem>,
    pub sticks: Vec<NamedItem>,
    pub armor: Vec<ArmorDef>,
    pub weapons: Vec<WeaponDef>,
}

/// Tunable game constants (lifted from `rogue.h`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub map_width: i32,
    pub map_height: i32,
    pub max_rooms: i32,
    pub max_passages: i32,
    pub max_objects_per_level: i32,
    pub max_traps: i32,
    /// Depth at which the Amulet of Yendor appears.
    pub amulet_level: i32,
    /// Turns of food a ration provides.
    pub hunger_time: i32,
    /// Maximum food the stomach can hold.
    pub stomach_size: i32,
    /// Player starting hit points.
    pub start_hp: i32,
    /// Player starting strength.
    pub start_strength: i32,
    /// Player starting armor class (internal value; lower is better).
    #[serde(default = "default_start_armor")]
    pub start_armor: i32,
}

fn default_start_armor() -> i32 {
    7
}

/// The complete bundle of game content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameData {
    pub config: Config,
    pub monsters: Vec<MonsterDef>,
    pub items: ItemTables,
}

/// Wrapper used for the embedded/loaded `monsters.ron` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MonstersFile {
    monsters: Vec<MonsterDef>,
}

const CONFIG_RON: &str = include_str!("../../assets/config.ron");
const MONSTERS_RON: &str = include_str!("../../assets/monsters.ron");
const ITEMS_RON: &str = include_str!("../../assets/items.ron");

/// Errors that can occur while loading game data.
#[derive(Debug)]
pub enum DataError {
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataError::Io(e) => write!(f, "io error: {e}"),
            DataError::Parse(e) => write!(f, "parse error: {e}"),
        }
    }
}

impl std::error::Error for DataError {}

impl From<std::io::Error> for DataError {
    fn from(e: std::io::Error) -> Self {
        DataError::Io(e)
    }
}

impl From<ron::error::SpannedError> for DataError {
    fn from(e: ron::error::SpannedError) -> Self {
        DataError::Parse(e.to_string())
    }
}

impl GameData {
    /// Parse the content embedded in the binary at build time.
    pub fn bundled() -> Result<GameData, DataError> {
        let config: Config = ron::from_str(CONFIG_RON)?;
        let monsters: MonstersFile = ron::from_str(MONSTERS_RON)?;
        let items: ItemTables = ron::from_str(ITEMS_RON)?;
        Ok(GameData {
            config,
            monsters: monsters.monsters,
            items,
        })
    }

    /// Load content from a directory containing `config.ron`, `monsters.ron`
    /// and `items.ron`, falling back to bundled files for any that are missing.
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<GameData, DataError> {
        let dir = dir.as_ref();
        let read = |name: &str, fallback: &str| -> Result<String, DataError> {
            let path = dir.join(name);
            if path.exists() {
                Ok(std::fs::read_to_string(path)?)
            } else {
                Ok(fallback.to_string())
            }
        };
        let config: Config = ron::from_str(&read("config.ron", CONFIG_RON)?)?;
        let monsters: MonstersFile = ron::from_str(&read("monsters.ron", MONSTERS_RON)?)?;
        let items: ItemTables = ron::from_str(&read("items.ron", ITEMS_RON)?)?;
        Ok(GameData {
            config,
            monsters: monsters.monsters,
            items,
        })
    }

    /// Look up a monster definition by its map glyph.
    pub fn monster_by_symbol(&self, symbol: char) -> Option<&MonsterDef> {
        self.monsters.iter().find(|m| m.symbol == symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> GameData {
        GameData::bundled().expect("bundled game data should parse")
    }

    #[test]
    fn bundled_parses() {
        let d = data();
        assert_eq!(d.monsters.len(), 26, "26 monsters A-Z");
        assert_eq!(d.config.map_width, 80);
        assert_eq!(d.config.map_height, 24);
    }

    #[test]
    fn monsters_cover_a_to_z() {
        let d = data();
        for c in 'A'..='Z' {
            assert!(
                d.monster_by_symbol(c).is_some(),
                "missing monster for symbol {c}"
            );
        }
    }

    #[test]
    fn dragon_is_canonical() {
        let d = data();
        let dragon = d.monster_by_symbol('D').unwrap();
        assert_eq!(dragon.name, "dragon");
        assert_eq!(dragon.experience, 5000);
        assert_eq!(dragon.armor, -1);
        // 1x8 + 1x8 + 3x10 attacks.
        assert_eq!(dragon.damage.terms().len(), 3);
    }

    #[test]
    fn item_tables_present() {
        let d = data();
        assert_eq!(d.items.potions.len(), 14);
        assert_eq!(d.items.scrolls.len(), 18);
        assert_eq!(d.items.rings.len(), 14);
        assert_eq!(d.items.sticks.len(), 14);
        assert_eq!(d.items.armor.len(), 8);
        assert_eq!(d.items.weapons.len(), 9);
        assert_eq!(d.items.categories.len(), 7);
    }

    /// Rogue's probability tables are each expressed so they sum to 100.
    #[test]
    fn probability_tables_sum_to_100() {
        let d = data();
        let sum_named = |v: &[NamedItem]| v.iter().map(|i| i.prob).sum::<i32>();
        assert_eq!(d.items.categories.iter().map(|c| c.prob).sum::<i32>(), 100);
        assert_eq!(sum_named(&d.items.potions), 100);
        assert_eq!(sum_named(&d.items.scrolls), 100);
        assert_eq!(sum_named(&d.items.rings), 100);
        assert_eq!(sum_named(&d.items.sticks), 100);
        assert_eq!(d.items.armor.iter().map(|a| a.prob).sum::<i32>(), 100);
        assert_eq!(d.items.weapons.iter().map(|w| w.prob).sum::<i32>(), 100);
    }
}
