//! Faithful procedural level generation, ported from Rogue v5.4.2
//! (`rooms.c`, `passages.c`, `new_level.c`).

use super::defs::ProcParams;
use crate::data::Config;
use crate::geometry::{Point, Rect};
use crate::map::{Map, Spawn, SpawnKind, TileKind};
use crate::rng::RogueRng;

/// Hard-coded 3x3 room adjacency graph from `passages.c` (`rdes[].conn`).
/// Room indices are laid out row-major: 0,1,2 / 3,4,5 / 6,7,8.
const CONN: [[bool; 9]; 9] = [
    [false, true, false, true, false, false, false, false, false],
    [true, false, true, false, true, false, false, false, false],
    [false, true, false, false, false, true, false, false, false],
    [true, false, false, false, true, false, true, false, false],
    [false, true, false, true, false, true, false, true, false],
    [false, false, true, false, true, false, false, false, true],
    [false, false, false, true, false, false, false, true, false],
    [false, false, false, false, true, false, true, false, true],
    [false, false, false, false, false, true, false, true, false],
];

const MAXROOMS: usize = 9;

/// A room during generation. `gone` rooms are corridor junctions (a single cell).
#[derive(Clone, Copy)]
struct GenRoom {
    pos: Point,
    max: Point, // width/height for normal rooms
    gone: bool,
    dark: bool,
    gold: i32,
}

impl GenRoom {
    fn rect(&self) -> Rect {
        Rect::new(self.pos.x, self.pos.y, self.max.x, self.max.y)
    }
}

/// Generate a procedural level for the given depth.
pub fn generate_procedural(
    cfg: &Config,
    depth: i32,
    params: &ProcParams,
    rng: &mut impl RogueRng,
) -> Map {
    let mut map = Map::new(cfg.map_width, cfg.map_height);
    let bsze = Point::new(cfg.map_width / 3, cfg.map_height / 3);

    let mut rooms = do_rooms(&mut map, cfg, depth, params, bsze, rng);
    do_passages(&mut map, &rooms, rng);

    // Record real (non-gone) room rectangles for lighting.
    for r in &rooms {
        if !r.gone {
            map.rooms.push(r.rect());
        }
    }

    place_traps(&mut map, depth, cfg, rng);
    place_stairs(&mut map, rng);
    put_things(&mut map, depth, cfg, rng);
    place_monsters_and_gold(&mut map, &mut rooms, rng);

    if let Some(p) = random_floor(&map, rng) {
        map.player_start = p;
    }

    map
}

/// `do_rooms`: lay out and draw the nine rooms.
fn do_rooms(
    map: &mut Map,
    _cfg: &Config,
    depth: i32,
    params: &ProcParams,
    bsze: Point,
    rng: &mut impl RogueRng,
) -> Vec<GenRoom> {
    let mut rooms = vec![
        GenRoom {
            pos: Point::new(0, 0),
            max: Point::new(0, 0),
            gone: false,
            dark: false,
            gold: 0,
        };
        MAXROOMS
    ];

    // Mark up to three "gone" rooms.
    if params.allow_gone_rooms {
        let left_out = rng.rnd(4);
        for _ in 0..left_out {
            let r = rnd_room(&rooms, rng);
            rooms[r].gone = true;
        }
    }

    for i in 0..MAXROOMS {
        let top = Point::new((i % 3) as i32 * bsze.x + 1, (i / 3) as i32 * bsze.y);

        if rooms[i].gone {
            // Place a corridor junction, keeping the top row clear.
            loop {
                rooms[i].pos = Point::new(
                    top.x + rng.rnd(bsze.x - 2) + 1,
                    top.y + rng.rnd(bsze.y - 2) + 1,
                );
                if rooms[i].pos.y > 0 && rooms[i].pos.y < map.height - 1 {
                    break;
                }
            }
            continue;
        }

        // Dark rooms become more likely deeper down.
        if params.allow_dark && rng.rnd(10) < depth - 1 {
            rooms[i].dark = true;
        }

        // Random size and position within the room's grid cell.
        loop {
            rooms[i].max = Point::new(rng.rnd(bsze.x - 4) + 4, rng.rnd(bsze.y - 4) + 4);
            rooms[i].pos = Point::new(
                top.x + rng.rnd(bsze.x - rooms[i].max.x),
                top.y + rng.rnd(bsze.y - rooms[i].max.y),
            );
            if rooms[i].pos.y != 0 {
                break;
            }
        }

        draw_room(map, &rooms[i]);
    }

    rooms
}

/// `draw_room`: walls around the border, floor inside.
fn draw_room(map: &mut Map, rp: &GenRoom) {
    let x0 = rp.pos.x;
    let y0 = rp.pos.y;
    let x1 = rp.pos.x + rp.max.x - 1;
    let y1 = rp.pos.y + rp.max.y - 1;

    for x in x0..=x1 {
        map.set_tile(Point::new(x, y0), TileKind::Wall);
        map.set_tile(Point::new(x, y1), TileKind::Wall);
    }
    for y in y0..=y1 {
        map.set_tile(Point::new(x0, y), TileKind::Wall);
        map.set_tile(Point::new(x1, y), TileKind::Wall);
    }
    for y in (y0 + 1)..y1 {
        for x in (x0 + 1)..x1 {
            let p = Point::new(x, y);
            map.set_tile(p, TileKind::Floor);
            if rp.dark {
                map.set_dark(p, true);
            }
        }
    }
}

/// Pick a random room that is not "gone".
fn rnd_room(rooms: &[GenRoom], rng: &mut impl RogueRng) -> usize {
    loop {
        let r = rng.rnd(MAXROOMS as i32) as usize;
        if !rooms[r].gone {
            return r;
        }
    }
}

/// `do_passages`: connect rooms with a spanning tree plus a few extra loops.
fn do_passages(map: &mut Map, rooms: &[GenRoom], rng: &mut impl RogueRng) {
    let mut ingraph = [false; MAXROOMS];
    let mut isconn = [[false; MAXROOMS]; MAXROOMS];

    let mut roomcount = 1;
    let mut r1 = rng.rnd(MAXROOMS as i32) as usize;
    ingraph[r1] = true;

    while roomcount < MAXROOMS {
        // Reservoir-pick a random adjacent room not yet in the graph.
        let mut j = 0;
        let mut r2 = None;
        for i in 0..MAXROOMS {
            if CONN[r1][i] && !ingraph[i] {
                j += 1;
                if rng.rnd(j) == 0 {
                    r2 = Some(i);
                }
            }
        }

        match r2 {
            None => {
                // Dead end: jump to another room already in the graph.
                loop {
                    r1 = rng.rnd(MAXROOMS as i32) as usize;
                    if ingraph[r1] {
                        break;
                    }
                }
            }
            Some(r2) => {
                ingraph[r2] = true;
                conn(map, rooms, r1, r2, rng);
                isconn[r1][r2] = true;
                isconn[r2][r1] = true;
                roomcount += 1;
                r1 = r2;
            }
        }
    }

    // Add a random number of extra connections so the layout isn't a pure tree.
    for _ in 0..rng.rnd(5) {
        let from = rng.rnd(MAXROOMS as i32) as usize;
        let mut j = 0;
        let mut r2 = None;
        for i in 0..MAXROOMS {
            if CONN[from][i] && !isconn[from][i] {
                j += 1;
                if rng.rnd(j) == 0 {
                    r2 = Some(i);
                }
            }
        }
        if let Some(to) = r2 {
            conn(map, rooms, from, to, rng);
            isconn[from][to] = true;
            isconn[to][from] = true;
        }
    }
}

/// `conn`: draw an L-shaped corridor between two adjacent rooms.
fn conn(map: &mut Map, rooms: &[GenRoom], a: usize, b: usize, rng: &mut impl RogueRng) {
    let (rm, horizontal) = if a < b {
        (a, a + 1 == b)
    } else {
        (b, b + 1 == a)
    };

    let rpf = rooms[rm];
    let (rpt, del, mut spos, mut epos);

    if horizontal {
        // Moving right to room rm+1.
        rpt = rooms[rm + 1];
        del = Point::new(1, 0);
        spos = rpf.pos;
        epos = rpt.pos;
        if !rpf.gone {
            spos = Point::new(rpf.pos.x + rpf.max.x - 1, rpf.pos.y + rng.rnd(rpf.max.y - 2) + 1);
        }
        if !rpt.gone {
            epos = Point::new(rpt.pos.x, rpt.pos.y + rng.rnd(rpt.max.y - 2) + 1);
        }
    } else {
        // Moving down to room rm+3.
        rpt = rooms[rm + 3];
        del = Point::new(0, 1);
        spos = rpf.pos;
        epos = rpt.pos;
        if !rpf.gone {
            spos = Point::new(rpf.pos.x + rng.rnd(rpf.max.x - 2) + 1, rpf.pos.y + rpf.max.y - 1);
        }
        if !rpt.gone {
            epos = Point::new(rpt.pos.x + rng.rnd(rpt.max.x - 2) + 1, rpt.pos.y);
        }
    }

    let (distance, turn_delta, turn_distance) = if horizontal {
        let dist = (spos.x - epos.x).abs() - 1;
        let td = Point::new(0, if spos.y < epos.y { 1 } else { -1 });
        (dist, td, (spos.y - epos.y).abs())
    } else {
        let dist = (spos.y - epos.y).abs() - 1;
        let td = Point::new(if spos.x < epos.x { 1 } else { -1 }, 0);
        (dist, td, (spos.x - epos.x).abs())
    };

    // Doors at room boundaries (or passage cells for gone rooms).
    door(map, &rpf, spos);
    door(map, &rpt, epos);

    if distance <= 0 {
        return;
    }
    let turn_spot = rng.rnd(distance - 1) + 1;

    let mut curr = spos;
    let mut dist = distance;
    while dist > 0 {
        curr = curr + del;
        if dist == turn_spot {
            let mut td = turn_distance;
            while td > 0 {
                putpass(map, curr);
                curr = curr + turn_delta;
                td -= 1;
            }
        }
        putpass(map, curr);
        dist -= 1;
    }
}

/// Place a door on a room wall, or a passage cell for a gone room.
fn door(map: &mut Map, room: &GenRoom, p: Point) {
    if room.gone {
        putpass(map, p);
    } else {
        map.set_tile(p, TileKind::Door);
    }
}

/// Lay a passage cell, but never overwrite existing floor/door/wall tiles.
fn putpass(map: &mut Map, p: Point) {
    if map.tile(p) == TileKind::Empty {
        map.set_tile(p, TileKind::Passage);
    }
}

/// Place gold piles and monsters inside rooms (from `do_rooms` in the original).
fn place_monsters_and_gold(map: &mut Map, rooms: &mut [GenRoom], rng: &mut impl RogueRng) {
    for i in 0..rooms.len() {
        if rooms[i].gone {
            continue;
        }
        // Gold: 50% chance.
        if rng.rnd(2) == 0 {
            if let Some(p) = random_floor_in(map, &rooms[i].rect(), rng) {
                let val = rng.rnd(50 + 10) + 2; // GOLDCALC-style baseline
                rooms[i].gold = val;
                map.spawns.push(Spawn {
                    pos: p,
                    kind: SpawnKind::Gold(val),
                });
            }
        }
        // Monster: 80% if the room has gold, else 25%.
        let chance = if rooms[i].gold > 0 { 80 } else { 25 };
        if rng.percent(chance) {
            if let Some(p) = random_floor_in(map, &rooms[i].rect(), rng) {
                map.spawns.push(Spawn {
                    pos: p,
                    kind: SpawnKind::Monster(None),
                });
            }
        }
    }
}

/// `new_level` trap placement.
fn place_traps(map: &mut Map, depth: i32, cfg: &Config, rng: &mut impl RogueRng) {
    if rng.rnd(10) < depth {
        let ntraps = (rng.rnd(depth / 4) + 1).min(cfg.max_traps);
        for _ in 0..ntraps {
            if let Some(p) = random_floor(map, rng) {
                map.set_tile(p, TileKind::Trap);
            }
        }
    }
}

fn place_stairs(map: &mut Map, rng: &mut impl RogueRng) {
    if let Some(p) = random_floor(map, rng) {
        map.set_tile(p, TileKind::StairsDown);
        map.stairs_down = Some(p);
    }
}

/// `put_things`: scatter floor items across the level.
fn put_things(map: &mut Map, _depth: i32, cfg: &Config, rng: &mut impl RogueRng) {
    for _ in 0..cfg.max_objects_per_level {
        if rng.rnd(100) < 36 {
            if let Some(p) = random_floor(map, rng) {
                map.spawns.push(Spawn {
                    pos: p,
                    kind: SpawnKind::Item,
                });
            }
        }
    }
}

/// Pick a random floor cell anywhere on the map.
fn random_floor(map: &Map, rng: &mut impl RogueRng) -> Option<Point> {
    let mut candidates = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            let p = Point::new(x, y);
            if map.tile(p) == TileKind::Floor && !occupied(map, p) {
                candidates.push(p);
            }
        }
    }
    if candidates.is_empty() {
        None
    } else {
        Some(candidates[rng.rnd(candidates.len() as i32) as usize])
    }
}

/// Pick a random floor cell inside a room rectangle.
fn random_floor_in(map: &Map, rect: &Rect, rng: &mut impl RogueRng) -> Option<Point> {
    let mut candidates = Vec::new();
    for y in (rect.y1 + 1)..(rect.y2 - 1) {
        for x in (rect.x1 + 1)..(rect.x2 - 1) {
            let p = Point::new(x, y);
            if map.tile(p) == TileKind::Floor && !occupied(map, p) {
                candidates.push(p);
            }
        }
    }
    if candidates.is_empty() {
        None
    } else {
        Some(candidates[rng.rnd(candidates.len() as i32) as usize])
    }
}

/// True if a spawn already targets this cell.
fn occupied(map: &Map, p: Point) -> bool {
    map.spawns.iter().any(|s| s.pos == p) || map.stairs_down == Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn cfg() -> Config {
        crate::data::GameData::bundled().unwrap().config
    }

    #[test]
    fn generates_connected_floor() {
        let cfg = cfg();
        for seed in 0..20u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let map = generate_procedural(&cfg, 3, &ProcParams::default(), &mut rng);

            // There must be a down staircase and a valid player start on floor.
            assert!(map.stairs_down.is_some(), "seed {seed}: no stairs");
            assert!(
                map.is_walkable(map.player_start),
                "seed {seed}: player not on walkable tile"
            );

            // Player start should be able to reach the stairs (flood fill).
            assert!(
                reachable(&map, map.player_start, map.stairs_down.unwrap()),
                "seed {seed}: stairs unreachable from player start"
            );

            assert!(!map.rooms.is_empty(), "seed {seed}: no rooms");
        }
    }

    /// Flood-fill reachability over walkable tiles.
    fn reachable(map: &Map, from: Point, to: Point) -> bool {
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![from];
        while let Some(p) = stack.pop() {
            if p == to {
                return true;
            }
            if !seen.insert((p.x, p.y)) {
                continue;
            }
            for d in [
                Point::new(1, 0),
                Point::new(-1, 0),
                Point::new(0, 1),
                Point::new(0, -1),
            ] {
                let np = p + d;
                if map.is_walkable(np) && !seen.contains(&(np.x, np.y)) {
                    stack.push(np);
                }
            }
        }
        false
    }
}
