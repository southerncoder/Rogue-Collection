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
    /// A potion with a specific effect when quaffed.
    Potion(PotionKind),
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
    /// A readable scroll with a one-shot effect.
    Scroll(ScrollKind),
    /// A wearable ring with a passive effect.
    Ring(RingKind),
    /// A zappable wand or staff.
    Wand { kind: WandKind, charges: i32 },
    /// Anything else: carried but inert in this build.
    Trinket,
}

/// The effect produced when a potion is quaffed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PotionKind {
    Healing,
    ExtraHealing,
    Poison,
    GainStrength,
    RestoreStrength,
    SeeInvisible,
    Confusion,
    Blindness,
    Hallucination,
    HasteSelf,
    RaiseLevel,
    MonsterDetection,
    MagicDetection,
    Levitation,
    Unknown,
}

/// The effect produced when a wand or staff is zapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WandKind {
    MagicMissile,
    Slow,
    Fear,
    Confusion,
    DrainLife,
    Polymorph,
    Haste,
    TeleportAway,
    CancellationWand,
    NothingWand,
    Light,
    Fire,
    Cold,
    Lightning,
    Unknown,
}

/// The effect produced when a scroll is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollKind {
    /// Reveal the entire level layout.
    MagicMapping,
    /// Whisk the player to a random walkable tile.
    Teleport,
    /// Permanently improve the wielded weapon (+1 to hit and damage).
    EnchantWeapon,
    /// Permanently improve worn armor (one point better).
    EnchantArmor,
    /// Wake and anger every monster on the level.
    Aggravate,
    /// A scroll whose effect is not modelled yet — reads harmlessly.
    Unknown,
}

/// The passive effect produced by a worn ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingKind {
    /// +1 to effective armor class (better defense).
    Protection,
    /// +1 effective strength (improves attack).
    AddStrength,
    /// Prevents strength drain (cosmetic in this build).
    SustainStrength,
    /// Auto-searches adjacent tiles each turn (larger FOV radius).
    Searching,
    /// See invisible monsters (cosmetic in this build).
    SeeInvisible,
    /// No mechanical effect.
    Adornment,
    /// Wakes all monsters when equipped (bad ring).
    AggravateMonster,
    /// +1 to hit bonus.
    Dexterity,
    /// +1 to damage bonus.
    IncreaseDamage,
    /// Doubles natural regeneration rate.
    Regeneration,
    /// Food is consumed half as quickly.
    SlowDigestion,
    /// Random teleport each ~100 turns (bad ring).
    Teleportation,
    /// Monsters less likely to notice you (cosmetic in this build).
    Stealth,
    /// Armor cannot rust (cosmetic in this build).
    MaintainArmor,
    /// A ring whose effect is not modelled yet.
    Unknown,
}

/// A pickable item on the floor or in the inventory.
#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub kind: ItemKind,
}

/// Active status effects on a creature.
#[derive(Debug, Clone, Default)]
pub struct StatusEffects {
    /// Remaining turns of confusion (random movement).
    pub confused: i32,
    /// Remaining turns of blindness (FOV radius = 1).
    pub blind: i32,
    /// Remaining turns of poison (lose 1 STR per 3 turns, tracked via `poison_tick`).
    pub poisoned: i32,
    /// Remaining turns of paralysis (skip turns).
    pub paralyzed: i32,
    /// Counter for poison damage timing.
    pub poison_tick: i32,
    /// Remaining turns of haste (double speed — simplified: take two steps per turn).
    pub haste: i32,
}

impl StatusEffects {
    pub fn is_any_active(&self) -> bool {
        self.confused > 0
            || self.blind > 0
            || self.poisoned > 0
            || self.paralyzed > 0
            || self.haste > 0
    }
}
