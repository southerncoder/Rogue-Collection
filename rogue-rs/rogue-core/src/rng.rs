//! A thin Rogue-flavoured wrapper over [`rand::Rng`].
//!
//! Classic Rogue uses `rnd(n)` returning a uniform value in `0..n`. We expose
//! the same primitive plus a couple of helpers so generation code reads like the
//! original C.

use rand::Rng;

/// Rogue-style random helpers available on any [`rand::Rng`].
pub trait RogueRng {
    /// Uniform integer in `0..n`. Returns 0 if `n <= 0` (matching the C guard).
    fn rnd(&mut self, n: i32) -> i32;

    /// Roll `count` dice each with `sides` faces (each die yields `1..=sides`).
    fn roll(&mut self, count: i32, sides: i32) -> i32 {
        let mut total = 0;
        for _ in 0..count {
            if sides > 0 {
                total += self.rnd(sides) + 1;
            }
        }
        total
    }

    /// True with probability `p` percent, i.e. `rnd(100) < p`.
    fn percent(&mut self, p: i32) -> bool {
        self.rnd(100) < p
    }
}

impl<R: Rng> RogueRng for R {
    fn rnd(&mut self, n: i32) -> i32 {
        if n <= 0 {
            0
        } else {
            self.gen_range(0..n)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn rnd_is_bounded_and_zero_safe() {
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(rng.rnd(0), 0);
        assert_eq!(rng.rnd(-5), 0);
        for _ in 0..1000 {
            let v = rng.rnd(6);
            assert!((0..6).contains(&v));
        }
    }

    #[test]
    fn roll_in_range() {
        let mut rng = StdRng::seed_from_u64(2);
        for _ in 0..1000 {
            let v = rng.roll(2, 6);
            assert!((2..=12).contains(&v));
        }
    }
}
