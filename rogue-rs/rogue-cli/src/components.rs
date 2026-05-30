//! ECS components for the hecs world.

use rogue_core::dice::DamageRoll;
use rogue_core::Point;

/// Where an entity sits on the map.
#[derive(Debug, Clone, Copy)]
pub struct Position(pub Point);

/// How an entity is drawn.
#[derive(Debug, Clone, Copy)]
pub struct Renderable {
    pub glyph: char,
    pub color: (u8, u8, u8),
}

/// Marker for the single player entity.
#[derive(Debug, Clone, Copy)]
pub struct Player;

/// A hostile creature.
#[derive(Debug, Clone)]
pub struct Monster {
    /// Has the monster noticed the player yet?
    pub awake: bool,
    /// Attacks the player on sight.
    pub mean: bool,
}

/// A human-readable name (for combat messages).
#[derive(Debug, Clone)]
pub struct Name(pub String);

/// Combat / vitality stats shared by the player and monsters.
#[derive(Debug, Clone)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    /// Experience level (player) or monster level — drives to-hit.
    pub level: i32,
    pub strength: i32,
    /// Armor value (lower is better, classic Rogue convention).
    pub armor: i32,
    pub damage: DamageRoll,
    pub hit_plus: i32,
    pub dam_plus: i32,
    /// Experience awarded when this entity dies (monsters only).
    pub xp_reward: i32,
}

/// Marks a tile-blocking entity (used for movement / attack resolution).
#[derive(Debug, Clone, Copy)]
pub struct BlocksTile;

/// A pile of gold lying on the floor.
#[derive(Debug, Clone, Copy)]
pub struct GoldPile(pub i32);

/// The kind of a pickable item and its gameplay effect.
#[derive(Debug, Clone)]
pub enum ItemKind {
    /// Restores hit points (healing / extra healing).
    Heal(i32),
    /// Resets the hunger clock.
    Food,
    /// Wieldable weapon.
    Weapon {
        damage: DamageRoll,
        hit_plus: i32,
        dam_plus: i32,
    },
    /// Wearable armor with the given armor value.
    Armor(i32),
    /// The Amulet of Yendor — the win condition.
    Amulet,
    /// Anything else: carried but inert in this build.
    Trinket,
}

/// A pickable item on the floor or in the inventory.
#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub kind: ItemKind,
}
