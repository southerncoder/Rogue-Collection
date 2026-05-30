//! Basic 2D geometry shared across the game.

use serde::{Deserialize, Serialize};

/// An integer 2D point / coordinate. `x` is the column, `y` is the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Chebyshev (king-move) distance — matches Rogue's 8-directional movement.
    pub fn chebyshev(self, other: Point) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }

    /// Manhattan distance.
    pub fn manhattan(self, other: Point) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

impl std::ops::Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

/// An axis-aligned rectangle, inclusive of `x1,y1`, exclusive of `x2,y2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    pub fn width(&self) -> i32 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> i32 {
        self.y2 - self.y1
    }

    pub fn center(&self) -> Point {
        Point::new((self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2)
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x1 <= other.x2 && self.x2 >= other.x1 && self.y1 <= other.y2 && self.y2 >= other.y1
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x1 && p.x < self.x2 && p.y >= self.y1 && p.y < self.y2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        let a = Point::new(0, 0);
        let b = Point::new(3, 4);
        assert_eq!(a.manhattan(b), 7);
        assert_eq!(a.chebyshev(b), 4);
    }

    #[test]
    fn rect_center_and_contains() {
        let r = Rect::new(2, 2, 4, 4);
        assert_eq!(r.center(), Point::new(4, 4));
        assert!(r.contains(Point::new(3, 3)));
        assert!(!r.contains(Point::new(6, 6)));
    }
}
