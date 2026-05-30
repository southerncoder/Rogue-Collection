//! Loader for hand-authored fixed maps.

use super::defs::{resolve_char, FixedLevel, SpawnSpec};
use crate::data::Config;
use crate::geometry::Point;
use crate::map::{Map, Spawn, SpawnKind, TileKind};
use crate::rng::RogueRng;

/// Build a [`Map`] from a hand-authored [`FixedLevel`].
///
/// The map is sized to the configured dimensions; rows are placed from the top
/// left. Gold values of `0` are rolled by depth, matching procedural levels.
pub fn load_fixed(level: &FixedLevel, cfg: &Config, depth: i32, rng: &mut impl RogueRng) -> Map {
    let width = cfg.map_width.max(
        level
            .rows
            .iter()
            .map(|r| r.chars().count() as i32)
            .max()
            .unwrap_or(0),
    );
    let height = cfg.map_height.max(level.rows.len() as i32);
    let mut map = Map::new(width, height);

    let mut player_start: Option<Point> = None;

    for (y, row) in level.rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let p = Point::new(x as i32, y as i32);
            let custom = level.legend.iter().find(|e| e.ch == ch);
            let custom_spawn = custom.and_then(|e| e.spawn.as_ref());

            let is_player = ch == '@' || matches!(custom_spawn, Some(SpawnSpec::Player));
            let is_stairs = ch == '>' || matches!(custom_spawn, Some(SpawnSpec::StairsDown));

            let (mut tile, spawn) = resolve_char(ch, &level.legend);

            if is_stairs {
                tile = TileKind::StairsDown;
                map.stairs_down = Some(p);
            }
            if is_player {
                tile = TileKind::Floor;
                player_start = Some(p);
            }

            map.set_tile(p, tile);
            if level.dark {
                map.set_dark(p, true);
            }

            // Skip placeholder spawns for player / stairs markers.
            if is_player || is_stairs {
                continue;
            }
            if let Some(kind) = spawn {
                let kind = match kind {
                    SpawnKind::Gold(0) => SpawnKind::Gold(rng.rnd(50 + 10 * depth) + 2),
                    other => other,
                };
                map.spawns.push(Spawn { pos: p, kind });
            }
        }
    }

    if let Some(p) = player_start {
        map.player_start = p;
    } else if let Some(p) = first_floor(&map) {
        map.player_start = p;
    }

    map
}

fn first_floor(map: &Map) -> Option<Point> {
    for y in 0..map.height {
        for x in 0..map.width {
            let p = Point::new(x, y);
            if map.tile(p) == TileKind::Floor {
                return Some(p);
            }
        }
    }
    None
}
