//! Dice expressions, matching Rogue's `"NxM"` damage-string format.
//!
//! Rogue 5.4.2 expresses damage as strings like `"1x8"` (roll 1d8) or
//! `"2x4/1x6"` (two separate attacks: 2d4 and 1d6). We parse that exact format
//! so item/monster data can be lifted verbatim from the legacy tables.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// A single `NxM` dice term: roll `count` dice each with `sides` faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dice {
    pub count: i32,
    pub sides: i32,
}

impl Dice {
    pub const fn new(count: i32, sides: i32) -> Self {
        Self { count, sides }
    }

    /// Roll using a closure that yields a uniform value in `1..=sides`.
    pub fn roll_with(&self, mut rng: impl FnMut(i32) -> i32) -> i32 {
        let mut total = 0;
        for _ in 0..self.count {
            total += rng(self.sides);
        }
        total
    }

    /// Average (expected) value, useful for balancing and tests.
    pub fn average(&self) -> f32 {
        self.count as f32 * (self.sides as f32 + 1.0) / 2.0
    }

    pub fn max(&self) -> i32 {
        self.count * self.sides
    }
}

impl FromStr for Dice {
    type Err = DiceParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (c, d) = s
            .split_once(['x', 'd', 'X', 'D'])
            .ok_or_else(|| DiceParseError(s.to_string()))?;
        let count = c.trim().parse().map_err(|_| DiceParseError(s.to_string()))?;
        let sides = d.trim().parse().map_err(|_| DiceParseError(s.to_string()))?;
        Ok(Dice::new(count, sides))
    }
}

/// One or more dice terms separated by `/`, e.g. `"2x4/1x6"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DamageRoll(pub Vec<Dice>);

impl DamageRoll {
    pub fn terms(&self) -> &[Dice] {
        &self.0
    }

    pub fn average(&self) -> f32 {
        self.0.iter().map(Dice::average).sum()
    }
}

impl FromStr for DamageRoll {
    type Err = DiceParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let dice = s
            .split('/')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(Dice::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        if dice.is_empty() {
            return Err(DiceParseError(s.to_string()));
        }
        Ok(DamageRoll(dice))
    }
}

impl TryFrom<String> for DamageRoll {
    type Error = DiceParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<DamageRoll> for String {
    fn from(d: DamageRoll) -> String {
        d.0.iter()
            .map(|t| format!("{}x{}", t.count, t.sides))
            .collect::<Vec<_>>()
            .join("/")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiceParseError(pub String);

impl std::fmt::Display for DiceParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid dice expression: {:?}", self.0)
    }
}

impl std::error::Error for DiceParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single() {
        let d: Dice = "1x8".parse().unwrap();
        assert_eq!(d, Dice::new(1, 8));
        assert_eq!(d.max(), 8);
        assert_eq!(d.average(), 4.5);
    }

    #[test]
    fn parse_multi() {
        let r: DamageRoll = "2x4/1x6".parse().unwrap();
        assert_eq!(r.terms(), &[Dice::new(2, 4), Dice::new(1, 6)]);
        assert_eq!(r.average(), 5.0 + 3.5);
    }

    #[test]
    fn roundtrip_string() {
        let r: DamageRoll = "3x10/1x8".parse().unwrap();
        let s: String = r.clone().into();
        assert_eq!(s, "3x10/1x8");
    }

    #[test]
    fn rejects_garbage() {
        assert!("hello".parse::<DamageRoll>().is_err());
        assert!("".parse::<DamageRoll>().is_err());
    }

    #[test]
    fn roll_is_bounded() {
        let d = Dice::new(2, 6);
        // Constant rng returning the max face.
        assert_eq!(d.roll_with(|s| s), 12);
        assert_eq!(d.roll_with(|_| 1), 2);
    }
}
