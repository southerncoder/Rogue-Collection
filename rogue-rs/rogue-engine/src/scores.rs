//! Persistent high-score table.
//!
//! On native targets the table is stored as JSON next to the game binary.
//! On WASM the file-system operations are compiled out and `load` always
//! returns an empty table; `save` is a no-op.

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreEntry {
    pub name: String,
    pub depth: i32,
    pub gold: i32,
    pub turns: u64,
    pub won: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Scores {
    pub entries: Vec<ScoreEntry>,
}

impl Scores {
    #[cfg(not(target_arch = "wasm32"))]
    fn path() -> PathBuf {
        PathBuf::from("rogue_scores.json")
    }

    pub fn load() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::fs::read_to_string(Self::path())
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self::default()
        }
    }

    pub fn save(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(Self::path(), json);
            }
        }
    }

    pub fn add(&mut self, entry: ScoreEntry) {
        self.entries.push(entry);
        self.entries
            .sort_by(|a, b| b.depth.cmp(&a.depth).then(b.gold.cmp(&a.gold)));
        self.entries.truncate(20);
        self.save();
    }

    /// Return the top `n` entries formatted as display strings.
    pub fn top_lines(&self, n: usize) -> Vec<String> {
        self.entries
            .iter()
            .take(n)
            .enumerate()
            .map(|(i, e)| {
                let outcome = if e.won { "WON" } else { "died" };
                format!(
                    "{:2}. {:16} depth:{:3}  gold:{:5}  turns:{:6}  {}",
                    i + 1,
                    e.name,
                    e.depth,
                    e.gold,
                    e.turns,
                    outcome
                )
            })
            .collect()
    }
}
