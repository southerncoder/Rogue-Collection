//! The dungeon level grid: tiles, visibility, rooms and spawn requests.

use crate::geometry::{Point, Rect};

/// What occupies a single map cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    /// Unused rock / void.
    Empty,
    /// Room floor (`.`).
    Floor,
    /// Room wall.
    Wall,
    /// Corridor (`#`).
    Passage,
    /// Door between a room and a corridor (`+`).
    Door,
    /// Stairs down to the next level (`>`).
    StairsDown,
    /// Stairs up (`<`).
    StairsUp,
    /// A trap (`^`).
    Trap,
}

impl TileKind {
    /// Can a creature stand on / walk through this tile?
    pub fn walkable(self) -> bool {
        !matches!(self, TileKind::Empty | TileKind::Wall)
    }

    /// Does this tile block line of sight?
    pub fn opaque(self) -> bool {
        matches!(self, TileKind::Empty | TileKind::Wall)
    }

    /// Default glyph used by simple renderers / fixed-map round-tripping.
    pub fn glyph(self) -> char {
        match self {
            TileKind::Empty => ' ',
            TileKind::Floor => '.',
            TileKind::Wall => '#',
            TileKind::Passage => '#',
            TileKind::Door => '+',
            TileKind::StairsDown => '>',
            TileKind::StairsUp => '<',
            TileKind::Trap => '^',
        }
    }
}

/// What the generator wants placed on the map after layout is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnKind {
    /// A monster identified by its glyph (`A`..=`Z`), or `None` to pick randomly.
    Monster(Option<char>),
    /// A random floor item (picked from the item tables).
    Item,
    /// A pile of gold worth the given value.
    Gold(i32),
    /// A specific item by name (used by hand-authored levels).
    NamedItem(&'static str),
}

/// A request to place something at a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawn {
    pub pos: Point,
    pub kind: SpawnKind,
}

/// A fully generated (or loaded) dungeon level.
#[derive(Debug, Clone)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    tiles: Vec<TileKind>,
    /// Has the player ever seen this tile (persisted "memory").
    revealed: Vec<bool>,
    /// Is the tile currently in view this turn.
    visible: Vec<bool>,
    /// Tile belongs to a dark room (only lit when adjacent / by torch).
    dark: Vec<bool>,
    /// Room rectangles, used for lighting whole rooms at once.
    pub rooms: Vec<Rect>,
    pub player_start: Point,
    pub stairs_down: Option<Point>,
    pub spawns: Vec<Spawn>,
}

impl Map {
    /// Create an empty (all-rock) map.
    pub fn new(width: i32, height: i32) -> Self {
        let n = (width * height) as usize;
        Map {
            width,
            height,
            tiles: vec![TileKind::Empty; n],
            revealed: vec![false; n],
            visible: vec![false; n],
            dark: vec![false; n],
            rooms: Vec::new(),
            player_start: Point::new(width / 2, height / 2),
            stairs_down: None,
            spawns: Vec::new(),
        }
    }

    #[inline]
    pub fn in_bounds(&self, p: Point) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    #[inline]
    pub fn idx(&self, p: Point) -> usize {
        (p.y * self.width + p.x) as usize
    }

    pub fn tile(&self, p: Point) -> TileKind {
        if self.in_bounds(p) {
            self.tiles[self.idx(p)]
        } else {
            TileKind::Empty
        }
    }

    pub fn set_tile(&mut self, p: Point, t: TileKind) {
        if self.in_bounds(p) {
            let i = self.idx(p);
            self.tiles[i] = t;
        }
    }

    pub fn is_walkable(&self, p: Point) -> bool {
        self.tile(p).walkable()
    }

    pub fn is_opaque(&self, p: Point) -> bool {
        self.tile(p).opaque()
    }

    pub fn is_revealed(&self, p: Point) -> bool {
        self.in_bounds(p) && self.revealed[self.idx(p)]
    }

    pub fn is_visible(&self, p: Point) -> bool {
        self.in_bounds(p) && self.visible[self.idx(p)]
    }

    pub fn is_dark(&self, p: Point) -> bool {
        self.in_bounds(p) && self.dark[self.idx(p)]
    }

    pub fn set_dark(&mut self, p: Point, dark: bool) {
        if self.in_bounds(p) {
            let i = self.idx(p);
            self.dark[i] = dark;
        }
    }

    pub fn reveal(&mut self, p: Point) {
        if self.in_bounds(p) {
            let i = self.idx(p);
            self.revealed[i] = true;
        }
    }

    /// Clear current visibility (call at the start of each FOV recompute).
    pub fn clear_visible(&mut self) {
        self.visible.iter_mut().for_each(|v| *v = false);
    }

    /// Mark a tile visible (and therefore revealed).
    pub fn set_visible(&mut self, p: Point) {
        if self.in_bounds(p) {
            let i = self.idx(p);
            self.visible[i] = true;
            self.revealed[i] = true;
        }
    }

    /// Which room (if any) contains this point.
    pub fn room_at(&self, p: Point) -> Option<usize> {
        self.rooms.iter().position(|r| r.contains(p))
    }

    /// Walkable 8-directional neighbours of `p`.
    fn walk_neighbors(&self, p: Point) -> Vec<Point> {
        const DIRS: [(i32, i32); 8] = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        DIRS.iter()
            .map(|&(dx, dy)| p + Point::new(dx, dy))
            .filter(|&q| self.is_walkable(q))
            .collect()
    }

    /// Breadth-first shortest path from `from` to `to` over walkable tiles
    /// (8-directional movement, matching the game). Returns the sequence of
    /// steps *excluding* `from` (so the last element is `to`), or `None` if
    /// `to` is unreachable.
    pub fn find_path(&self, from: Point, to: Point) -> Option<Vec<Point>> {
        if from == to {
            return Some(Vec::new());
        }
        if !self.is_walkable(to) {
            return None;
        }
        use std::collections::HashMap;
        use std::collections::VecDeque;
        let mut prev: HashMap<Point, Point> = HashMap::new();
        let mut queue: VecDeque<Point> = VecDeque::new();
        prev.insert(from, from);
        queue.push_back(from);
        while let Some(cur) = queue.pop_front() {
            for n in self.walk_neighbors(cur) {
                if prev.contains_key(&n) {
                    continue;
                }
                prev.insert(n, cur);
                if n == to {
                    let mut path = vec![to];
                    let mut step = cur;
                    while step != from {
                        path.push(step);
                        step = prev[&step];
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back(n);
            }
        }
        None
    }

    /// Is `to` reachable from `from` over walkable tiles?
    pub fn reachable(&self, from: Point, to: Point) -> bool {
        self.find_path(from, to).is_some()
    }

    /// Render the tile layer to a string (handy for tests / debugging).
    pub fn to_ascii(&self) -> String {
        let mut s = String::with_capacity(((self.width + 1) * self.height) as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                s.push(self.tile(Point::new(x, y)).glyph());
            }
            s.push('\n');
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_and_tiles() {
        let mut m = Map::new(10, 5);
        assert!(m.in_bounds(Point::new(0, 0)));
        assert!(!m.in_bounds(Point::new(10, 0)));
        m.set_tile(Point::new(3, 2), TileKind::Floor);
        assert_eq!(m.tile(Point::new(3, 2)), TileKind::Floor);
        assert!(m.is_walkable(Point::new(3, 2)));
        assert!(!m.is_walkable(Point::new(0, 0)));
    }

    #[test]
    fn visibility_tracking() {
        let mut m = Map::new(4, 4);
        let p = Point::new(1, 1);
        assert!(!m.is_revealed(p));
        m.set_visible(p);
        assert!(m.is_visible(p) && m.is_revealed(p));
        m.clear_visible();
        assert!(!m.is_visible(p));
        assert!(m.is_revealed(p), "revealed memory persists");
    }
}
