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
    ConfirmQuit,
    Dead,
    Won,
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
            monster_order,
            log: Vec::new(),
            mode: Mode::Title,
            autopilot: false,
            auto_cooldown: 0,
            quit_return: Mode::Title,
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
                    self.world.spawn((Position(s.pos), GoldPile(v)));
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
        self.world.spawn((
            Position(pos),
            Renderable {
                glyph: def.symbol,
                color: monster_color(idx, self.data.monsters.len()),
            },
            Monster {
                awake: false,
                mean,
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
                let kind = if p.contains("healing") {
                    let amount = if p.contains("extra") {
                        self.rng.roll(3, 8) + 6
                    } else {
                        self.rng.roll(2, 8) + 2
                    };
                    ItemKind::Heal(amount)
                } else {
                    ItemKind::Trinket
                };
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
                Item {
                    name: format!("scroll of {s}"),
                    kind: ItemKind::Trinket,
                }
            }
            ItemCategory::Ring => {
                let s = pick_named(&cats.rings, &mut self.rng);
                Item {
                    name: format!("ring of {s}"),
                    kind: ItemKind::Trinket,
                }
            }
            ItemCategory::Stick => {
                let s = pick_named(&cats.sticks, &mut self.rng);
                Item {
                    name: format!("wand of {s}"),
                    kind: ItemKind::Trinket,
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
            VirtualKeyCode::E => acted = self.eat(),
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
            let dmg = self.rng.roll(1, 6);
            self.damage_player(dmg);
            self.log(format!("A trap springs! You take {dmg} damage."));
            self.map.set_tile(p, TileKind::Floor);
        }
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

    fn quaff(&mut self) -> bool {
        if let Some(i) = self
            .inventory
            .iter()
            .position(|it| matches!(it.kind, ItemKind::Heal(_)))
        {
            let item = self.inventory.remove(i);
            if let ItemKind::Heal(amount) = item.kind {
                let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
                s.hp = (s.hp + amount).min(s.max_hp);
                drop(s);
                self.log(format!("You quaff {} and feel better.", item.name));
            }
            true
        } else {
            self.log("You have no healing potions.");
            false
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

    // --- Combat -------------------------------------------------------------

    fn player_attack(&mut self, target: Entity) {
        let attacker = {
            let s = self.world.get::<&Stats>(self.player).unwrap();
            Attacker {
                level: s.level,
                strength: s.strength,
                damage: s.damage.clone(),
                hit_plus: s.hit_plus,
                dam_plus: s.dam_plus,
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
        let def_arm = self.world.get::<&Stats>(self.player).unwrap().armor;
        match roll_attack(&attacker, def_arm, &mut self.rng) {
            Some(dmg) => {
                self.damage_player(dmg);
                self.log(format!("The {name} hits you for {dmg}."));
            }
            None => self.log(format!("The {name} misses you.")),
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
        self.food -= 1;
        if self.food == 20 {
            self.log("You are starting to feel hungry.");
        }
        if self.food <= 0 {
            // Starving: occasional damage.
            if self.rng.percent(20) {
                self.damage_player(1);
                self.log("You faint from lack of food!");
            }
        }
        // Natural regeneration, faster at higher levels.
        self.regen_counter += 1;
        let level = self.world.get::<&Stats>(self.player).map(|s| s.level).unwrap_or(1);
        let period = (21 - level * 2).max(3);
        if self.regen_counter >= period {
            self.regen_counter = 0;
            let mut s = self.world.get::<&mut Stats>(self.player).unwrap();
            if s.hp < s.max_hp {
                s.hp += 1;
            }
        }
    }

    // --- Visibility ---------------------------------------------------------

    fn recompute_visibility(&mut self) {
        self.map.clear_visible();
        let p = self.player_pos();
        const RADIUS: i32 = 8;

        // Raycast field of view: for every tile within the radius, trace a line
        // from the player and reveal tiles until (and including) the first
        // opaque one. This lets the player see down corridors and across rooms.
        for dy in -RADIUS..=RADIUS {
            for dx in -RADIUS..=RADIUS {
                if dx * dx + dy * dy > RADIUS * RADIUS {
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

        // Standing in a lit (non-dark) room reveals the whole room at once,
        // walls included — the classic Rogue "the room lights up" behaviour.
        if let Some(ri) = self.map.room_at(p) {
            if !self.map.is_dark(p) {
                let r = self.map.rooms[ri];
                for y in r.y1..r.y2 {
                    for x in r.x1..r.x2 {
                        self.map.set_visible(Point::new(x, y));
                    }
                }
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
                self.render_play(ctx);
                self.render_help(ctx);
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
                self.render_banner(ctx, "You have died. Press Enter to play again.");
            }
            Mode::Won => {
                self.render_play(ctx);
                self.render_banner(ctx, "You escaped with the Amulet! Press Enter to play again.");
            }
        }
    }

    fn render_confirm_quit(&self, ctx: &mut BTerm) {
        let mid = SCREEN_HEIGHT / 2;
        ctx.print_color_centered(
            mid - 1,
            RGB::named(YELLOW),
            RGB::named(BLACK),
            "Quit the game?",
        );
        ctx.print_color_centered(
            mid + 1,
            RGB::named(WHITE),
            RGB::named(BLACK),
            "Press Y or Enter to quit   ·   N or Esc to keep playing",
        );
    }

    fn render_title(&self, ctx: &mut BTerm) {
        ctx.print_centered(8, "R O G U E");
        ctx.print_centered(10, "a modern Rust reimplementation");
        ctx.print_centered(13, "Press Enter to begin");
        ctx.print_centered(15, "Move: hjkl / yubn / arrows   Wait: .   Descend: >");
        ctx.print_centered(16, "Pick up: g   Quaff: q   Eat: e   Quit: Esc");
        ctx.print_centered(18, "Press ? in game for help   ·   A = autopilot bot");
    }

    fn render_help(&self, ctx: &mut BTerm) {
        let lines = [
            "===== HELP =====",
            "",
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
            "",
            "Other",
            "  ?              show / hide this help",
            "  A              toggle autopilot (a bot plays for you)",
            "  Esc            quit",
            "",
            "Press any key to return to the game.",
        ];
        let top = (SCREEN_HEIGHT - lines.len() as i32) / 2;
        for (i, line) in lines.iter().enumerate() {
            ctx.print_color_centered(
                top + i as i32,
                RGB::named(WHITE),
                RGB::named(BLACK),
                line,
            );
        }
    }

    fn render_banner(&self, ctx: &mut BTerm, msg: &str) {
        ctx.print_color_centered(
            SCREEN_HEIGHT / 2,
            RGB::named(YELLOW),
            RGB::named(BLACK),
            msg,
        );
    }

    fn render_play(&self, ctx: &mut BTerm) {
        // Most recent message on the top line.
        if let Some(last) = self.log.last() {
            ctx.print(0, MSG_ROW, last);
        }

        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let p = Point::new(x, y);
                if !self.map.is_revealed(p) {
                    continue;
                }
                let (glyph, mut color) = tile_render(self.map.tile(p));
                if !self.map.is_visible(p) {
                    color = dim(color); // remembered but out of sight
                }
                ctx.set(x, y + MAP_TOP, rgb(color), RGB::named(BLACK), to_cp437(glyph));
            }
        }

        // Entities, only where currently visible.
        for (_e, (pos, r)) in self.world.query::<(&Position, &Renderable)>().iter() {
            if self.map.is_visible(pos.0) {
                ctx.set(
                    pos.0.x,
                    pos.0.y + MAP_TOP,
                    rgb(r.color),
                    RGB::named(BLACK),
                    to_cp437(r.glyph),
                );
            }
        }

        self.render_status(ctx);
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
        let line = format!(
            "Level:{}  HP:{}/{}  Str:{}  Arm:{}  Gold:{}  Exp:{}  Depth:{}  {}{}",
            s.level, s.hp, s.max_hp, s.strength, s.armor, self.gold, s.xp_reward, self.depth, hunger, auto
        );
        ctx.print(0, row, line);
    }

    // --- Autopilot bot ------------------------------------------------------

    /// Where the bot wants to go on this level: the Amulet if it is here,
    /// otherwise the down staircase.
    fn auto_target(&self) -> Option<Point> {
        for (_e, (pos, item)) in self.world.query::<(&Position, &Item)>().iter() {
            if matches!(item.kind, ItemKind::Amulet) {
                return Some(pos.0);
            }
        }
        self.map.stairs_down
    }

    /// Take a single bot action: descend/pick up if on the target, else step
    /// one tile along the shortest path toward it (bumping any monster in the
    /// way). Returns true if a turn was consumed.
    pub fn auto_step(&mut self) -> bool {
        let here = self.player_pos();
        let Some(target) = self.auto_target() else {
            return false;
        };
        if here == target {
            if self.map.tile(here) == TileKind::StairsDown {
                return self.try_descend();
            }
            return self.pickup();
        }
        let Some(path) = self.map.find_path(here, target) else {
            return false;
        };
        let Some(&next) = path.first() else {
            return false;
        };
        self.try_move(Point::new(next.x - here.x, next.y - here.y))
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
            self.log("Autopilot stopped (no path to the stairs).");
        }
    }

    /// Toggle the autopilot bot on/off (bound to the `A` key in game).
    pub fn toggle_autopilot(&mut self) {
        self.autopilot = !self.autopilot;
        self.auto_cooldown = 0;
        if self.autopilot {
            self.log("Autopilot engaged — watch the bot descend. Press A to stop.");
        } else {
            self.log("Autopilot disengaged.");
        }
    }

    /// Start the game straight into a watchable autopilot run (CLI `--demo`).
    pub fn start_demo(&mut self) {
        self.mode = Mode::Playing;
        self.autopilot = true;
        self.auto_cooldown = 0;
        self.log("Demo mode: autopilot is driving. Press A to take control.");
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
                } else if ctx.key == Some(VirtualKeyCode::Escape) {
                    self.request_quit();
                }
            }
            Mode::Playing => match ctx.key {
                Some(VirtualKeyCode::Escape) => self.request_quit(),
                Some(VirtualKeyCode::A) => self.toggle_autopilot(),
                Some(VirtualKeyCode::Slash) if !self.autopilot => self.mode = Mode::Help,
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
            Mode::ConfirmQuit => match ctx.key {
                Some(VirtualKeyCode::Y) | Some(VirtualKeyCode::Return) => ctx.quitting = true,
                Some(VirtualKeyCode::N) | Some(VirtualKeyCode::Escape) => {
                    self.mode = self.quit_return;
                }
                _ => {}
            },
            Mode::Dead | Mode::Won => {
                if ctx.key == Some(VirtualKeyCode::Return) {
                    *self = Game::new();
                    self.mode = Mode::Playing;
                } else if ctx.key == Some(VirtualKeyCode::Escape) {
                    self.request_quit();
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

fn tile_render(t: TileKind) -> (char, (u8, u8, u8)) {
    match t {
        TileKind::Empty => (' ', (0, 0, 0)),
        TileKind::Floor => ('.', (120, 120, 120)),
        TileKind::Wall => ('#', (140, 110, 80)),
        TileKind::Passage => ('#', (90, 90, 90)),
        TileKind::Door => ('+', (160, 120, 60)),
        TileKind::StairsDown => ('>', (255, 255, 255)),
        TileKind::StairsUp => ('<', (255, 255, 255)),
        TileKind::Trap => ('^', (255, 80, 80)),
    }
}

fn item_render(item: &Item) -> Renderable {
    let (glyph, color) = match item.kind {
        ItemKind::Heal(_) => ('!', (255, 0, 255)),
        ItemKind::Food => (':', (200, 160, 80)),
        ItemKind::Weapon { .. } => (')', (180, 180, 220)),
        ItemKind::Armor(_) => ('[', (150, 150, 200)),
        ItemKind::Amulet => ('&', (255, 255, 0)),
        ItemKind::Trinket => ('?', (120, 200, 120)),
    };
    Renderable { glyph, color }
}

fn monster_color(idx: usize, total: usize) -> (u8, u8, u8) {
    // Easier (low index) greenish, tougher reddish.
    let t = if total <= 1 {
        0.0
    } else {
        idx as f32 / (total - 1) as f32
    };
    let r = (80.0 + t * 175.0) as u8;
    let g = (200.0 - t * 150.0) as u8;
    (r, g, 80)
}

fn dim(c: (u8, u8, u8)) -> (u8, u8, u8) {
    (c.0 / 3, c.1 / 3, c.2 / 3)
}

fn rgb(c: (u8, u8, u8)) -> RGB {
    RGB::from_u8(c.0, c.1, c.2)
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
}

