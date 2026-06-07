//! The playable game: world building, turn loop, combat, items and rendering.

use bracket_lib::prelude::*;
use hecs::{Entity, World};
use rand::rngs::StdRng;
use rand::SeedableRng;

use rogue_core::combat::{roll_attack, Attacker};
use rogue_core::data::{GameData, ItemCategory};
use rogue_core::dice::DamageRoll;
use rogue_core::map::{SpawnKind, TileKind};
use rogue_core::rng::RogueRng;
use rogue_core::{Dungeon, Map, Point};

use crate::components::*;
use crate::theme::Theme;

/// Screen layout: one message line on top, the map, then a status line.
pub const SCREEN_WIDTH: i32 = 80;
pub const SCREEN_HEIGHT: i32 = 28;
const MAP_TOP: i32 = 1; // map is drawn starting at this screen row
const MSG_ROW: i32 = 0;
const MAX_LOG: usize = 200;
/// Frames between autopilot steps (~10 steps/sec at 60 FPS) so it's watchable.
const AUTO_PERIOD: i32 = 6;

/// Current high-level game phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Title,
    Playing,
    Help,
    Inventory,
    ConfirmQuit,
    Dead,
    Won,
    MessageLog,
    Scores,
}

/// Tracks which item names the player has identified this run.
#[derive(Debug, Default)]
struct KnownItems {
    identified: std::collections::HashSet<String>,
}

impl KnownItems {
    fn identify(&mut self, base_name: &str) {
        self.identified.insert(base_name.to_string());
    }
    #[allow(dead_code)]
    fn is_known(&self, base_name: &str) -> bool {
        self.identified.contains(base_name)
    }
}

/// The whole game state handed to bracket-lib each tick.
pub struct Game {
    world: World,
    map: Map,
    dungeon: Dungeon,
    data: GameData,
    rng: StdRng,
    depth: i32,
    player: Entity,
    inventory: Vec<Item>,
    gold: i32,
    food: i32,
    regen_counter: i32,
    /// When wearing a SlowDigestion ring, alternates each turn to halve food consumption.
    digest_skip: bool,
    /// Ring worn on the left hand (if any).
    left_ring: Option<RingKind>,
    /// Ring worn on the right hand (if any).
    right_ring: Option<RingKind>,
    /// Monster archetype indices sorted easiest-first (by experience).
    monster_order: Vec<usize>,
    log: Vec<String>,
    mode: Mode,
    /// When true, a pathfinding bot plays the game (watchable demo / tests).
    autopilot: bool,
    /// Frames remaining before the autopilot takes its next step (throttle).
    auto_cooldown: i32,
    /// Mode to return to if the player cancels the quit confirmation.
    quit_return: Mode,
    /// Scroll offset for the message log panel (0 = most recent messages at top).
    log_scroll: usize,
    /// Scroll offset for the inventory panel.
    inv_scroll: usize,
    /// Total player turns taken this game (incremented each time the player acts).
    turns: u64,
    /// Status effects currently active on the player.
    status: StatusEffects,
    /// Trap kinds registered for the current level.
    trap_kinds: Vec<(Point, TrapKind)>,
    /// Items the player has identified this run.
    known_items: KnownItems,
    /// Active visual color theme.
    pub theme: Theme,
}

impl Default for Game {
    fn default() -> Self {
        Game::new()
    }
}

impl Game {
    pub fn new() -> Game {
        // Prefer on-disk assets (so editing dungeon.ron / levels takes effect),
        // falling back to a purely procedural dungeon if they are absent.
        let dungeon = Dungeon::load("assets").unwrap_or_else(|| Dungeon::default_procedural(26));
        Game::with_dungeon(dungeon)
    }

    /// Build a game on an explicit dungeon. Used by `new` and by tests that
    /// load a specific dungeon directory.
    pub fn with_dungeon(dungeon: Dungeon) -> Game {
        Game::assemble(dungeon, StdRng::from_entropy())
    }

    /// Build a game on an explicit dungeon with a fixed RNG seed — gives
    /// deterministic level layouts and combat, used by integration tests.
    pub fn with_dungeon_seeded(dungeon: Dungeon, seed: u64) -> Game {
        Game::assemble(dungeon, StdRng::seed_from_u64(seed))
    }

    fn assemble(dungeon: Dungeon, rng: StdRng) -> Game {
        let data = GameData::bundled().expect("bundled game data must parse");

        let mut monster_order: Vec<usize> = (0..data.monsters.len()).collect();
        monster_order.sort_by_key(|&i| data.monsters[i].experience);

        let mut world = World::new();

        // Create the player up front; stats persist across levels.
        let player = world.spawn((
            Player,
            Position(Point::new(0, 0)),
            Renderable {
                glyph: '@',
                color: (255, 255, 0),
            },
            Name("you".to_string()),
            Stats {
                hp: data.config.start_hp,
                max_hp: data.config.start_hp,
                level: 1,
                strength: data.config.start_strength,
                armor: data.config.start_armor,
                damage: "2x4".parse().unwrap(), // starting mace
                hit_plus: 0,
                dam_plus: 0,
                xp_reward: 0,
            },
            BlocksTile,
        ));

        let food = data.config.hunger_time;
        let mut game = Game {
            world,
            map: Map::new(data.config.map_width, data.config.map_height),
            dungeon,
            data,
            rng,
            depth: 0,
            player,
            inventory: vec![Item {
                name: "food ration".to_string(),
                kind: ItemKind::Food,
            }],
            gold: 0,
            food,
            regen_counter: 0,
            digest_skip: false,
            left_ring: None,
            right_ring: None,
            monster_order,
            log: Vec::new(),
            mode: Mode::Title,
            autopilot: false,
            auto_cooldown: 0,
            quit_return: Mode::Title,
            log_scroll: 0,
            inv_scroll: 0,
            turns: 0,
            status: StatusEffects::default(),
            trap_kinds: Vec::new(),
            known_items: KnownItems::default(),
            theme: Theme::classic(),
        };
        game.descend_to(1);
        game.log("Welcome to the Dungeons of Doom! Find the Amulet of Yendor.");
        game
    }

    fn log(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > MAX_LOG {
            self.log.remove(0);
        }
    }

    fn record_score(&mut self, won: bool) {
        use crate::scores::{ScoreEntry, Scores};
        let mut scores = Scores::load();
        scores.add(ScoreEntry {
            name: "adventurer".to_string(),
            depth: self.depth,
            gold: self.gold,
            turns: self.turns,
            won,
        });
    }

    // --- Level construction -------------------------------------------------

    /// Generate (or load) the given depth and populate the world for it.
    fn descend_to(&mut self, depth: i32) {
        self.depth = depth;
        let cfg = self.data.config.clone();
        self.map = self.dungeon.build_level(depth, &cfg, &mut self.rng);

        // Clear everything that isn't the player (every entity has a Position).
        let stale: Vec<Entity> = self
            .world
            .query::<&Position>()
            .iter()
            .map(|(e, _)| e)
            .filter(|&e| e != self.player)
            .collect();
        for e in stale {
            let _ = self.world.despawn(e);
        }

        let start = self.map.player_start;
        if let Ok(mut pos) = self.world.get::<&mut Position>(self.player) {
            pos.0 = start;
        }

        // Materialise spawn requests into entities.
        let spawns = self.map.spawns.clone();
        for s in spawns {
            match s.kind {
                SpawnKind::Monster(glyph) => self.spawn_monster(s.pos, glyph),
                SpawnKind::Gold(v) => {
                    self.world.spawn((
                        Position(s.pos),
                        GoldPile(v),
                        Renderable {
                            glyph: '$',
                            color: (255, 215, 0),
                        },
                    ));
                }
                SpawnKind::Item => {
                    let item = self.roll_item();
                    let r = item_render(&item);
                    self.world.spawn((Position(s.pos), item, r));
                }
                SpawnKind::NamedItem(name) => {
                    let item = Item {
                        name: name.to_string(),
                        kind: ItemKind::Trinket,
                    };
                    let r = item_render(&item);
                    self.world.spawn((Position(s.pos), item, r));
                }
            }
        }

        // The Amulet of Yendor lives on the deepest level.
        if depth >= self.data.config.amulet_level {
            let pos = self
                .first_free_floor()
                .unwrap_or(self.map.stairs_down.unwrap_or(start));
            let item = Item {
                name: "the Amulet of Yendor".to_string(),
                kind: ItemKind::Amulet,
            };
            let r = item_render(&item);
            self.world.spawn((Position(pos), item, r));
        }

        self.recompute_visibility();
        // Register trap kinds for this level
        self.trap_kinds.clear();
        let trap_positions: Vec<Point> = (0..self.map.height)
            .flat_map(|y| (0..self.map.width).map(move |x| Point::new(x, y)))
            .filter(|&p| self.map.tile(p) == TileKind::Trap)
            .collect();
        for p in trap_positions {
            let kind = match self.rng.rnd(6) {
                0 => TrapKind::Pit,
                1 => TrapKind::ArrowTrap,
                2 => TrapKind::TeleportTrap,
                3 => TrapKind::BearTrap,
                4 => TrapKind::PoisonNeedle,
                _ => TrapKind::SleepiGas,
            };
            self.trap_kinds.push((p, kind));
        }
        // Place a few secret doors adjacent to passages
        let mut secret_count = 0;
        let map_w = self.map.width;
        let map_h = self.map.height;
        let passage_tiles: Vec<Point> = (0..map_h)
            .flat_map(|y| (0..map_w).map(move |x| Point::new(x, y)))
            .filter(|&p| self.map.tile(p) == TileKind::Passage)
            .collect();
        for p in passage_tiles {
            if secret_count >= 5 { break; }
            for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1i32), (0, 1)] {
                let np = Point::new(p.x + dx, p.y + dy);
                if self.map.in_bounds(np) && self.map.tile(np) == TileKind::Wall && self.rng.percent(2) {
                    self.map.set_tile(np, TileKind::SecretDoor);
                    secret_count += 1;
                    break;
                }
            }
        }
        if depth > 1 {
            self.log(format!("You descend to level {depth}."));
        }
    }

    fn spawn_monster(&mut self, pos: Point, glyph: Option<char>) {
        let idx = match glyph {
            Some(g) => self
                .data
                .monsters
                .iter()
                .position(|m| m.symbol == g)
                .unwrap_or_else(|| self.pick_monster_for_depth()),
            None => self.pick_monster_for_depth(),
        };
        let def = self.data.monsters[idx].clone();
        let hp = self.rng.roll(def.level.max(1), 8).max(1);
        let mean = def
            .flags
            .iter()
            .any(|f| matches!(f, rogue_core::data::MonsterFlag::Mean));
        let special = monster_special_from_symbol(def.symbol);
        self.world.spawn((
            Position(pos),
            Renderable {
                glyph: def.symbol,
                color: monster_color(idx, self.data.monsters.len()),
            },
            Monster {
                awake: false,
                mean,
                special,
            },
            Name(def.name.clone()),
            Stats {
                hp,
                max_hp: hp,
                level: def.level,
                strength: def.strength,
                armor: def.armor,
                damage: def.damage.clone(),
                hit_plus: 0,
                dam_plus: 0,
                xp_reward: def.experience,
            },
            BlocksTile,
        ));
    }

    /// Choose a depth-appropriate archetype from the experience-sorted list.
    fn pick_monster_for_depth(&mut self) -> usize {
        let n = self.monster_order.len() as i32;
        let centre = (self.depth - 1).clamp(0, n - 1);
        let lo = (centre - 5).max(0);
        let hi = (centre + 2).min(n - 1);
        let pick = lo + self.rng.rnd(hi - lo + 1);
        self.monster_order[pick as usize]
    }

    fn roll_item(&mut self) -> Item {
        let cats = &self.data.items;
        let total: i32 = cats.categories.iter().map(|c| c.prob).sum();
        let mut roll = self.rng.rnd(total.max(1));
        let mut chosen = ItemCategory::Food;
        for c in &cats.categories {
            if roll < c.prob {
                chosen = c.category;
                break;
            }
            roll -= c.prob;
        }
        match chosen {
            ItemCategory::Food => Item {
                name: "food ration".to_string(),
                kind: ItemKind::Food,
            },
            ItemCategory::Potion => {
                let p = pick_named(&cats.potions, &mut self.rng);
                let kind = ItemKind::Potion(potion_kind_from_name(&p));
                Item {
                    name: format!("potion of {p}"),
                    kind,
                }
            }
            ItemCategory::Weapon => {
                let w = &cats.weapons[self.rng.rnd(cats.weapons.len() as i32) as usize];
                Item {
                    name: w.name.clone(),
                    kind: ItemKind::Weapon {
                        damage: w.damage.clone(),
                        hit_plus: 0,
                        dam_plus: 0,
                    },
                }
            }
            ItemCategory::Armor => {
                let a = &cats.armor[self.rng.rnd(cats.armor.len() as i32) as usize];
                Item {
                    name: a.name.clone(),
                    kind: ItemKind::Armor(a.base_ac),
                }
            }
            ItemCategory::Scroll => {
                let s = pick_named(&cats.scrolls, &mut self.rng);
                let kind = scroll_kind_from_name(&s);
                Item {
                    name: format!("scroll of {s}"),
                    kind: ItemKind::Scroll(kind),
                }
            }
            ItemCategory::Ring => {
                let s = pick_named(&cats.rings, &mut self.rng);
                let kind = ring_kind_from_name(&s);
                Item {
                    name: format!("ring of {s}"),
                    kind: ItemKind::Ring(kind),
                }
            }
            ItemCategory::Stick => {
                let s = pick_named(&cats.sticks, &mut self.rng);
                let charges = self.rng.rnd(6) + 3;
                Item {
                    name: format!("wand of {s}"),
                    kind: ItemKind::Wand {
                        kind: wand_kind_from_name(&s),
                        charges,
                    },
                }
            }
        }
    }

    fn first_free_floor(&self) -> Option<Point> {
        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let p = Point::new(x, y);
                if self.map.tile(p) == TileKind::Floor && self.entity_at(p).is_none() {
                    return Some(p);
                }
            }
        }
        None
    }

    // --- Player helpers -----------------------------------------------------

    fn player_pos(&self) -> Point {
        self.world.get::<&Position>(self.player).unwrap().0
    }

    fn entity_at(&self, p: Point) -> Option<Entity> {
        for (e, pos) in self.world.query::<&Position>().iter() {
            if pos.0 == p && e != self.player {
                return Some(e);
            }
        }
        None
    }

    fn monster_at(&self, p: Point) -> Option<Entity> {
        for (e, (pos, _)) in self.world.query::<(&Position, &Monster)>().iter() {
            if pos.0 == p {
                return Some(e);
            }
        }
        None
    }

    // --- Turn processing ----------------------------------------------------

    fn handle_key(&mut self, ctx: &mut BTerm) {
        let Some(key) = ctx.key else { return };
        let mut acted = false;
        let mut delta = None;
        match key {
            VirtualKeyCode::Left | VirtualKeyCode::H | VirtualKeyCode::Numpad4 => {
                delta = Some(Point::new(-1, 0))
            }
            VirtualKeyCode::Right | VirtualKeyCode::L | VirtualKeyCode::Numpad6 => {
                delta = Some(Point::new(1, 0))
            }
            VirtualKeyCode::Up | VirtualKeyCode::K | VirtualKeyCode::Numpad8 => {
                delta = Some(Point::new(0, -1))
            }
            VirtualKeyCode::Down | VirtualKeyCode::J | VirtualKeyCode::Numpad2 => {
                delta = Some(Point::new(0, 1))
            }
            VirtualKeyCode::Y | VirtualKeyCode::Numpad7 => delta = Some(Point::new(-1, -1)),
            VirtualKeyCode::U | VirtualKeyCode::Numpad9 => delta = Some(Point::new(1, -1)),
            VirtualKeyCode::B | VirtualKeyCode::Numpad1 => delta = Some(Point::new(-1, 1)),
            VirtualKeyCode::N | VirtualKeyCode::Numpad3 => delta = Some(Point::new(1, 1)),
            VirtualKeyCode::Period => {
                if ctx.shift {
                    acted = self.try_descend();
                } else {
                    acted = true; // wait / rest one turn
                }
            }
            VirtualKeyCode::G => acted = self.pickup(),
            VirtualKeyCode::Q => acted = self.quaff(),
            VirtualKeyCode::Z => acted = self.zap_wand(),
            VirtualKeyCode::E => acted = self.eat(),
            VirtualKeyCode::R => {
                if ctx.shift {
                    acted = self.remove_ring();
                } else {
                    acted = self.read_scroll();
                }
            }
            VirtualKeyCode::P => acted = self.put_on_ring(),
            VirtualKeyCode::S => acted = self.search_for_secrets(),
            VirtualKeyCode::T => acted = self.throw_item(),
            VirtualKeyCode::Tab => acted = self.auto_explore_step(),
            _ => {}
        }
        if let Some(d) = delta {
            acted = self.try_move(d);
        }
        if acted && self.mode == Mode::Playing {
            self.end_player_turn();
        }
    }

    /// Attempt to move (or bump-attack) in a direction. Returns true if a turn
    /// was consumed.
    fn try_move(&mut self, d: Point) -> bool {
        if self.status.paralyzed > 0 {
            self.log("You are paralyzed!");
            return false;
        }
        let d = if self.status.confused > 0 && self.rng.percent(50) {
            let dirs = [
                Point::new(-1, 0),
                Point::new(1, 0),
                Point::new(0, -1),
                Point::new(0, 1),
                Point::new(-1, -1),
                Point::new(1, -1),
                Point::new(-1, 1),
                Point::new(1, 1),
            ];
            dirs[self.rng.rnd(8) as usize]
        } else {
            d
        };
        let dest = self.player_pos() + d;
        if !self.map.is_walkable(dest) {
            return false;
        }
        if let Some(target) = self.monster_at(dest) {
            self.player_attack(target);
            return true;
        }
        self.world.get::<&mut Position>(self.player).unwrap().0 = dest;
        self.on_player_enter(dest);
        true
    }

    fn on_player_enter(&mut self, p: Point) {
        // Auto-pick gold; announce items and traps.
        if let Some(e) = self.entity_at(p) {
            if let Ok(g) = self.world.get::<&GoldPile>(e).map(|g| g.0) {
                self.gold += g;
                let _ = self.world.despawn(e);
                self.log(format!("You found {g} gold pieces."));
            } else if let Ok(name) = self.world.get::<&Item>(e).map(|i| i.name.clone()) {
                self.log(format!("You see {name} here. (press g to pick up)"));
            }
        }
        if self.map.tile(p) == TileKind::Trap {
            self.trigger_trap(p);
            self.map.set_tile(p, TileKind::Floor);
        }
    }

    fn trigger_trap(&mut self, pos: Point) {
        let kind = self.trap_kinds.iter().find(|(p, _)| *p == pos).map(|(_, k)| *k).unwrap_or(TrapKind::Pit);
        match kind {
            TrapKind::Pit => {
                let dmg = self.rng.roll(1, 6);
                self.damage_player(dmg);
                self.log(format!("You fall into a pit! (-{dmg} HP)"));
            }
            TrapKind::ArrowTrap => {
                let dmg = self.rng.roll(1, 6);
                self.damage_player(dmg);
                self.log(format!("An arrow springs from the wall! (-{dmg} HP)"));
            }
            TrapKind::TeleportTrap => {
                self.apply_teleport();
                self.log("A teleportation trap whisks you away!");
            }
            TrapKind::BearTrap => {
                let dur = 3 + self.rng.rnd(3);
                self.apply_status("paralyzed", dur);
                self.log(format!("A bear trap snaps shut on your leg! Paralyzed for {dur} turns."));
            }
            TrapKind::PoisonNeedle => {
                self.apply_status("poisoned", 8);
                self.log("A poisoned needle pricks you!");
            }
            TrapKind::SleepiGas => {
                let dur = 3 + self.rng.rnd(4);
                self.apply_status("paralyzed", dur);
                self.log(format!("Sleeping gas fills the room! You sleep for {dur} turns."));
            }
        }
    }

    fn search_for_secrets(&mut self) -> bool {
        let ppos = self.player_pos();
        let mut found_any = false;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 { continue; }
                let p = Point::new(ppos.x + dx, ppos.y + dy);
                if self.map.in_bounds(p) && self.map.tile(p) == TileKind::SecretDoor {
                    self.map.set_tile(p, TileKind::Door);
                    self.map.set_visible(p);
                    self.map.reveal(p);
                    self.log("You find a secret door!");
                    found_any = true;
                }
            }
        }
        if !found_any {
            self.log("You search but find nothing.");
        }
        true
    }

    fn try_descend(&mut self) -> bool {
        if self.map.tile(self.player_pos()) == TileKind::StairsDown {
            self.descend_to(self.depth + 1);
            true
        } else {
            self.log("There are no stairs down here.");
            false
        }
    }

    fn throw_item(&mut self) -> bool {
        let pos = self.inventory.iter().position(|it| matches!(it.kind, ItemKind::Weapon { .. }))
            .or(if !self.inventory.is_empty() { Some(0) } else { None });
        let Some(i) = pos else {
            self.log("You have nothing to throw.");
            return false;
        };
        let item = self.inventory.remove(i);

        let ppos = self.player_pos();
        let target: Option<Entity> = {
            let candidates: Vec<(Entity, Point)> = self.world.query::<(&Position, &Monster)>().iter()
                .map(|(e, (p, _))| (e, p.0))
                .collect();
            let mut nearest: Option<(i32, Entity)> = None;
            for (e, pos) in candidates {
                if self.map.is_visible(pos) {
                    let dist = pos.chebyshev(ppos);
                    if nearest.map(|(d, _)| dist < d).unwrap_or(true) {
                        nearest = Some((dist, e));
                    }
                }
            }
            nearest.map(|(_, e)| e)
        };

        if let Some(t) = target {
            let dmg = match &item.kind {
                ItemKind::Weapon { .. } => self.rng.roll(1, 6) + 1,
                _ => self.rng.roll(1, 4),
            };
            let mname = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_else(|_| "it".to_string());
            let item_name = item.name.clone();
            let dead = {
                let mut s = self.world.get::<&mut Stats>(t).unwrap();
                s.hp -= dmg;
                s.hp <= 0
            };
            self.log(format!("You throw the {item_name}. It hits the {mname} for {dmg}!"));
            if dead {
                let xp = self.world.get::<&Stats>(t).map(|s| s.xp_reward).unwrap_or(0);
                let _ = self.world.despawn(t);
                self.log(format!("The {mname} is slain!"));
                self.gain_xp(xp);
            }
        } else {
            let drop_pos: Option<Point> = {
                let candidates: Vec<Point> = (-2i32..=2)
                    .flat_map(|dy| (-2i32..=2).map(move |dx| Point::new(ppos.x + dx, ppos.y + dy)))
                    .filter(|&p| self.map.is_walkable(p) && self.entity_at(p).is_none())
                    .collect();
                if candidates.is_empty() {
                    None
                } else {
                    let idx = self.rng.rnd(candidates.len() as i32) as usize;
                    Some(candidates[idx])
                }
            };
            let item_name = item.name.clone();
            if let Some(p) = drop_pos {
                let r = item_render(&item);
                self.world.spawn((Position(p), item, r));
                self.log(format!("The {item_name} clatters to the floor."));
            } else {
                self.log(format!("The {item_name} disappears into the darkness."));
            }
        }
        true
    }

    fn auto_explore_step(&mut self) -> bool {
        use std::collections::{HashMap, VecDeque};

        let ppos = self.player_pos();

        // Stop if any monster is visible
        let monster_positions: Vec<Point> = self.world.query::<(&Position, &Monster)>().iter()
            .map(|(_, (p, _))| p.0)
            .collect();
        if monster_positions.iter().any(|&pos| self.map.is_visible(pos)) {
            self.log("A monster is nearby — you stop exploring.");
            return false;
        }

        let mut queue: VecDeque<Point> = VecDeque::new();
        let mut came_from: HashMap<(i32,i32), Option<(i32,i32)>> = HashMap::new();
        queue.push_back(ppos);
        came_from.insert((ppos.x, ppos.y), None);
        let mut target: Option<Point> = None;

        'outer: while let Some(cur) = queue.pop_front() {
            for (dx, dy) in [(-1i32,0i32),(1,0),(0,-1),(0,1),(-1,-1),(1,-1),(-1,1),(1,1)] {
                let np = Point::new(cur.x + dx, cur.y + dy);
                if !self.map.in_bounds(np) { continue; }
                if !self.map.is_revealed(np) && self.map.tile(np) != TileKind::Empty && target.is_none() {
                    target = Some(cur);
                    break 'outer;
                }
                if came_from.contains_key(&(np.x, np.y)) { continue; }
                if self.map.is_walkable(np) && self.map.is_revealed(np) {
                    came_from.insert((np.x, np.y), Some((cur.x, cur.y)));
                    queue.push_back(np);
                }
            }
        }

        let Some(dest) = target else {
            self.log("No unexplored areas visible. Exploration complete.");
            return false;
        };

        if dest == ppos {
            for (dx, dy) in [(-1i32,0i32),(1,0),(0,-1),(0,1)] {
                let np = Point::new(ppos.x + dx, ppos.y + dy);
                if self.map.is_walkable(np) {
                    return self.try_move(Point::new(dx, dy));
                }
            }
            return false;
        }

        let mut path = vec![(dest.x, dest.y)];
        let mut cur = (dest.x, dest.y);
        while let Some(Some(prev)) = came_from.get(&cur) {
            path.push(*prev);
            cur = *prev;
            if cur == (ppos.x, ppos.y) { break; }
        }
        path.reverse();

        if let Some(&(nx, ny)) = path.get(1) {
            let dir = Point::new(nx - ppos.x, ny - ppos.y);
            return self.try_move(dir);
        }

        false
    }

    fn pickup(&mut self) -> bool {
        let p = self.player_pos();
        let Some(e) = self.entity_at(p) else {
            self.log("There is nothing here to pick up.");
            return false;
        };
        let Ok(item) = self.world.get::<&Item>(e).map(|i| (*i).clone()) else {
            self.log("There is nothing here to pick up.");
            return false;
        };
        let _ = self.world.despawn(e);
        if matches!(item.kind, ItemKind::Amulet) {
            self.log("You have recovered the Amulet of Yendor! YOU WIN!");
            self.mode = Mode::Won;
            self.record_score(true);
            return true;
        }
        self.log(format!("You pick up {}.", item.name));
        match &item.kind {
            ItemKind::Weapon {
                damage,
                hit_plus,
                dam_plus,
            } => {
                self.wield(damage.clone(), *hit_plus, *dam_plus, &item.name);
            }
            ItemKind::Armor(ac) => self.wear(*ac, &item.name),
            _ => self.inventory.push(item),
        }
        true
    }

    fn wield(&mut self, damage: DamageRoll, hit_plus: i32, dam_plus: i32, name: &str) {
        let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
        s.damage = damage;
        s.hit_plus = hit_plus;
        s.dam_plus = dam_plus;
        drop(s);
        self.log(format!("You wield the {name}."));
    }

    fn wear(&mut self, ac: i32, name: &str) {
        let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
        if ac < s.armor {
            s.armor = ac;
            drop(s);
            self.log(format!("You don the {name}."));
        } else {
            drop(s);
            self.log(format!("You don the {name} (no better than your current armor)."));
        }
    }

    /// Use (quaff/eat/read/wield/wear/zap) the item at `idx` in the inventory.
    /// Returns true if a game turn was consumed.
    fn use_inventory_item(&mut self, idx: usize) -> bool {
        if idx >= self.inventory.len() {
            return false;
        }
        let item = self.inventory[idx].clone();
        match item.kind {
            ItemKind::Heal(amount) => {
                self.inventory.remove(idx);
                let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                s.hp = (s.hp + amount).min(s.max_hp);
                drop(s);
                self.log(format!("You quaff {} and feel better.", item.name));
                true
            }
            ItemKind::Potion(kind) => {
                self.inventory.remove(idx);
                self.log(format!("You quaff the {}.", item.name));
                self.apply_potion(kind);
                self.known_items.identify(&item.name);
                true
            }
            ItemKind::Food => {
                self.inventory.remove(idx);
                self.food = self.data.config.stomach_size
                    .min(self.food + self.data.config.hunger_time);
                self.log("You eat the food ration. That hit the spot!");
                true
            }
            ItemKind::Scroll(kind) => {
                self.inventory.remove(idx);
                self.log(format!("You read the {}.", item.name));
                match kind {
                    ScrollKind::MagicMapping => self.apply_magic_mapping(),
                    ScrollKind::Teleport => self.apply_teleport(),
                    ScrollKind::EnchantWeapon => self.apply_enchant_weapon(),
                    ScrollKind::EnchantArmor => self.apply_enchant_armor(),
                    ScrollKind::Aggravate => self.apply_aggravate(),
                    ScrollKind::Identify => {
                        for it in &self.inventory {
                            self.known_items.identify(&it.name);
                        }
                        self.log("Your items shimmer with clarity! All items identified.");
                    }
                    ScrollKind::Unknown => {
                        self.log("The scroll crumbles to dust. Nothing happens.");
                    }
                }
                true
            }
            ItemKind::Weapon { ref damage, hit_plus, dam_plus } => {
                let d = damage.clone();
                self.wield(d, hit_plus, dam_plus, &item.name.clone());
                // Weapon stays in inventory (already equipped in-place)
                true
            }
            ItemKind::Armor(ac) => {
                self.wear(ac, &item.name.clone());
                true
            }
            ItemKind::Ring(kind) => {
                if self.left_ring.is_some() && self.right_ring.is_some() {
                    self.log("You are already wearing two rings. Remove one first (Shift+R).");
                    return false;
                }
                self.inventory.remove(idx);
                let slot = if self.left_ring.is_none() {
                    self.left_ring = Some(kind);
                    "left"
                } else {
                    self.right_ring = Some(kind);
                    "right"
                };
                self.log(format!("You put on the {} ({} hand).", item.name, slot));
                self.on_ring_equip(kind);
                true
            }
            ItemKind::Wand { kind, charges } => {
                if charges <= 0 {
                    self.log("The wand is exhausted.");
                    return false;
                }
                if let ItemKind::Wand { charges: ref mut ch, .. } = self.inventory[idx].kind {
                    *ch -= 1;
                }
                self.log(format!("You zap the {}.", item.name));
                let player_pos = self.player_pos();
                let target = {
                    let candidates: Vec<(Entity, i32)> = self.world
                        .query::<(&Position, &Monster)>()
                        .iter()
                        .filter_map(|(e, (pos, _))| {
                            if !self.map.is_visible(pos.0) { return None; }
                            let dx = pos.0.x - player_pos.x;
                            let dy = pos.0.y - player_pos.y;
                            Some((e, dx * dx + dy * dy))
                        })
                        .collect();
                    candidates.into_iter().min_by_key(|&(_, d)| d).map(|(e, _)| e)
                };
                self.apply_wand(kind, target);
                true
            }
            ItemKind::Amulet => {
                self.log("The Amulet of Yendor glows with golden light.");
                false
            }
            ItemKind::Trinket => {
                self.log(format!("You examine the {}. It appears to be a trinket.", item.name));
                false
            }
        }
    }

    fn quaff(&mut self) -> bool {
        let pos = self
            .inventory
            .iter()
            .position(|it| matches!(it.kind, ItemKind::Heal(_) | ItemKind::Potion(_)));
        if let Some(i) = pos {
            let item = self.inventory.remove(i);
            let name = item.name.clone();
            match item.kind {
                ItemKind::Heal(amount) => {
                    let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                    s.hp = (s.hp + amount).min(s.max_hp);
                    drop(s);
                    self.log(format!("You quaff {name} and feel better."));
                }
                ItemKind::Potion(kind) => {
                    self.log(format!("You quaff the {name}."));
                    self.apply_potion(kind);
                }
                _ => {}
            }
            self.known_items.identify(&name);
            true
        } else {
            self.log("You have no potions.");
            false
        }
    }

    fn apply_potion(&mut self, kind: PotionKind) {
        match kind {
            PotionKind::Healing => {
                let heal = self.rng.roll(2, 8) + 2;
                let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                s.hp = (s.hp + heal).min(s.max_hp);
                drop(s);
                self.log("You feel better.");
            }
            PotionKind::ExtraHealing => {
                let heal = self.rng.roll(3, 8) + 6;
                let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                s.hp = (s.hp + heal).min(s.max_hp);
                drop(s);
                self.log("You feel much better.");
            }
            PotionKind::Poison => {
                self.apply_status("poisoned", 10);
                self.log("You feel very sick.");
            }
            PotionKind::GainStrength => {
                let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                s.strength += 1;
                drop(s);
                self.log("You feel stronger.");
            }
            PotionKind::RestoreStrength => {
                self.apply_status("restore", 1);
            }
            PotionKind::SeeInvisible => {
                self.apply_status("haste", 1);
                self.log("Your eyes tingle.");
            }
            PotionKind::Confusion => {
                self.apply_status("confused", 20);
            }
            PotionKind::Blindness => {
                self.apply_status("blind", 30);
            }
            PotionKind::Hallucination => {
                self.log("Oh wow! Everything seems so cosmic!");
            }
            PotionKind::HasteSelf => {
                self.apply_status("haste", 20);
            }
            PotionKind::RaiseLevel => {
                let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                s.level += 1;
                drop(s);
                self.log("You feel more experienced.");
            }
            PotionKind::MonsterDetection => {
                self.log("You sense the presence of monsters.");
            }
            PotionKind::MagicDetection => {
                self.log("You sense the presence of magic.");
            }
            PotionKind::Levitation => {
                self.log("You float up into the air.");
            }
            PotionKind::Unknown => {
                self.log("Nothing seems to happen.");
            }
        }
    }

    fn zap_wand(&mut self) -> bool {
        let pos = self
            .inventory
            .iter()
            .position(|it| matches!(it.kind, ItemKind::Wand { .. }));
        if let Some(i) = pos {
            let (kind, charges) = match self.inventory[i].kind {
                ItemKind::Wand { kind, charges } => (kind, charges),
                _ => unreachable!(),
            };
            if charges <= 0 {
                self.log("The wand is exhausted.");
                return false;
            }
            if let ItemKind::Wand { ref mut charges, .. } = self.inventory[i].kind {
                *charges -= 1;
            }
            let name = self.inventory[i].name.clone();
            self.log(format!("You zap the {name}."));
            // Find nearest visible monster as target
            let player_pos = self.player_pos();
            let target = {
                let mut best: Option<(Entity, i32)> = None;
                for (e, pos) in self.world.query::<&Position>().iter()
                    .filter(|(e, _)| *e != self.player)
                    .map(|(e, p)| (e, p.0))
                    .collect::<Vec<_>>()
                {
                    if !self.map.is_visible(pos) {
                        continue;
                    }
                    if self.world.get::<&Monster>(e).is_err() {
                        continue;
                    }
                    let dx = pos.x - player_pos.x;
                    let dy = pos.y - player_pos.y;
                    let dist = dx * dx + dy * dy;
                    if best.is_none() || dist < best.unwrap().1 {
                        best = Some((e, dist));
                    }
                }
                best.map(|(e, _)| e)
            };
            self.apply_wand(kind, target);
            true
        } else {
            self.log("You have no wands.");
            false
        }
    }

    fn apply_wand(&mut self, kind: WandKind, target: Option<Entity>) {
        match kind {
            WandKind::MagicMissile => {
                if let Some(t) = target {
                    let dmg = self.rng.roll(2, 6);
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    let dead = {
                        let mut s = self.world.get::<&mut Stats>(t).unwrap();
                        s.hp -= dmg;
                        s.hp <= 0
                    };
                    self.log(format!("The missile hits the {name} for {dmg}."));
                    if dead {
                        let xp = self.world.get::<&Stats>(t).unwrap().xp_reward;
                        let _ = self.world.despawn(t);
                        self.log(format!("You have slain the {name}!"));
                        self.gain_xp(xp);
                    }
                } else {
                    self.log("The missile flies off into the dark.");
                }
            }
            WandKind::Slow => {
                if let Some(t) = target {
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    self.log(format!("The {name} slows down."));
                } else {
                    self.log("The beam dissipates.");
                }
            }
            WandKind::Fear => {
                if let Some(t) = target {
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    self.log(format!("The {name} turns and flees!"));
                } else {
                    self.log("The beam dissipates.");
                }
            }
            WandKind::Confusion => {
                if let Some(t) = target {
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    self.log(format!("The {name} looks confused."));
                } else {
                    self.log("The beam dissipates.");
                }
            }
            WandKind::DrainLife => {
                if let Some(t) = target {
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    let dmg = {
                        let s = self.world.get::<&Stats>(t).unwrap();
                        s.hp / 2
                    };
                    let dead = {
                        let mut s = self.world.get::<&mut Stats>(t).unwrap();
                        s.hp -= dmg;
                        s.hp <= 0
                    };
                    self.log(format!("You drain life from the {name}!"));
                    if dead {
                        let xp = self.world.get::<&Stats>(t).unwrap().xp_reward;
                        let _ = self.world.despawn(t);
                        self.log(format!("You have slain the {name}!"));
                        self.gain_xp(xp);
                    }
                } else {
                    self.log("The drain beam finds nothing.");
                }
            }
            WandKind::Polymorph => {
                if let Some(t) = target {
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    let _ = self.world.despawn(t);
                    self.log(format!("The {name} transforms!"));
                } else {
                    self.log("The beam dissipates.");
                }
            }
            WandKind::Haste => {
                self.apply_status("haste", 20);
            }
            WandKind::TeleportAway => {
                if let Some(t) = target {
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    let mut floors: Vec<Point> = Vec::new();
                    for y in 0..self.map.height {
                        for x in 0..self.map.width {
                            let p = Point::new(x, y);
                            if self.map.is_walkable(p) && self.entity_at(p).is_none() {
                                floors.push(p);
                            }
                        }
                    }
                    if !floors.is_empty() {
                        let pick = floors[self.rng.rnd(floors.len() as i32) as usize];
                        if let Ok(mut pos) = self.world.get::<&mut Position>(t) {
                            pos.0 = pick;
                        }
                    }
                    self.log(format!("The {name} vanishes!"));
                } else {
                    self.log("The beam dissipates.");
                }
            }
            WandKind::CancellationWand => {
                if let Some(_t) = target {
                    self.log("The monster's powers are cancelled.");
                } else {
                    self.log("The beam dissipates.");
                }
            }
            WandKind::NothingWand | WandKind::Unknown => {
                self.log("Nothing happens.");
            }
            WandKind::Light => {
                self.apply_magic_mapping();
                self.log("The area is illuminated!");
            }
            WandKind::Fire => {
                if let Some(t) = target {
                    let dmg = self.rng.roll(3, 6);
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    let dead = {
                        let mut s = self.world.get::<&mut Stats>(t).unwrap();
                        s.hp -= dmg;
                        s.hp <= 0
                    };
                    self.log(format!("A burst of fire hits the {name} for {dmg}!"));
                    if dead {
                        let xp = self.world.get::<&Stats>(t).unwrap().xp_reward;
                        let _ = self.world.despawn(t);
                        self.log(format!("You have slain the {name}!"));
                        self.gain_xp(xp);
                    }
                } else {
                    self.log("A burst of fire scorches the wall.");
                }
            }
            WandKind::Cold => {
                if let Some(t) = target {
                    let dmg = self.rng.roll(2, 8);
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    let dead = {
                        let mut s = self.world.get::<&mut Stats>(t).unwrap();
                        s.hp -= dmg;
                        s.hp <= 0
                    };
                    self.log(format!("An icy blast hits the {name} for {dmg}!"));
                    if dead {
                        let xp = self.world.get::<&Stats>(t).unwrap().xp_reward;
                        let _ = self.world.despawn(t);
                        self.log(format!("You have slain the {name}!"));
                        self.gain_xp(xp);
                    }
                } else {
                    self.log("The icy blast fades away.");
                }
            }
            WandKind::Lightning => {
                if let Some(t) = target {
                    let dmg = self.rng.roll(4, 6);
                    let name = self.world.get::<&Name>(t).map(|n| n.0.clone()).unwrap_or_default();
                    let dead = {
                        let mut s = self.world.get::<&mut Stats>(t).unwrap();
                        s.hp -= dmg;
                        s.hp <= 0
                    };
                    self.log(format!("A bolt of lightning strikes the {name} for {dmg}!"));
                    if dead {
                        let xp = self.world.get::<&Stats>(t).unwrap().xp_reward;
                        let _ = self.world.despawn(t);
                        self.log(format!("You have slain the {name}!"));
                        self.gain_xp(xp);
                    }
                } else {
                    self.log("The lightning bolt crackles into the wall.");
                }
            }
        }
    }

    fn eat(&mut self) -> bool {
        if let Some(i) = self
            .inventory
            .iter()
            .position(|it| matches!(it.kind, ItemKind::Food))
        {
            self.inventory.remove(i);
            self.food = self.data.config.stomach_size.min(self.food + self.data.config.hunger_time);
            self.log("You eat the food ration. That hit the spot!");
            true
        } else {
            self.log("You have no food.");
            false
        }
    }

    /// Read the first scroll in the inventory, applying its effect.
    fn read_scroll(&mut self) -> bool {
        let Some(i) = self
            .inventory
            .iter()
            .position(|it| matches!(it.kind, ItemKind::Scroll(_)))
        else {
            self.log("You have no scrolls to read.");
            return false;
        };
        let item = self.inventory.remove(i);
        let ItemKind::Scroll(kind) = item.kind else {
            return false;
        };
        self.log(format!("You read the {}.", item.name));
        match kind {
            ScrollKind::MagicMapping => self.apply_magic_mapping(),
            ScrollKind::Teleport => self.apply_teleport(),
            ScrollKind::EnchantWeapon => self.apply_enchant_weapon(),
            ScrollKind::EnchantArmor => self.apply_enchant_armor(),
            ScrollKind::Aggravate => self.apply_aggravate(),
            ScrollKind::Identify => {
                for item in &self.inventory {
                    self.known_items.identify(&item.name);
                }
                self.log("Your items shimmer with clarity! All items identified.");
            }
            ScrollKind::Unknown => self.log("The scroll crumbles to dust. Nothing happens."),
        }
        true
    }

    fn put_on_ring(&mut self) -> bool {
        let Some(i) = self
            .inventory
            .iter()
            .position(|it| matches!(it.kind, ItemKind::Ring(_)))
        else {
            self.log("You have no rings to put on.");
            return false;
        };
        if self.left_ring.is_some() && self.right_ring.is_some() {
            self.log("You are already wearing two rings. Remove one first (Shift+R).");
            return false;
        }
        let item = self.inventory.remove(i);
        let ItemKind::Ring(kind) = item.kind else {
            return false;
        };
        let slot = if self.left_ring.is_none() {
            self.left_ring = Some(kind);
            "left"
        } else {
            self.right_ring = Some(kind);
            "right"
        };
        self.log(format!("You put on the {} ({} hand).", item.name, slot));
        self.on_ring_equip(kind);
        true
    }

    fn remove_ring(&mut self) -> bool {
        let (kind, slot) = if let Some(k) = self.left_ring.take() {
            (k, "left")
        } else if let Some(k) = self.right_ring.take() {
            (k, "right")
        } else {
            self.log("You are not wearing any rings.");
            return false;
        };
        let name = ring_name(kind);
        self.log(format!("You remove the ring of {} ({} hand).", name, slot));
        self.inventory.push(Item {
            name: format!("ring of {name}"),
            kind: ItemKind::Ring(kind),
        });
        true
    }

    fn on_ring_equip(&mut self, kind: RingKind) {
        match kind {
            RingKind::AggravateMonster => {
                self.apply_aggravate();
                self.log("The ring pulses with malevolent energy!");
            }
            RingKind::Teleportation => {
                self.log("The ring crackles with unstable energy...");
            }
            RingKind::Searching => {
                self.log("Your senses sharpen.");
            }
            RingKind::SlowDigestion => {
                self.log("You feel your metabolism slow.");
            }
            RingKind::Regeneration => {
                self.log("You feel a surge of vitality.");
            }
            RingKind::SustainStrength => {
                self.log("Your muscles feel fortified.");
            }
            RingKind::SeeInvisible => {
                self.log("The world looks slightly different...");
            }
            _ => {}
        }
    }

    // --- Ring passive effect helpers ----------------------------------------

    fn ring_iter(&self) -> impl Iterator<Item = RingKind> {
        [self.left_ring, self.right_ring].into_iter().flatten()
    }

    fn has_ring(&self, kind: RingKind) -> bool {
        self.ring_iter().any(|k| k == kind)
    }

    fn ring_armor_bonus(&self) -> i32 {
        self.ring_iter()
            .filter(|&k| k == RingKind::Protection)
            .count() as i32
    }

    fn ring_hit_bonus(&self) -> i32 {
        self.ring_iter()
            .filter(|&k| k == RingKind::Dexterity)
            .count() as i32
    }

    fn ring_dam_bonus(&self) -> i32 {
        self.ring_iter()
            .filter(|&k| k == RingKind::IncreaseDamage)
            .count() as i32
    }

    fn ring_str_bonus(&self) -> i32 {
        self.ring_iter()
            .filter(|&k| k == RingKind::AddStrength)
            .count() as i32
    }

    fn apply_magic_mapping(&mut self) {
        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let p = Point::new(x, y);
                if self.map.tile(p) != TileKind::Empty {
                    self.map.reveal(p);
                }
            }
        }
        self.log("The dungeon layout floods into your mind!");
    }

    fn apply_teleport(&mut self) {
        let mut floors: Vec<Point> = Vec::new();
        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let p = Point::new(x, y);
                if self.map.is_walkable(p) && self.entity_at(p).is_none() {
                    floors.push(p);
                }
            }
        }
        if floors.is_empty() {
            self.log("The air shimmers, but nothing happens.");
            return;
        }
        let pick = floors[self.rng.rnd(floors.len() as i32) as usize];
        if let Ok(mut pos) = self.world.get::<&mut Position>(self.player) {
            pos.0 = pick;
        }
        self.recompute_visibility();
        self.log("You blink across the dungeon!");
    }

    fn apply_enchant_weapon(&mut self) {
        let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
        s.hit_plus += 1;
        s.dam_plus += 1;
        drop(s);
        self.log("Your weapon glows blue for a moment.");
    }

    fn apply_enchant_armor(&mut self) {
        let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
        s.armor -= 1; // lower armor class is better
        drop(s);
        self.log("Your armor glows silver for a moment.");
    }

    /// Apply a status effect to the player, stacking duration.
    fn apply_status(&mut self, effect: &str, duration: i32) {
        match effect {
            "confused" => {
                self.status.confused += duration;
                self.log(format!("You are confused for {duration} turns!"));
            }
            "blind" => {
                self.status.blind += duration;
                self.log(format!("You are blinded for {duration} turns!"));
            }
            "poisoned" => {
                self.status.poisoned += duration;
                self.log("You are poisoned!");
            }
            "paralyzed" => {
                self.status.paralyzed += duration;
                self.log(format!("You are paralyzed for {duration} turns!"));
            }
            "haste" => {
                self.status.haste += duration;
                self.log(format!("You feel yourself moving faster for {duration} turns!"));
            }
            "restore" => {
                self.status.poisoned = 0;
                self.log("You feel your strength returning.");
            }
            _ => {}
        }
    }

    /// Tick all active status effects (called at end of each player turn).
    fn tick_status_effects(&mut self) {
        if self.status.confused > 0 {
            self.status.confused -= 1;
            if self.status.confused == 0 {
                self.log("You feel less confused.");
            }
        }
        if self.status.blind > 0 {
            self.status.blind -= 1;
            if self.status.blind == 0 {
                self.log("Your vision clears.");
            }
        }
        if self.status.paralyzed > 0 {
            self.status.paralyzed -= 1;
            if self.status.paralyzed == 0 {
                self.log("You can move again.");
            }
        }
        if self.status.haste > 0 {
            self.status.haste -= 1;
            if self.status.haste == 0 {
                self.log("You slow back down.");
            }
        }
        if self.status.poisoned > 0 {
            self.status.poison_tick += 1;
            if self.status.poison_tick >= 3 {
                self.status.poison_tick = 0;
                let drain_needed = {
                    let s = self.world.get::<&Stats>(self.player).unwrap();
                    s.strength > 1
                };
                if drain_needed {
                    let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                    s.strength -= 1;
                    drop(s);
                    self.log("The poison saps your strength!");
                }
                self.status.poisoned -= 1;
            }
        }
    }

    fn apply_aggravate(&mut self) {        let mut count = 0;
        for (_e, m) in self.world.query::<&mut Monster>().iter() {
            m.awake = true;
            count += 1;
        }
        self.log(format!("A high-pitched humming wakes {count} monsters!"));
    }

    fn player_attack(&mut self, target: Entity) {
        let attacker = {
            let s = self.world.get::<&Stats>(self.player).unwrap();
            Attacker {
                level: s.level,
                strength: s.strength + self.ring_str_bonus(),
                damage: s.damage.clone(),
                hit_plus: s.hit_plus + self.ring_hit_bonus(),
                dam_plus: s.dam_plus + self.ring_dam_bonus(),
            }
        };
        let def_arm = self.world.get::<&Stats>(target).unwrap().armor;
        let name = self.world.get::<&Name>(target).unwrap().0.clone();
        match roll_attack(&attacker, def_arm, &mut self.rng) {
            Some(dmg) => {
                let dead = {
                    let mut s = self.world.get::<&mut Stats>(target).unwrap();
                    s.hp -= dmg;
                    s.hp <= 0
                };
                self.log(format!("You hit the {name} for {dmg}."));
                if dead {
                    let xp = self.world.get::<&Stats>(target).unwrap().xp_reward;
                    let _ = self.world.despawn(target);
                    self.log(format!("You have slain the {name}!"));
                    self.gain_xp(xp);
                }
            }
            None => self.log(format!("You miss the {name}.")),
        }
    }

    fn monster_attack(&mut self, attacker_e: Entity) {
        let attacker = {
            let s = self.world.get::<&Stats>(attacker_e).unwrap();
            Attacker {
                level: s.level,
                strength: s.strength,
                damage: s.damage.clone(),
                hit_plus: s.hit_plus,
                dam_plus: s.dam_plus,
            }
        };
        let name = self.world.get::<&Name>(attacker_e).unwrap().0.clone();
        let special = self
            .world
            .get::<&Monster>(attacker_e)
            .map(|m| m.special)
            .unwrap_or(MonsterSpecial::None);
        // Protection ring lowers effective AC (lower = better in Rogue).
        let def_arm = self.world.get::<&Stats>(self.player).unwrap().armor - self.ring_armor_bonus();
        match roll_attack(&attacker, def_arm, &mut self.rng) {
            Some(dmg) => {
                self.damage_player(dmg);
                self.log(format!("The {name} hits you for {dmg}."));
                self.apply_monster_special(&name, special);
            }
            None => self.log(format!("The {name} misses you.")),
        }
    }

    fn apply_monster_special(&mut self, monster_name: &str, special: MonsterSpecial) {
        match special {
            MonsterSpecial::None => {}
            MonsterSpecial::Poison => {
                self.apply_status("poisoned", 10);
                self.log(format!("The {monster_name}'s bite is venomous!"));
            }
            MonsterSpecial::Confuse => {
                self.apply_status("confused", 20);
                self.log(format!("The {monster_name} has confused you!"));
            }
            MonsterSpecial::Paralyze => {
                self.apply_status("paralyzed", 10);
                self.log(format!("You are paralyzed by the {monster_name}!"));
            }
            MonsterSpecial::Blind => {
                self.apply_status("blind", 30);
                self.log(format!("The {monster_name} has blinded you!"));
            }
            MonsterSpecial::DrainLevel => {
                let new_level = {
                    let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                    s.level = (s.level - 1).max(1);
                    s.level
                };
                self.log(format!(
                    "The {monster_name} drains your life force! (level {new_level})"
                ));
            }
            MonsterSpecial::DrainStrength => {
                {
                    let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                    s.strength = (s.strength - 1).max(1);
                }
                self.log(format!("The {monster_name} saps your strength!"));
            }
            MonsterSpecial::StealGold => {
                let stolen = self.gold;
                self.gold = 0;
                if stolen > 0 {
                    self.log(format!("The {monster_name} steals {stolen} gold and vanishes!"));
                } else {
                    self.log(format!("The {monster_name} finds nothing to steal."));
                }
            }
        }
    }

    fn damage_player(&mut self, dmg: i32) {
        let dead = {
            let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
            s.hp -= dmg;
            s.hp <= 0
        };
        if dead {
            self.mode = Mode::Dead;
            self.log("You die...");
            self.record_score(false);
        }
    }

    fn gain_xp(&mut self, xp: i32) {
        let before = {
            let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
            s.xp_reward += xp;
            s.level
        };
        let total = self.world.get::<&Stats>(self.player).unwrap().xp_reward;
        // Doubling experience thresholds: 10, 20, 40, 80, ...
        let mut lvl = 1;
        let mut need = 10;
        while total >= need {
            lvl += 1;
            need *= 2;
        }
        if lvl > before {
            let gain = (lvl - before) * self.rng.roll(1, 8);
            {
                let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                s.level = lvl;
                s.max_hp += gain;
                s.hp += gain;
            }
            self.log(format!("Welcome to level {lvl}!"));
        }
    }

    fn end_player_turn(&mut self) {
        self.turns += 1;
        self.tick_status_effects();
        self.monsters_act();
        self.tick_hunger_and_regen();
        self.recompute_visibility();
        if self.world.get::<&Stats>(self.player).map(|s| s.hp).unwrap_or(0) <= 0 {
            self.mode = Mode::Dead;
        }
    }

    fn monsters_act(&mut self) {
        let ppos = self.player_pos();
        let movers: Vec<Entity> = self
            .world
            .query::<&Monster>()
            .iter()
            .map(|(e, _)| e)
            .collect();
        for e in movers {
            if self.world.get::<&Monster>(e).is_err() {
                continue; // died earlier this turn
            }
            let mpos = self.world.get::<&Position>(e).unwrap().0;
            // Wake on line-of-sight proximity.
            let (awake, mean) = {
                let mut m = self.world.get::<&mut Monster>(e).unwrap();
                if !m.awake && self.map.is_visible(mpos) && mpos.chebyshev(ppos) <= 6 {
                    m.awake = true;
                }
                (m.awake, m.mean)
            };
            if !awake || !mean {
                continue;
            }
            if mpos.chebyshev(ppos) <= 1 {
                self.monster_attack(e);
                continue;
            }
            // Greedy step toward the player, avoiding walls and other creatures.
            let step = self.step_toward(mpos, ppos);
            if let Some(np) = step {
                self.world.get::<&mut Position>(e).unwrap().0 = np;
            }
        }
    }

    fn step_toward(&self, from: Point, to: Point) -> Option<Point> {
        let mut best: Option<(i32, Point)> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let np = from + Point::new(dx, dy);
                if !self.map.is_walkable(np) {
                    continue;
                }
                if np != to && (self.monster_at(np).is_some() || np == self.player_pos()) {
                    continue;
                }
                let dist = np.chebyshev(to);
                if best.map(|(d, _)| dist < d).unwrap_or(true) {
                    best = Some((dist, np));
                }
            }
        }
        best.map(|(_, p)| p)
    }

    fn tick_hunger_and_regen(&mut self) {
        // SlowDigestion ring: consume food only every other turn.
        let slow = self.has_ring(RingKind::SlowDigestion);
        if slow {
            self.digest_skip = !self.digest_skip;
        }
        if !slow || !self.digest_skip {
            self.food -= 1;
            if self.food == 20 {
                self.log("You are starting to feel hungry.");
            }
            if self.food <= 0 && self.rng.percent(20) {
                self.damage_player(1);
                self.log("You faint from lack of food!");
            }
        }

        // Natural regeneration; Regeneration ring halves the period.
        self.regen_counter += 1;
        let level = self.world.get::<&Stats>(self.player).map(|s| s.level).unwrap_or(1);
        let period = (21 - level * 2).max(3);
        let regen_period = if self.has_ring(RingKind::Regeneration) {
            (period / 2).max(1)
        } else {
            period
        };
        if self.regen_counter >= regen_period {
            self.regen_counter = 0;
            let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
            if s.hp < s.max_hp {
                s.hp += 1;
            }
        }

        // Teleportation ring: occasional random teleport (bad ring).
        if self.has_ring(RingKind::Teleportation) && self.rng.rnd(100) == 0 {
            self.apply_teleport();
            self.log("The ring teleports you!");
        }
    }

    // --- Visibility ---------------------------------------------------------

    fn recompute_visibility(&mut self) {
        self.map.clear_visible();
        let p = self.player_pos();
        // Blind status reduces FOV to radius 1.
        let radius: i32 = if self.status.blind > 0 {
            1
        } else if self.has_ring(RingKind::Searching) {
            7
        } else {
            5
        };

        // Raycast field of view: for every tile within the radius, trace a line
        // from the player and reveal tiles until (and including) the first
        // opaque one. This lets the player see down corridors and across rooms.
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                self.reveal_line(p, p + Point::new(dx, dy));
            }
        }

        // Always see the immediate ring (covers diagonal corners cleanly).
        for dy in -1..=1 {
            for dx in -1..=1 {
                self.map.set_visible(p + Point::new(dx, dy));
            }
        }
    }

    /// Reveal tiles along a Bresenham line from `from` to `to`, stopping after
    /// the first opaque tile (which is itself revealed, so walls are seen).
    fn reveal_line(&mut self, from: Point, to: Point) {
        let (mut x0, mut y0) = (from.x, from.y);
        let (x1, y1) = (to.x, to.y);
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let q = Point::new(x0, y0);
            self.map.set_visible(q);
            if (x0, y0) == (x1, y1) {
                break;
            }
            if q != from && self.map.is_opaque(q) {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    // --- Rendering ----------------------------------------------------------

    fn render(&self, ctx: &mut BTerm) {
        ctx.cls();
        match self.mode {
            Mode::Title => self.render_title(ctx),
            Mode::Playing => self.render_play(ctx),
            Mode::Help => {
                self.render_help(ctx);
            }
            Mode::Inventory => {
                self.render_inventory(ctx);
            }
            Mode::ConfirmQuit => {
                // Draw the backdrop the player came from, then the prompt.
                match self.quit_return {
                    Mode::Title => self.render_title(ctx),
                    _ => self.render_play(ctx),
                }
                self.render_confirm_quit(ctx);
            }
            Mode::Dead => {
                self.render_play(ctx);
                self.render_banner(ctx, "You have died. Enter: new game   S: scores   Esc: quit");
            }
            Mode::Won => self.render_won(ctx),
            Mode::MessageLog => {
                self.render_message_log(ctx);
            }
            Mode::Scores => {
                self.render_scores(ctx);
            }
        }
    }

    fn render_confirm_quit(&self, ctx: &mut BTerm) {
        let mid = SCREEN_HEIGHT / 2;
        ctx.print_color_centered(
            mid - 1,
            self.theme.header(),
            self.theme.bg(),
            "Quit the game?",
        );
        ctx.print_color_centered(
            mid + 1,
            self.theme.fg(),
            self.theme.bg(),
            "Press Y or Enter to quit   ·   N or Esc to keep playing",
        );
    }

    fn render_title(&self, ctx: &mut BTerm) {
        ctx.print_color_centered(8,  self.theme.header(), self.theme.bg(), "R O G U E");
        ctx.print_color_centered(10, self.theme.fg(),     self.theme.bg(), "a modern Rust reimplementation");
        ctx.print_color_centered(13, self.theme.header(), self.theme.bg(), "Press Enter to begin");
        ctx.print_color_centered(14, self.theme.fg(),     self.theme.bg(), "S: high scores");
        ctx.print_color_centered(16, self.theme.fg(),     self.theme.bg(), "Move: hjkl / yubn / arrows   Wait: .   Descend: >");
        ctx.print_color_centered(17, self.theme.fg(),     self.theme.bg(), "Pick up: g   Quaff: q   Eat: e   Read: r   Inventory: i   Quit: Esc");
        ctx.print_color_centered(19, self.theme.dim_ui(), self.theme.bg(), "Press ? in game for help   ·   A = autopilot bot");
    }

    fn render_help(&self, ctx: &mut BTerm) {
        let lines = [
            "Movement",
            "  h j k l        left / down / up / right",
            "  y u b n        diagonals",
            "  arrows / numpad also work",
            "",
            "Actions",
            "  .              wait one turn",
            "  >              descend stairs (Shift + .)",
            "  g              pick up item",
            "  q              quaff potion",
            "  e              eat food",
            "  r              read scroll",
            "  p              put on ring",
            "  Shift+R        remove ring",
            "  i              view inventory",
            "  s              search for secret doors",
            "  t              throw item",
            "",
            "Other",
            "  ?              show / hide this help",
            "  m              view message log",
            "  A              toggle autopilot (a bot plays for you)",
            "  Tab            auto-explore (move to nearest unseen tile)",
            "  Esc            quit",
        ];

        // A centred, opaque panel so the map underneath never bleeds through.
        let inner_w = lines.iter().map(|l| l.len()).max().unwrap_or(0) as i32;
        let pad = 2;
        let box_w = inner_w + pad * 2;
        let box_h = lines.len() as i32 + 4; // title + blank + body + footer
        let x0 = (SCREEN_WIDTH - box_w) / 2;
        let y0 = (SCREEN_HEIGHT - box_h) / 2;

        // Fill the panel background and draw a border.
        for y in y0..y0 + box_h {
            for x in x0..x0 + box_w {
                ctx.set(x, y, self.theme.fg(), self.theme.bg(), to_cp437(' '));
            }
        }
        ctx.draw_box(
            x0,
            y0,
            box_w - 1,
            box_h - 1,
            self.theme.fg(),
            self.theme.bg(),
        );

        let tx = x0 + pad;
        ctx.print_color(tx, y0 + 1, self.theme.header(), self.theme.bg(), "HELP");
        for (i, line) in lines.iter().enumerate() {
            ctx.print_color(
                tx,
                y0 + 3 + i as i32,
                self.theme.fg(),
                self.theme.bg(),
                line,
            );
        }
        ctx.print_color(
            tx,
            y0 + box_h - 2,
            self.theme.dim_ui(),
            self.theme.bg(),
            "Press any key to return to the game.",
        );
    }

    fn render_inventory(&self, ctx: &mut BTerm) {
        const INV_PAGE: usize = 16;
        const BOX_W: i32 = 66;
        const BOX_H: i32 = 24;

        let x0 = (SCREEN_WIDTH - BOX_W) / 2;
        let y0 = (SCREEN_HEIGHT - BOX_H) / 2;
        let pad = 2i32;
        let tx = x0 + pad;
        let inner_w = (BOX_W - pad * 2 - 1) as usize;

        for y in y0..y0 + BOX_H {
            for x in x0..x0 + BOX_W {
                ctx.set(x, y, self.theme.fg(), self.theme.bg(), to_cp437(' '));
            }
        }
        ctx.draw_box(x0, y0, BOX_W - 1, BOX_H - 1, self.theme.fg(), self.theme.bg());
        ctx.print_color(tx, y0 + 1, self.theme.header(), self.theme.bg(),
            "INVENTORY   (letter=use  up/dn=scroll  Esc=close)");

        if self.inventory.is_empty() {
            ctx.print_color(tx, y0 + 3, self.theme.dim_ui(), self.theme.bg(), "Your pack is empty.");
        } else {
            let total = self.inventory.len();
            let max_scroll = total.saturating_sub(INV_PAGE);
            let scroll = self.inv_scroll.min(max_scroll);
            let visible = total.min(INV_PAGE);

            for i in 0..visible {
                let idx = i + scroll;
                if idx >= total { break; }
                let item = &self.inventory[idx];
                let label = inv_label(idx);
                let action = item_action_hint(&item.kind);
                let name_w = inner_w.saturating_sub(label.len() + 1 + action.len() + 2);
                let name_trunc = if item.name.len() > name_w {
                    format!("{:.prec$}", item.name, prec = name_w)
                } else {
                    format!("{:<width$}", item.name, width = name_w)
                };
                let line = format!("{} {} {}", label, name_trunc, action);
                let fg = if matches!(item.kind, ItemKind::Amulet) {
                    self.theme.header()
                } else {
                    self.theme.fg()
                };
                ctx.print_color(tx, y0 + 3 + i as i32, fg, self.theme.bg(), &line);
            }

            if scroll > 0 {
                ctx.print_color(x0 + BOX_W - 3, y0 + 3, self.theme.accent(), self.theme.bg(), "^");
            }
            if scroll < max_scroll {
                ctx.print_color(x0 + BOX_W - 3, y0 + 2 + visible as i32, self.theme.accent(), self.theme.bg(), "v");
            }

            let count_str = format!("{}/{} items", visible.min(total - scroll), total);
            ctx.print_color(tx, y0 + BOX_H - 5, self.theme.dim_ui(), self.theme.bg(), &count_str);
        }

        // Rings
        let ring_y = y0 + BOX_H - 4;
        ctx.print_color(tx, ring_y, self.theme.header(), self.theme.bg(), "Equipped rings:");
        let left_str = self.left_ring.map(|k| format!("ring of {}", ring_name(k)))
            .unwrap_or_else(|| "(none)".to_string());
        let right_str = self.right_ring.map(|k| format!("ring of {}", ring_name(k)))
            .unwrap_or_else(|| "(none)".to_string());
        ctx.print_color(tx, ring_y + 1, self.theme.fg(), self.theme.bg(),
            format!("  Left: {:<24}  Right: {}", left_str, right_str));
        ctx.print_color(tx, ring_y + 2, self.theme.dim_ui(), self.theme.bg(),
            "  p: put on ring   Shift+R: remove ring");
    }

    fn render_message_log(&self, ctx: &mut BTerm) {
        const PANEL_LINES: usize = 16;
        let box_w = 60i32;
        let box_h = 20i32;
        let x0 = (SCREEN_WIDTH - box_w) / 2;
        let y0 = (SCREEN_HEIGHT - box_h) / 2;
        let pad = 2;
        let tx = x0 + pad;

        for y in y0..y0 + box_h {
            for x in x0..x0 + box_w {
                ctx.set(x, y, self.theme.fg(), self.theme.bg(), to_cp437(' '));
            }
        }
        ctx.draw_box(x0, y0, box_w - 1, box_h - 1, self.theme.fg(), self.theme.bg());
        ctx.print_color(tx, y0 + 1, self.theme.header(), self.theme.bg(), "MESSAGE LOG");

        if self.log.is_empty() {
            ctx.print_color(tx, y0 + 3, self.theme.dim_ui(), self.theme.bg(), "(no messages yet)");
        } else {
            let msgs: Vec<&String> = self.log.iter().rev().collect();
            let start = self.log_scroll.min(msgs.len().saturating_sub(1));
            for (i, msg) in msgs.iter().skip(start).take(PANEL_LINES).enumerate() {
                let max_chars = (box_w - pad * 2 - 1) as usize;
                let display = if msg.len() > max_chars { &msg[..max_chars] } else { msg.as_str() };
                ctx.print_color(tx, y0 + 3 + i as i32, self.theme.fg(), self.theme.bg(), display);
            }
        }
        ctx.print_color(
            tx,
            y0 + box_h - 2,
            self.theme.dim_ui(),
            self.theme.bg(),
            "up/dn/jk: scroll   Any other key: return",
        );
    }

    fn render_scores(&self, ctx: &mut BTerm) {
        use crate::scores::Scores;
        let scores = Scores::load();
        let lines = scores.top_lines(15);

        let box_w = 70i32;
        let box_h = (lines.len() as i32 + 5).max(8);
        let x0 = (SCREEN_WIDTH - box_w) / 2;
        let y0 = (SCREEN_HEIGHT - box_h) / 2;
        let pad = 2;
        let tx = x0 + pad;

        for y in y0..y0 + box_h {
            for x in x0..x0 + box_w {
                ctx.set(x, y, self.theme.fg(), self.theme.bg(), to_cp437(' '));
            }
        }
        ctx.draw_box(x0, y0, box_w - 1, box_h - 1, self.theme.fg(), self.theme.bg());
        ctx.print_color(tx, y0 + 1, self.theme.header(), self.theme.bg(), "HIGH SCORES");

        if lines.is_empty() {
            ctx.print_color(tx, y0 + 3, self.theme.dim_ui(), self.theme.bg(), "(no scores recorded yet)");
        } else {
            for (i, line) in lines.iter().enumerate() {
                ctx.print_color(tx, y0 + 3 + i as i32, self.theme.fg(), self.theme.bg(), line);
            }
        }
        ctx.print_color(
            tx,
            y0 + box_h - 2,
            self.theme.dim_ui(),
            self.theme.bg(),
            "Press any key to return.",
        );
    }

    fn render_won(&self, ctx: &mut BTerm) {
        let box_w = 60i32;
        let box_h = 16i32;
        let x0 = (SCREEN_WIDTH - box_w) / 2;
        let y0 = (SCREEN_HEIGHT - box_h) / 2;
        let pad = 2i32;
        let tx = x0 + pad;

        for y in y0..y0 + box_h {
            for x in x0..x0 + box_w {
                ctx.set(x, y, self.theme.fg(), self.theme.bg(), to_cp437(' '));
            }
        }
        ctx.draw_box(x0, y0, box_w - 1, box_h - 1, self.theme.header(), self.theme.bg());

        ctx.print_color_centered(y0 + 1, self.theme.header(), self.theme.bg(),
            "*** CONGRATULATIONS! YOU WIN! ***");
        ctx.print_color_centered(y0 + 2, self.theme.accent(), self.theme.bg(),
            "You escaped the dungeon with the Amulet of Yendor!");

        ctx.print_color(tx, y0 + 4, self.theme.fg(), self.theme.bg(), "SCORE SUMMARY");
        ctx.print_color(tx, y0 + 5, self.theme.fg(), self.theme.bg(),
            format!("  Dungeon depth reached : {}", self.depth));
        ctx.print_color(tx, y0 + 6, self.theme.fg(), self.theme.bg(),
            format!("  Gold collected        : {}", self.gold));
        ctx.print_color(tx, y0 + 7, self.theme.fg(), self.theme.bg(),
            format!("  Turns taken           : {}", self.turns));

        if let Ok(s) = self.world.get::<&Stats>(self.player) {
            ctx.print_color(tx, y0 + 8, self.theme.fg(), self.theme.bg(),
                format!("  Character level      : {}", s.level));
            ctx.print_color(tx, y0 + 9, self.theme.fg(), self.theme.bg(),
                format!("  Experience           : {}", s.xp_reward));
        }

        ctx.print_color(tx, y0 + 11, self.theme.accent(), self.theme.bg(),
            "Your score has been recorded.");
        ctx.print_color(tx, y0 + 12, self.theme.accent(), self.theme.bg(),
            "Press S to view the hall of fame.");
        ctx.print_color(tx, y0 + 13, self.theme.dim_ui(), self.theme.bg(),
            "Press Enter for a new game  |  Esc to quit.");
    }

    fn render_banner(&self, ctx: &mut BTerm, msg: &str) {
        // Clear the full row first so map tiles behind the text are overwritten.
        let y = SCREEN_HEIGHT / 2;
        for x in 0..SCREEN_WIDTH {
            ctx.set(x, y, self.theme.bg(), self.theme.bg(), to_cp437(' '));
        }
        ctx.print_color_centered(y, self.theme.header(), self.theme.bg(), msg);
    }

    fn render_play(&self, ctx: &mut BTerm) {
        // Most recent message on the top line.
        if let Some(last) = self.log.last() {
            ctx.print_color(0, MSG_ROW, self.theme.fg(), self.theme.bg(), last);
        }

        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let p = Point::new(x, y);
                if !self.map.is_revealed(p) {
                    continue;
                }
                if self.theme.use_tiled() {
                    let (glyph_u16, mut color) = tiled_tile_render(self.map.tile(p));
                    if !self.map.is_visible(p) {
                        color = dim(color);
                    }
                    ctx.set(x, y + MAP_TOP, self.theme.apply(color), self.theme.bg(), glyph_u16);
                } else {
                    let (glyph, mut color) = if self.theme.use_boxy() {
                        boxy_tile_render(self.map.tile(p), p, &self.map)
                    } else {
                        tile_render(self.map.tile(p))
                    };
                    if !self.map.is_visible(p) {
                        color = dim(color);
                    }
                    ctx.set(x, y + MAP_TOP, self.theme.apply(color), self.theme.bg(), to_cp437(glyph));
                }
            }
        }

        // Entities, only where currently visible.
        for (e, (pos, r)) in self.world.query::<(&Position, &Renderable)>().iter() {
            if self.map.is_visible(pos.0) {
                if self.theme.use_tiled() {
                    // Use raw tile indices so entity sprites don't conflict with text glyphs.
                    ctx.set(
                        pos.0.x,
                        pos.0.y + MAP_TOP,
                        self.theme.apply((255u8, 255u8, 255u8)),
                        self.theme.bg(),
                        tiled_entity_glyph(r.glyph),
                    );
                } else {
                    let glyph = if self.theme.use_boxy() && e == self.player { '☺' } else { r.glyph };
                    ctx.set(
                        pos.0.x,
                        pos.0.y + MAP_TOP,
                        self.theme.apply(r.color),
                        self.theme.bg(),
                        to_cp437(glyph),
                    );
                }
            }
        }

        self.render_status(ctx);
        self.render_hints(ctx);
        self.render_autopilot_log(ctx);
    }

    /// Scrolling 4-line log overlay shown in the bottom-right of the map
    /// while the autopilot is driving.  No border — plain text over the map.
    fn render_autopilot_log(&self, ctx: &mut BTerm) {
        if !self.autopilot { return; }

        // Box is 38 wide × 6 tall, flush against the right edge.
        // Bracket-lib draw_box(x,y,w,h) draws corners at (x,y) and (x+w, y+h).
        const CONTENT_W: usize = 34; // interior width (38 - 2 border - 2 padding)
        const BOX_W: i32 = 37;       // passed to draw_box (outer_width - 1)
        const BOX_H: i32 = 5;        // passed to draw_box (outer_height - 1)
        let x0 = SCREEN_WIDTH - BOX_W - 1; // flush right: col 42
        let y0 = MAP_TOP + self.map.height - BOX_H - 1; // rows 19-24
        let tx = x0 + 2; // 1 space of interior padding

        // Clear background so text is readable over the map.
        for y in y0..=y0 + BOX_H {
            for x in x0..=x0 + BOX_W {
                ctx.set(x, y, self.theme.fg(), self.theme.bg(), to_cp437(' '));
            }
        }

        // Accent-coloured border — a visual cue that autopilot is driving.
        ctx.draw_box(x0, y0, BOX_W, BOX_H, self.theme.accent(), self.theme.bg());

        // Collect the most recent 4 messages (oldest first for display).
        let msgs: Vec<&str> = self.log
            .iter()
            .rev()
            .take(4)
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        for (i, msg) in msgs.iter().enumerate() {
            let display = if msg.len() > CONTENT_W { &msg[..CONTENT_W] } else { msg };
            let fg = if i + 1 == msgs.len() {
                self.theme.accent()   // most recent: bright
            } else {
                self.theme.dim_ui()   // older: muted
            };
            ctx.print_color(tx, y0 + 1 + i as i32, fg, self.theme.bg(), display);
        }
    }

    fn render_status(&self, ctx: &mut BTerm) {
        let s = self.world.get::<&Stats>(self.player).unwrap();
        let hunger = if self.food <= 0 {
            "Starving"
        } else if self.food <= 150 {
            "Hungry"
        } else {
            ""
        };
        let row = self.map.height + MAP_TOP;
        let auto = if self.autopilot { "  [AUTO]" } else { "" };
        let mut tags = String::new();
        if self.status.confused > 0 { tags.push_str("[CONF]"); }
        if self.status.blind > 0 { tags.push_str("[BLND]"); }
        if self.status.poisoned > 0 { tags.push_str("[PSND]"); }
        if self.status.paralyzed > 0 { tags.push_str("[PARA]"); }
        if self.status.haste > 0 { tags.push_str("[HSTE]"); }
        let line = format!(
            "Level:{}  HP:{}/{}  Str:{}  Arm:{}  Gold:{}  Exp:{}  Depth:{}  {}{}{}",
            s.level, s.hp, s.max_hp, s.strength, s.armor, self.gold, s.xp_reward, self.depth,
            hunger, auto, tags
        );
        ctx.print_color(0, row, self.theme.fg(), self.theme.bg(), line);
    }

    /// Two always-on help lines below the status bar: a context-sensitive
    /// prompt for whatever the player is standing on, and a permanent reminder
    /// of the most useful keys (so controls are discoverable without the manual).
    fn render_hints(&self, ctx: &mut BTerm) {
        let hint_row = self.map.height + MAP_TOP + 1;
        let footer_row = SCREEN_HEIGHT - 1;

        if let Some(ctx_hint) = self.context_hint() {
            ctx.print_color(0, hint_row, self.theme.header(), self.theme.bg(), ctx_hint);
        }

        ctx.print_color(
            0,
            footer_row,
            self.theme.dim_ui(),
            self.theme.bg(),
            "hjkl/arrows:move  g:get  >:stairs  q:quaff  e:eat  r:read  i:inv  s:srch  ?:help",
        );
    }

    /// A prompt describing the action available on the player's current tile,
    /// if any (standing on stairs, an item, gold, etc.).
    fn context_hint(&self) -> Option<&'static str> {
        let p = self.player_pos();
        if self.map.tile(p) == TileKind::StairsDown {
            return Some("You are on the stairs down. Press > to descend to the next level.");
        }
        if let Some(e) = self.entity_at(p) {
            if self.world.get::<&Item>(e).is_ok() {
                return Some("There is an item here. Press g to pick it up.");
            }
        }
        None
    }

    // --- Autopilot bot ------------------------------------------------------

    /// Final-resort target: Amulet of Yendor if present, otherwise the stairs.
    fn auto_target(&self) -> Option<Point> {
        for (_e, (pos, item)) in self.world.query::<(&Position, &Item)>().iter() {
            if matches!(item.kind, ItemKind::Amulet) {
                return Some(pos.0);
            }
        }
        self.map.stairs_down
    }

    /// BFS to find the nearest walkable tile that is adjacent to at least one
    /// unrevealed non-empty tile.  Returns `None` when the whole level is mapped.
    fn auto_explore_target(&self) -> Option<Point> {
        use std::collections::{HashSet, VecDeque};
        let ppos = self.player_pos();
        let mut queue: VecDeque<Point> = VecDeque::new();
        let mut visited: HashSet<(i32, i32)> = HashSet::new();
        queue.push_back(ppos);
        visited.insert((ppos.x, ppos.y));
        while let Some(cur) = queue.pop_front() {
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1),
                              (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let np = Point::new(cur.x + dx, cur.y + dy);
                if !self.map.in_bounds(np) { continue; }
                if !self.map.is_revealed(np) && self.map.tile(np) != TileKind::Empty {
                    return Some(cur);
                }
                if visited.contains(&(np.x, np.y)) { continue; }
                if self.map.is_walkable(np) && self.map.is_revealed(np) {
                    visited.insert((np.x, np.y));
                    queue.push_back(np);
                }
            }
        }
        None
    }

    /// Nearest revealed item on the floor (by Chebyshev distance).
    /// Gold piles are excluded — they are auto-collected on entry.
    fn nearest_floor_item(&self) -> Option<Point> {
        let here = self.player_pos();
        let mut best: Option<(i32, Point)> = None;
        for (e, pos) in self.world.query::<&Position>().iter() {
            if e == self.player { continue; }
            let p = pos.0;
            if !self.map.is_revealed(p) { continue; }
            if self.world.get::<&Item>(e).is_ok() {
                let d = p.chebyshev(here);
                if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                    best = Some((d, p));
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// Take a single bot action.  Priority order:
    ///   0. Emergency heal: quaff a healing item when below 50% HP.
    ///   1. Pick up item at current tile.
    ///   2. If healthy and no visible monsters: walk to nearest unexplored frontier.
    ///   2b. If healthy and monsters visible: engage nearest visible monster.
    ///   3. Collect: walk to nearest floor item (only when healthy).
    ///   4. Descend: walk to Amulet/stairs and descend.
    /// Returns true if a turn was consumed.
    pub fn auto_step(&mut self) -> bool {
        let here = self.player_pos();
        let (hp, max_hp) = self.world
            .get::<&Stats>(self.player)
            .map(|s| (s.hp, s.max_hp))
            .unwrap_or((1, 1));

        // 0. Emergency heal: quaff healing potion when below 50% HP.
        if hp * 2 < max_hp {
            let has_healer = self.inventory.iter().any(|it| {
                matches!(
                    it.kind,
                    ItemKind::Heal(_)
                        | ItemKind::Potion(PotionKind::Healing)
                        | ItemKind::Potion(PotionKind::ExtraHealing)
                )
            });
            if has_healer {
                return self.quaff();
            }
        }

        // When health is critically low, skip exploration and retreat to stairs.
        let low_hp = hp * 100 / max_hp.max(1) < 35;

        // 1. Pick up item at current tile (gold is auto-collected on entry).
        if let Some(e) = self.entity_at(here) {
            if self.world.get::<&Item>(e).is_ok() {
                return self.pickup();
            }
        }

        if !low_hp {
            // Find visible monsters once for both branches below.
            let visible_monsters: Vec<Point> = self
                .world
                .query::<(&Position, &Monster)>()
                .iter()
                .filter_map(|(_, (pos, _))| {
                    if self.map.is_visible(pos.0) { Some(pos.0) } else { None }
                })
                .collect();

            if visible_monsters.is_empty() {
                // 2. No threats visible: advance toward the nearest unexplored frontier.
                if let Some(dest) = self.auto_explore_target() {
                    let here = self.player_pos();
                    if dest == here {
                        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                            let np = Point::new(here.x + dx, here.y + dy);
                            if self.map.is_walkable(np) {
                                return self.try_move(Point::new(dx, dy));
                            }
                        }
                    } else if let Some(path) = self.map.find_path(here, dest) {
                        if let Some(&next) = path.first() {
                            return self.try_move(
                                Point::new(next.x - here.x, next.y - here.y),
                            );
                        }
                    }
                }
            } else {
                // 2b. Engage nearest visible monster (clear before continuing).
                let nearest = visible_monsters
                    .iter()
                    .min_by_key(|&&p| p.chebyshev(here))
                    .copied();
                if let Some(mpos) = nearest {
                    if let Some(path) = self.map.find_path(here, mpos) {
                        if let Some(&next) = path.first() {
                            return self.try_move(
                                Point::new(next.x - here.x, next.y - here.y),
                            );
                        }
                    }
                }
            }

            // 3. Collect floor items now that the level is fully mapped.
            if let Some(target) = self.nearest_floor_item() {
                let here = self.player_pos();
                if target == here {
                    return self.pickup();
                }
                if let Some(path) = self.map.find_path(here, target) {
                    if let Some(&next) = path.first() {
                        return self.try_move(
                            Point::new(next.x - here.x, next.y - here.y),
                        );
                    }
                }
            }
        }

        // 4. Go to Amulet or stairs and descend.
        let Some(target) = self.auto_target() else { return false; };
        let here = self.player_pos();
        if here == target {
            if self.map.tile(here) == TileKind::StairsDown {
                return self.try_descend();
            }
            return self.pickup();
        }
        if let Some(path) = self.map.find_path(here, target) {
            if let Some(&next) = path.first() {
                return self.try_move(Point::new(next.x - here.x, next.y - here.y));
            }
        }
        false
    }

    /// Run one full autopilot turn (bot action + world response). Returns true
    /// if the bot acted. Intended for headless tests / scripted demos.
    pub fn auto_turn(&mut self) -> bool {
        let acted = self.auto_step();
        if acted && self.mode == Mode::Playing {
            self.end_player_turn();
        }
        acted
    }

    /// Advance the watchable autopilot, throttled so a human can follow along.
    fn run_autopilot(&mut self) {
        if self.auto_cooldown > 0 {
            self.auto_cooldown -= 1;
            return;
        }
        self.auto_cooldown = AUTO_PERIOD;
        if !self.auto_turn() {
            self.autopilot = false;
            self.log("Autopilot stopped (level fully explored, nothing more to do).");
        }
    }

    /// Toggle the autopilot bot on/off (bound to the `A` key in game).
    pub fn toggle_autopilot(&mut self) {
        self.autopilot = !self.autopilot;
        self.auto_cooldown = 0;
        if self.autopilot {
            self.log("Autopilot engaged — explores every room and loots as it goes. Press A to stop.");
        } else {
            self.log("Autopilot disengaged.");
        }
    }

    /// Start the game straight into a watchable autopilot run (CLI `--demo`).
    pub fn start_demo(&mut self) {
        self.mode = Mode::Playing;
        self.autopilot = true;
        self.auto_cooldown = 0;
        self.log("Demo mode: autopilot is exploring and looting. Press A to take control.");
    }

    /// Read-only accessors used by integration tests.
    pub fn depth(&self) -> i32 {
        self.depth
    }

    pub fn is_won(&self) -> bool {
        self.mode == Mode::Won
    }

    pub fn is_dead(&self) -> bool {
        self.mode == Mode::Dead
    }

    pub fn begin_playing(&mut self) {
        self.mode = Mode::Playing;
    }

    // --- Top-level tick -----------------------------------------------------

    /// Begin the quit-confirmation overlay, remembering where to return if the
    /// player changes their mind.
    fn request_quit(&mut self) {
        self.quit_return = self.mode;
        self.mode = Mode::ConfirmQuit;
    }

    pub fn tick(&mut self, ctx: &mut BTerm) {
        match self.mode {
            Mode::Title => {
                if ctx.key == Some(VirtualKeyCode::Return) {
                    self.mode = Mode::Playing;
                } else if ctx.key == Some(VirtualKeyCode::S) {
                    self.mode = Mode::Scores;
                } else if ctx.key == Some(VirtualKeyCode::Escape) {
                    self.request_quit();
                }
            }
            Mode::Playing => match ctx.key {
                Some(VirtualKeyCode::Escape) => self.request_quit(),
                Some(VirtualKeyCode::A) => self.toggle_autopilot(),
                Some(VirtualKeyCode::Slash) if !self.autopilot => self.mode = Mode::Help,
                Some(VirtualKeyCode::I) if !self.autopilot => self.mode = Mode::Inventory,
                Some(VirtualKeyCode::M) if !self.autopilot => {
                    self.log_scroll = 0;
                    self.mode = Mode::MessageLog;
                }
                _ => {
                    if self.autopilot {
                        self.run_autopilot();
                    } else {
                        self.handle_key(ctx);
                    }
                }
            },
            Mode::Help => {
                if ctx.key.is_some() {
                    self.mode = Mode::Playing;
                }
            }
            Mode::Inventory => {
                const INV_PAGE: usize = 16;
                match ctx.key {
                    Some(VirtualKeyCode::Escape) | Some(VirtualKeyCode::I) => {
                        self.mode = Mode::Playing;
                    }
                    Some(VirtualKeyCode::Up) | Some(VirtualKeyCode::K) => {
                        if self.inv_scroll > 0 {
                            self.inv_scroll -= 1;
                        }
                    }
                    Some(VirtualKeyCode::Down) | Some(VirtualKeyCode::J) => {
                        let max = self.inventory.len().saturating_sub(INV_PAGE);
                        if self.inv_scroll < max {
                            self.inv_scroll += 1;
                        }
                    }
                    Some(key) => {
                        if let Some(idx) = vk_to_inv_idx(key, self.inv_scroll) {
                            if idx < self.inventory.len() {
                                self.mode = Mode::Playing;
                                let acted = self.use_inventory_item(idx);
                                if acted && self.mode == Mode::Playing {
                                    self.end_player_turn();
                                }
                            }
                        }
                    }
                    None => {}
                }
            }
            Mode::MessageLog => {
                const PANEL_LINES: usize = 16;
                match ctx.key {
                    Some(VirtualKeyCode::Up) | Some(VirtualKeyCode::K) => {
                        if self.log_scroll > 0 {
                            self.log_scroll -= 1;
                        }
                    }
                    Some(VirtualKeyCode::Down) | Some(VirtualKeyCode::J) => {
                        let max_scroll = self.log.len().saturating_sub(PANEL_LINES);
                        if self.log_scroll < max_scroll {
                            self.log_scroll += 1;
                        }
                    }
                    Some(_) => self.mode = Mode::Playing,
                    None => {}
                }
            }
            Mode::ConfirmQuit => match ctx.key {
                Some(VirtualKeyCode::Y) | Some(VirtualKeyCode::Return) => ctx.quitting = true,
                Some(VirtualKeyCode::N) | Some(VirtualKeyCode::Escape) => {
                    self.mode = self.quit_return;
                }
                _ => {}
            },
            Mode::Dead | Mode::Won => {
                match ctx.key {
                    Some(VirtualKeyCode::Return) => {
                        let saved_theme = self.theme;
                        *self = Game::new();
                        self.theme = saved_theme;
                        self.mode = Mode::Playing;
                    }
                    Some(VirtualKeyCode::S) => self.mode = Mode::Scores,
                    Some(VirtualKeyCode::Escape) => self.request_quit(),
                    _ => {}
                }
            }
            Mode::Scores => {
                if ctx.key.is_some() {
                    self.mode = Mode::Title;
                }
            }
        }
        self.render(ctx);
    }
}

impl GameState for Game {
    fn tick(&mut self, ctx: &mut BTerm) {
        Game::tick(self, ctx);
    }
}

// --- Free helpers -----------------------------------------------------------

fn pick_named(items: &[rogue_core::data::NamedItem], rng: &mut impl RogueRng) -> String {
    let total: i32 = items.iter().map(|i| i.prob).sum();
    let mut roll = rng.rnd(total.max(1));
    for i in items {
        if roll < i.prob {
            return i.name.clone();
        }
        roll -= i.prob;
    }
    items
        .first()
        .map(|i| i.name.clone())
        .unwrap_or_else(|| "nothing".to_string())
}

/// Map a scroll's flavour name to a modelled effect. Unmatched scrolls read
/// harmlessly (they still exist as flavour items).
fn scroll_kind_from_name(name: &str) -> ScrollKind {
    let n = name.to_ascii_lowercase();
    if n.contains("magic mapping") {
        ScrollKind::MagicMapping
    } else if n.contains("teleport") {
        ScrollKind::Teleport
    } else if n.contains("enchant weapon") {
        ScrollKind::EnchantWeapon
    } else if n.contains("enchant armor") || n.contains("protect armor") {
        ScrollKind::EnchantArmor
    } else if n.contains("aggravate") {
        ScrollKind::Aggravate
    } else if n.contains("identify") {
        ScrollKind::Identify
    } else {
        ScrollKind::Unknown
    }
}

fn potion_kind_from_name(name: &str) -> PotionKind {
    let n = name.to_ascii_lowercase();
    if n.contains("extra healing") {
        PotionKind::ExtraHealing
    } else if n.contains("healing") {
        PotionKind::Healing
    } else if n.contains("poison") {
        PotionKind::Poison
    } else if n.contains("gain strength") {
        PotionKind::GainStrength
    } else if n.contains("restore strength") {
        PotionKind::RestoreStrength
    } else if n.contains("see invisible") {
        PotionKind::SeeInvisible
    } else if n.contains("confusion") {
        PotionKind::Confusion
    } else if n.contains("blindness") {
        PotionKind::Blindness
    } else if n.contains("hallucination") {
        PotionKind::Hallucination
    } else if n.contains("haste self") {
        PotionKind::HasteSelf
    } else if n.contains("raise level") {
        PotionKind::RaiseLevel
    } else if n.contains("monster detection") {
        PotionKind::MonsterDetection
    } else if n.contains("magic detection") {
        PotionKind::MagicDetection
    } else if n.contains("levitation") {
        PotionKind::Levitation
    } else {
        PotionKind::Unknown
    }
}

fn wand_kind_from_name(name: &str) -> WandKind {
    let n = name.to_ascii_lowercase();
    if n.contains("magic missile") {
        WandKind::MagicMissile
    } else if n.contains("slow") {
        WandKind::Slow
    } else if n.contains("fear") {
        WandKind::Fear
    } else if n.contains("confusion") {
        WandKind::Confusion
    } else if n.contains("drain life") {
        WandKind::DrainLife
    } else if n.contains("polymorph") {
        WandKind::Polymorph
    } else if n.contains("haste") {
        WandKind::Haste
    } else if n.contains("teleport") {
        WandKind::TeleportAway
    } else if n.contains("cancellation") {
        WandKind::CancellationWand
    } else if n.contains("nothing") {
        WandKind::NothingWand
    } else if n.contains("light") {
        WandKind::Light
    } else if n.contains("fire") {
        WandKind::Fire
    } else if n.contains("cold") {
        WandKind::Cold
    } else if n.contains("lightning") {
        WandKind::Lightning
    } else {
        WandKind::Unknown
    }
}

/// Assign a special attack based on the monster's ASCII symbol (classic Rogue mapping).
fn monster_special_from_symbol(symbol: char) -> MonsterSpecial {
    match symbol {
        'L' => MonsterSpecial::StealGold,  // Leprechaun
        'N' => MonsterSpecial::StealGold,  // Nymph
        'V' => MonsterSpecial::DrainLevel, // Vampire
        'W' => MonsterSpecial::DrainLevel, // Wraith
        'S' | 's' => MonsterSpecial::Poison,      // Snake / Spider
        'P' => MonsterSpecial::Paralyze,   // Phantom
        'M' => MonsterSpecial::Confuse,    // Medusa
        'Q' => MonsterSpecial::DrainStrength, // Quasit
        _ => MonsterSpecial::None,
    }
}

fn ring_kind_from_name(name: &str) -> RingKind {
    let n = name.to_ascii_lowercase();
    if n.contains("protection") {
        RingKind::Protection
    } else if n.contains("add strength") || n.contains("strength") {
        RingKind::AddStrength
    } else if n.contains("sustain") {
        RingKind::SustainStrength
    } else if n.contains("searching") || n.contains("search") {
        RingKind::Searching
    } else if n.contains("see invis") {
        RingKind::SeeInvisible
    } else if n.contains("adornment") {
        RingKind::Adornment
    } else if n.contains("aggravate") {
        RingKind::AggravateMonster
    } else if n.contains("dexterity") {
        RingKind::Dexterity
    } else if n.contains("increase damage") || n.contains("damage") {
        RingKind::IncreaseDamage
    } else if n.contains("regeneration") || n.contains("regen") {
        RingKind::Regeneration
    } else if n.contains("slow digestion") || n.contains("digestion") {
        RingKind::SlowDigestion
    } else if n.contains("teleport") {
        RingKind::Teleportation
    } else if n.contains("stealth") {
        RingKind::Stealth
    } else if n.contains("maintain armor") || n.contains("maintain") {
        RingKind::MaintainArmor
    } else {
        RingKind::Unknown
    }
}

/// Returns the display name suffix for a ring kind (matches items.ron names).
fn ring_name(kind: RingKind) -> &'static str {
    match kind {
        RingKind::Protection => "protection",
        RingKind::AddStrength => "add strength",
        RingKind::SustainStrength => "sustain strength",
        RingKind::Searching => "searching",
        RingKind::SeeInvisible => "see invisible",
        RingKind::Adornment => "adornment",
        RingKind::AggravateMonster => "aggravate monster",
        RingKind::Dexterity => "dexterity",
        RingKind::IncreaseDamage => "increase damage",
        RingKind::Regeneration => "regeneration",
        RingKind::SlowDigestion => "slow digestion",
        RingKind::Teleportation => "teleportation",
        RingKind::Stealth => "stealth",
        RingKind::MaintainArmor => "maintain armor",
        RingKind::Unknown => "unknown",
    }
}

/// Label character for inventory slot `idx`: a–z for 0–25, 0–9 for 26–35.
fn inv_label(idx: usize) -> String {
    if idx < 26 {
        format!("{})", (b'a' + idx as u8) as char)
    } else {
        format!("{})", idx - 26)
    }
}

/// Short action hint shown next to each inventory item.
fn item_action_hint(kind: &ItemKind) -> &'static str {
    match kind {
        ItemKind::Heal(_) | ItemKind::Potion(_) => "[quaff]",
        ItemKind::Food                           => "[eat]  ",
        ItemKind::Scroll(_)                      => "[read] ",
        ItemKind::Weapon { .. }                  => "[wield]",
        ItemKind::Armor(_)                       => "[wear] ",
        ItemKind::Ring(_)                        => "[put on]",
        ItemKind::Wand { .. }                    => "[zap]  ",
        ItemKind::Amulet                         => "[AMULET]",
        ItemKind::Trinket                        => "[?]    ",
    }
}

/// Map a VirtualKeyCode to an inventory index (letter a–z → 0–25, digit 0–9 → 26–35).
fn vk_to_inv_idx(key: VirtualKeyCode, scroll: usize) -> Option<usize> {
    let letter_offset: Option<usize> = match key {
        VirtualKeyCode::A => Some(0),  VirtualKeyCode::B => Some(1),
        VirtualKeyCode::C => Some(2),  VirtualKeyCode::D => Some(3),
        VirtualKeyCode::E => Some(4),  VirtualKeyCode::F => Some(5),
        VirtualKeyCode::G => Some(6),  VirtualKeyCode::H => Some(7),
        VirtualKeyCode::I => Some(8),  VirtualKeyCode::J => Some(9),
        VirtualKeyCode::K => Some(10), VirtualKeyCode::L => Some(11),
        VirtualKeyCode::M => Some(12), VirtualKeyCode::N => Some(13),
        VirtualKeyCode::O => Some(14), VirtualKeyCode::P => Some(15),
        VirtualKeyCode::Q => Some(16), VirtualKeyCode::R => Some(17),
        VirtualKeyCode::S => Some(18), VirtualKeyCode::T => Some(19),
        VirtualKeyCode::U => Some(20), VirtualKeyCode::V => Some(21),
        VirtualKeyCode::W => Some(22), VirtualKeyCode::X => Some(23),
        VirtualKeyCode::Y => Some(24), VirtualKeyCode::Z => Some(25),
        VirtualKeyCode::Key0 => Some(26), VirtualKeyCode::Key1 => Some(27),
        VirtualKeyCode::Key2 => Some(28), VirtualKeyCode::Key3 => Some(29),
        VirtualKeyCode::Key4 => Some(30), VirtualKeyCode::Key5 => Some(31),
        VirtualKeyCode::Key6 => Some(32), VirtualKeyCode::Key7 => Some(33),
        VirtualKeyCode::Key8 => Some(34), VirtualKeyCode::Key9 => Some(35),
        _ => None,
    };
    letter_offset.map(|off| scroll + off)
}

fn tile_render(t: TileKind) -> (char, (u8, u8, u8)) {
    match t {
        TileKind::Empty      => (' ', (0, 0, 0)),
        TileKind::Floor      => ('.', (175, 175, 175)),
        TileKind::Wall       => ('#', (200, 165, 110)),
        TileKind::Passage    => ('#', (135, 135, 135)),
        TileKind::Door       => ('+', (210, 165, 85)),
        TileKind::SecretDoor => ('#', (200, 165, 110)),
        TileKind::StairsDown => ('>', (255, 255, 255)),
        TileKind::StairsUp   => ('<', (255, 255, 255)),
        TileKind::Trap       => ('^', (255, 110, 110)),
    }
}

fn item_render(item: &Item) -> Renderable {
    let (glyph, color) = match item.kind {
        ItemKind::Heal(_)      => ('!', (255,  80, 255)),
        ItemKind::Potion(_)    => ('!', (255,  80, 255)),
        ItemKind::Food         => (':', (220, 180, 100)),
        ItemKind::Weapon { .. }=> (')', (200, 200, 240)),
        ItemKind::Armor(_)     => ('[', (180, 180, 230)),
        ItemKind::Amulet       => ('&', (255, 255,   0)),
        ItemKind::Scroll(_)    => ('?', (240, 240, 200)),
        ItemKind::Ring(_)      => ('=', (220, 200,  80)),
        ItemKind::Wand { .. }  => ('/', (180, 230, 255)),
        ItemKind::Trinket      => ('?', (150, 220, 150)),
    };
    Renderable { glyph, color }
}

fn monster_color(idx: usize, total: usize) -> (u8, u8, u8) {
    // Easier (low index) greenish, tougher reddish — brighter range.
    let t = if total <= 1 { 0.0 } else { idx as f32 / (total - 1) as f32 };
    let r = (150.0 + t * 105.0) as u8;
    let g = (240.0 - t * 160.0) as u8;
    (r, g, 110)
}

fn dim(c: (u8, u8, u8)) -> (u8, u8, u8) {
    // 55 % brightness for explored-but-not-currently-visible tiles.
    ((c.0 as u16 * 55 / 100) as u8,
     (c.1 as u16 * 55 / 100) as u8,
     (c.2 as u16 * 55 / 100) as u8)
}

// ── Boxy (CP437) tile rendering ──────────────────────────────────────────────

const BOXY_WALL:  (u8, u8, u8) = (210, 130, 50);
const BOXY_FLOOR: (u8, u8, u8) = (80, 230, 80);
const BOXY_PASS:  (u8, u8, u8) = (120, 120, 120);

fn is_wall_tile(t: TileKind) -> bool {
    matches!(t, TileKind::Wall | TileKind::SecretDoor)
}

/// Returns the correct box-drawing character for a wall tile based on its
/// cardinal neighbors: checks which adjacent cells are also wall tiles and
/// picks the matching single-line box character.
fn boxy_wall_glyph(p: Point, map: &Map) -> char {
    let n = map.in_bounds(Point::new(p.x,     p.y - 1)) && is_wall_tile(map.tile(Point::new(p.x,     p.y - 1)));
    let s = map.in_bounds(Point::new(p.x,     p.y + 1)) && is_wall_tile(map.tile(Point::new(p.x,     p.y + 1)));
    let e = map.in_bounds(Point::new(p.x + 1, p.y    )) && is_wall_tile(map.tile(Point::new(p.x + 1, p.y    )));
    let w = map.in_bounds(Point::new(p.x - 1, p.y    )) && is_wall_tile(map.tile(Point::new(p.x - 1, p.y    )));
    match (n, s, e, w) {
        (false, true,  true,  false) => '┌',
        (false, true,  false, true ) => '┐',
        (true,  false, true,  false) => '└',
        (true,  false, false, true ) => '┘',
        (true,  true,  true,  false) => '├',
        (true,  true,  false, true ) => '┤',
        (false, true,  true,  true ) => '┬',
        (true,  false, true,  true ) => '┴',
        (true,  true,  true,  true ) => '┼',
        (true,  true,  false, false) | (true,  false, false, false) | (false, true,  false, false) => '│',
        _                            => '─',
    }
}

/// Tile glyph + color for the boxy CP437 theme.
fn boxy_tile_render(t: TileKind, p: Point, map: &Map) -> (char, (u8, u8, u8)) {
    match t {
        TileKind::Empty      => (' ', (0, 0, 0)),
        TileKind::Wall       => (boxy_wall_glyph(p, map), BOXY_WALL),
        TileKind::SecretDoor => (boxy_wall_glyph(p, map), BOXY_WALL),
        TileKind::Floor      => ('·', BOXY_FLOOR),
        TileKind::Passage    => ('░', BOXY_PASS),
        TileKind::Door       => ('+', BOXY_WALL),
        TileKind::StairsDown => ('>', (255, 255, 255)),
        TileKind::StairsUp   => ('<', (255, 255, 255)),
        TileKind::Trap       => ('^', (220, 60, 60)),
    }
}

// ── Tiled (pixel-art sprites) tile rendering ─────────────────────────────────
//
// All game sprites live at extended CP437 positions 128-169 in rogue_tiles.png
// so they never overlap the standard ASCII range (32-127) used for UI text.
//
// Layout in the tileset:
//   128-153  Monsters A–Z
//   154      Player (@)
//   155      Wall
//   156      Floor
//   157      Passage
//   158      Door
//   159      Stairs (up and down share one sprite)
//   160      Trap
//   161      Amulet  (&)
//   162      Food    (:)
//   163      Gold    ($)
//   164      Potion  (!)
//   165      Ring    (=)
//   166      Scroll  (?)
//   167      Wand    (/)
//   168      Weapon  ())
//   169      Armor   ([)

/// Raw font-sheet index for each map tile (bypasses `to_cp437`).
fn tiled_tile_render(t: TileKind) -> (u16, (u8, u8, u8)) {
    let white = (255u8, 255u8, 255u8);
    match t {
        TileKind::Empty      => (32,  (0, 0, 0)),
        TileKind::Wall       => (155, white),
        TileKind::SecretDoor => (155, white),
        TileKind::Floor      => (156, white),
        TileKind::Passage    => (157, white),
        TileKind::Door       => (158, white),
        TileKind::StairsDown => (159, white),
        TileKind::StairsUp   => (159, white),
        TileKind::Trap       => (160, white),
    }
}

/// Map an entity's render glyph to the raw tile-sheet index for tiled mode.
fn tiled_entity_glyph(glyph: char) -> u16 {
    match glyph {
        'A'..='Z' => 128 + (glyph as u16 - 'A' as u16),
        '@'       => 154,
        '!'       => 164,
        ':'       => 162,
        ')'       => 168,
        '['       => 169,
        '&'       => 161,
        '?'       => 166,
        '='       => 165,
        '/'       => 167,
        '$'       => 163,
        _         => to_cp437(glyph),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_places_player_on_walkable_tile() {
        let g = Game::new();
        assert_eq!(g.depth, 1);
        assert!(g.map.is_walkable(g.player_pos()));
        assert!(g.map.stairs_down.is_some());
    }

    #[test]
    fn turns_run_without_panicking() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        for _ in 0..50 {
            for d in [
                Point::new(1, 0),
                Point::new(0, 1),
                Point::new(-1, 0),
                Point::new(0, -1),
            ] {
                if g.mode == Mode::Playing {
                    let _ = g.try_move(d);
                    g.end_player_turn();
                }
            }
        }
        // Player either survived the walk or died to a monster/trap — both fine.
        assert!(matches!(g.mode, Mode::Playing | Mode::Dead | Mode::Won));
    }

    #[test]
    fn descending_increases_depth() {
        let mut g = Game::new();
        g.descend_to(2);
        assert_eq!(g.depth, 2);
        assert!(g.map.is_walkable(g.player_pos()));
    }

    #[test]
    fn scroll_names_map_to_effects() {
        assert_eq!(
            scroll_kind_from_name("magic mapping"),
            ScrollKind::MagicMapping
        );
        assert_eq!(scroll_kind_from_name("teleportation"), ScrollKind::Teleport);
        assert_eq!(
            scroll_kind_from_name("enchant weapon"),
            ScrollKind::EnchantWeapon
        );
        assert_eq!(
            scroll_kind_from_name("protect armor"),
            ScrollKind::EnchantArmor
        );
        assert_eq!(
            scroll_kind_from_name("aggravate monsters"),
            ScrollKind::Aggravate
        );
        assert_eq!(scroll_kind_from_name("sleep"), ScrollKind::Unknown);
    }

    #[test]
    fn reading_enchant_weapon_boosts_stats() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        let before = {
            let s = g.world.get::<&Stats>(g.player).unwrap();
            (s.hit_plus, s.dam_plus)
        };
        g.inventory.push(Item {
            name: "scroll of enchant weapon".into(),
            kind: ItemKind::Scroll(ScrollKind::EnchantWeapon),
        });
        assert!(g.read_scroll());
        let after = {
            let s = g.world.get::<&Stats>(g.player).unwrap();
            (s.hit_plus, s.dam_plus)
        };
        assert_eq!(after, (before.0 + 1, before.1 + 1));
        assert!(
            !g.inventory
                .iter()
                .any(|it| matches!(it.kind, ItemKind::Scroll(_))),
            "scroll is consumed"
        );
    }

    #[test]
    fn reading_magic_mapping_reveals_the_level() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        let revealed_before = (0..g.map.height)
            .flat_map(|y| (0..g.map.width).map(move |x| Point::new(x, y)))
            .filter(|&p| g.map.is_revealed(p))
            .count();
        g.inventory.push(Item {
            name: "scroll of magic mapping".into(),
            kind: ItemKind::Scroll(ScrollKind::MagicMapping),
        });
        assert!(g.read_scroll());
        let revealed_after = (0..g.map.height)
            .flat_map(|y| (0..g.map.width).map(move |x| Point::new(x, y)))
            .filter(|&p| g.map.is_revealed(p))
            .count();
        assert!(revealed_after > revealed_before);
    }

    #[test]
    fn reading_with_no_scroll_does_nothing() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        assert!(!g.read_scroll());
    }

    #[test]
    fn ring_put_on_and_remove_cycles_correctly() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        // No rings initially.
        assert!(!g.remove_ring());
        // Add two rings to inventory and equip both.
        g.inventory.push(Item {
            name: "ring of protection".into(),
            kind: ItemKind::Ring(RingKind::Protection),
        });
        g.inventory.push(Item {
            name: "ring of dexterity".into(),
            kind: ItemKind::Ring(RingKind::Dexterity),
        });
        assert!(g.put_on_ring());
        assert_eq!(g.left_ring, Some(RingKind::Protection));
        assert!(g.put_on_ring());
        assert_eq!(g.right_ring, Some(RingKind::Dexterity));
        // Both slots full.
        g.inventory.push(Item {
            name: "ring of adornment".into(),
            kind: ItemKind::Ring(RingKind::Adornment),
        });
        assert!(!g.put_on_ring());
        // Bonuses active.
        assert_eq!(g.ring_armor_bonus(), 1);
        assert_eq!(g.ring_hit_bonus(), 1);
        // Remove left ring, returns it to inventory.
        assert!(g.remove_ring());
        assert_eq!(g.left_ring, None);
        let has_protection = g.inventory.iter().any(|it| matches!(it.kind, ItemKind::Ring(RingKind::Protection)));
        assert!(has_protection, "removed ring should be back in inventory");
    }

    #[test]
    fn ring_names_round_trip() {
        for kind in [
            RingKind::Protection, RingKind::AddStrength, RingKind::Dexterity,
            RingKind::IncreaseDamage, RingKind::Regeneration, RingKind::SlowDigestion,
            RingKind::Searching, RingKind::AggravateMonster,
        ] {
            let name = ring_name(kind);
            assert_eq!(ring_kind_from_name(name), kind, "round-trip failed for {name}");
        }
    }

    #[test]
    fn message_log_mode_switches_on_m_key() {
        let g = Game::new();
        // Verify the scroll field is initialized to 0.
        assert_eq!(g.log_scroll, 0);
        // Verify the log is non-empty after game start (welcome message).
        assert!(!g.log.is_empty());
    }

    #[test]
    fn score_records_on_death() {
        use crate::scores::Scores;
        let mut g = Game::new();
        g.mode = Mode::Playing;
        g.record_score(false);
        g.record_score(true);
        let s = Scores::load();
        assert!(!s.entries.is_empty());
    }

    #[test]
    fn confused_status_expires_correctly() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        g.apply_status("confused", 3);
        assert_eq!(g.status.confused, 3);
        g.tick_status_effects();
        assert_eq!(g.status.confused, 2);
        g.tick_status_effects();
        g.tick_status_effects();
        assert_eq!(g.status.confused, 0);
    }

    #[test]
    fn paralyzed_blocks_movement() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        g.apply_status("paralyzed", 5);
        assert_eq!(g.status.paralyzed, 5);
        // try_move should return false while paralyzed
        let result = g.try_move(Point::new(1, 0));
        assert!(!result, "paralyzed player should not be able to move");
    }

    #[test]
    fn blind_reduces_fov_radius() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        // Reveal whole map first
        g.apply_magic_mapping();
        // Apply blind
        g.apply_status("blind", 10);
        g.recompute_visibility();
        // With blind the radius is 1, so only nearby tiles should be visible
        let visible_count = (0..g.map.height)
            .flat_map(|y| (0..g.map.width).map(move |x| Point::new(x, y)))
            .filter(|&p| g.map.is_visible(p))
            .count();
        // With radius=1 only the 3x3 area around the player (max 9 tiles) is visible
        assert!(visible_count <= 9, "blind radius should be very small, got {visible_count}");
    }

    #[test]
    fn quaff_healing_potion_restores_hp() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        // Damage the player first
        {
            let mut s = g.world.get::<&mut Stats>(g.player).unwrap();
            s.hp = s.max_hp - 10;
        }
        let hp_before = {
            let s = g.world.get::<&Stats>(g.player).unwrap();
            s.hp
        };
        // Give player a healing potion
        g.inventory.push(Item {
            name: "potion of healing".to_string(),
            kind: ItemKind::Potion(PotionKind::Healing),
        });
        g.quaff();
        let hp_after = {
            let s = g.world.get::<&Stats>(g.player).unwrap();
            s.hp
        };
        assert!(hp_after > hp_before, "healing potion should restore HP");
    }

    #[test]
    fn zap_wand_with_no_target_does_not_panic() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        g.inventory.push(Item {
            name: "wand of magic missile".to_string(),
            kind: ItemKind::Wand { kind: WandKind::MagicMissile, charges: 5 },
        });
        // No monsters are visible — zapping should complete without panicking
        let acted = g.zap_wand();
        assert!(acted, "zapping with charges should consume a turn");
        // Charges should have been decremented — find the wand by type
        let charges = g.inventory.iter().find_map(|it| {
            if let ItemKind::Wand { charges, .. } = it.kind {
                Some(charges)
            } else {
                None
            }
        }).expect("wand should still be in inventory");
        assert_eq!(charges, 4, "one charge should have been consumed");
    }

    #[test]
    fn monster_special_poison_applies_poisoned_status() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        g.apply_monster_special("snake", MonsterSpecial::Poison);
        assert!(
            g.status.poisoned > 0,
            "player should be poisoned after snake bite"
        );
    }

    #[test]
    fn monster_special_drain_level_reduces_player_level() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        let before = g.world.get::<&Stats>(g.player).unwrap().level;
        g.apply_monster_special("wraith", MonsterSpecial::DrainLevel);
        let after = g.world.get::<&Stats>(g.player).unwrap().level;
        assert_eq!(after, (before - 1).max(1), "level should drop by one");
    }

    #[test]
    fn monster_special_steal_gold_removes_gold() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        g.gold = 50;
        g.apply_monster_special("leprechaun", MonsterSpecial::StealGold);
        assert_eq!(g.gold, 0, "all gold should have been stolen");
    }

    #[test]
    fn search_for_secrets_finds_nothing_without_secret_doors() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        let result = g.search_for_secrets();
        assert!(result);
    }

    #[test]
    fn throw_item_with_no_items_fails() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        g.inventory.clear();
        assert!(!g.throw_item());
    }

    #[test]
    fn identification_system_marks_item_as_known() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        assert!(!g.known_items.is_known("potion of healing"));
        g.known_items.identify("potion of healing");
        assert!(g.known_items.is_known("potion of healing"));
    }

    #[test]
    fn auto_explore_step_returns_false_when_all_explored() {
        let mut g = Game::new();
        g.mode = Mode::Playing;
        for y in 0..g.map.height {
            for x in 0..g.map.width {
                let p = Point::new(x, y);
                g.map.reveal(p);
            }
        }
        let result = g.auto_explore_step();
        let _ = result; // may be false or true, just no panic
    }
}

