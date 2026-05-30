//! Combat resolution, ported faithfully from Rogue v5.4.2 `fight.c`.
//!
//! The to-hit roll is `swing(at_lvl, op_arm, wplus)` and damage is rolled per
//! attack term, with strength-based bonuses from the `str_plus` / `add_dam`
//! tables. Keeping this engine-agnostic makes it unit-testable without a
//! renderer.

use crate::dice::DamageRoll;
use crate::rng::RogueRng;

/// Adjustment to hit probability due to strength (index by strength value).
const STR_PLUS: [i32; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 3,
];

/// Adjustment to damage due to strength (index by strength value).
const ADD_DAM: [i32; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3, 3, 4, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 6,
];

fn table_lookup(table: &[i32; 32], strength: i32) -> i32 {
    let idx = strength.clamp(0, 31) as usize;
    table[idx]
}

/// Strength-based to-hit bonus.
pub fn str_plus(strength: i32) -> i32 {
    table_lookup(&STR_PLUS, strength)
}

/// Strength-based damage bonus.
pub fn add_dam(strength: i32) -> i32 {
    table_lookup(&ADD_DAM, strength)
}

/// The attacker side of a melee exchange.
#[derive(Debug, Clone)]
pub struct Attacker {
    /// Experience / monster level (`s_lvl`).
    pub level: i32,
    /// Strength (`s_str`).
    pub strength: i32,
    /// One or more attacks, e.g. `"1x8"` or `"1x8/1x8/3x10"`.
    pub damage: DamageRoll,
    /// Flat to-hit bonus from a wielded weapon (`hplus`).
    pub hit_plus: i32,
    /// Flat damage bonus from a wielded weapon (`dplus`).
    pub dam_plus: i32,
}

/// One swing's to-hit check: `swing(at_lvl, op_arm, wplus)` from `fight.c`.
///
/// `rnd(20)` yields `0..20`; the swing connects when `res + wplus >= need`,
/// where `need = (20 - at_lvl) - op_arm`. A higher defender armor value is
/// *easier* to hit, exactly as in classic Rogue (where lower AC is better).
pub fn swing(at_lvl: i32, op_arm: i32, wplus: i32, rng: &mut impl RogueRng) -> bool {
    let res = rng.rnd(20);
    let need = (20 - at_lvl) - op_arm;
    res + wplus >= need
}

/// Resolve a full attack (possibly several swings) against a defender whose
/// armor value is `def_arm`. Returns the total damage dealt, or `None` on a
/// complete miss (no swing connected).
pub fn roll_attack(att: &Attacker, def_arm: i32, rng: &mut impl RogueRng) -> Option<i32> {
    let wplus = att.hit_plus + str_plus(att.strength);
    let mut total = 0;
    let mut hit = false;
    for term in att.damage.terms() {
        if swing(att.level, def_arm, wplus, rng) {
            let proll = rng.roll(term.count, term.sides);
            let damage = att.dam_plus + proll + add_dam(att.strength);
            total += damage.max(0);
            hit = true;
        }
    }
    if hit {
        Some(total)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn strength_tables_match_legacy() {
        // str 16 is the unenchanted baseline: no to-hit, +1 damage.
        assert_eq!(str_plus(16), 0);
        assert_eq!(add_dam(16), 1);
        // Extremes clamp safely.
        assert_eq!(str_plus(0), -7);
        assert_eq!(str_plus(31), 3);
        assert_eq!(add_dam(31), 6);
        assert_eq!(str_plus(99), 3);
    }

    #[test]
    fn very_strong_low_ac_attacker_always_hits_weak_defender() {
        let att = Attacker {
            level: 10,
            strength: 31,
            damage: "2x6".parse().unwrap(),
            hit_plus: 5,
            dam_plus: 3,
        };
        let mut rng = StdRng::seed_from_u64(7);
        // High armor defender (easy to hit) -> always connects, positive damage.
        let mut hits = 0;
        for _ in 0..100 {
            if let Some(dmg) = roll_attack(&att, 9, &mut rng) {
                assert!(dmg > 0);
                hits += 1;
            }
        }
        assert_eq!(hits, 100);
    }

    #[test]
    fn hopeless_attacker_misses_tough_defender() {
        let att = Attacker {
            level: 1,
            strength: 3,
            damage: "1x4".parse().unwrap(),
            hit_plus: 0,
            dam_plus: 0,
        };
        let mut rng = StdRng::seed_from_u64(3);
        // Very low (good) AC defender: need = 19 - (-10) = 29, impossible with rnd(20).
        for _ in 0..100 {
            assert!(roll_attack(&att, -10, &mut rng).is_none());
        }
    }
}
