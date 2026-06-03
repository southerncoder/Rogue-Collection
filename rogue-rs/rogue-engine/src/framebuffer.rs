//! Platform-neutral framebuffer.
//!
//! The game renders into a [`FrameBuffer`] each tick; the active frontend
//! (bracket-lib, macroquad, …) then blits it to the actual display.

use crate::game::SCREEN_WIDTH;

/// A single terminal-style cell.
#[derive(Clone)]
pub struct Cell {
    /// Unicode glyph to display (used when `cp437` is `None`).
    pub glyph: char,
    /// Optional raw CP437 code point (takes precedence over `glyph` when set).
    pub cp437: Option<u16>,
    /// Foreground color `(r, g, b)`.
    pub fg: (u8, u8, u8),
    /// Background color `(r, g, b)`.
    pub bg: (u8, u8, u8),
}

impl Cell {
    fn blank(bg: (u8, u8, u8)) -> Self {
        Self { glyph: ' ', cp437: None, fg: (0, 0, 0), bg }
    }
}

/// Full-screen cell grid produced by a single game tick.
pub struct FrameBuffer {
    pub width: i32,
    pub height: i32,
    pub cells: Vec<Cell>,
    /// Set to `true` when the game requests that the application exit.
    pub wants_quit: bool,
}

impl FrameBuffer {
    pub fn new(width: i32, height: i32) -> Self {
        let bg = (0u8, 0u8, 0u8);
        Self {
            width,
            height,
            cells: vec![Cell::blank(bg); (width * height) as usize],
            wants_quit: false,
        }
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        Some((y * self.width + x) as usize)
    }

    /// Fill all cells with the given background color.
    pub fn clear(&mut self, bg: (u8, u8, u8)) {
        for c in &mut self.cells {
            *c = Cell::blank(bg);
        }
    }

    /// Place a Unicode glyph at `(x, y)`.
    pub fn set(&mut self, x: i32, y: i32, fg: (u8, u8, u8), bg: (u8, u8, u8), glyph: char) {
        if let Some(i) = self.idx(x, y) {
            self.cells[i] = Cell { glyph, cp437: None, fg, bg };
        }
    }

    /// Place a raw CP437 code point at `(x, y)`.
    pub fn set_raw(&mut self, x: i32, y: i32, fg: (u8, u8, u8), bg: (u8, u8, u8), cp437: u16) {
        if let Some(i) = self.idx(x, y) {
            self.cells[i] = Cell { glyph: ' ', cp437: Some(cp437), fg, bg };
        }
    }

    /// Print a string starting at `(x, y)`.
    pub fn print(&mut self, x: i32, y: i32, fg: (u8, u8, u8), bg: (u8, u8, u8), text: impl AsRef<str>) {
        for (i, ch) in text.as_ref().chars().enumerate() {
            self.set(x + i as i32, y, fg, bg, ch);
        }
    }

    /// Print a string centered on row `y`.
    pub fn print_centered(&mut self, y: i32, fg: (u8, u8, u8), bg: (u8, u8, u8), text: impl AsRef<str>) {
        let s = text.as_ref();
        let x = (SCREEN_WIDTH - s.chars().count() as i32) / 2;
        self.print(x.max(0), y, fg, bg, s);
    }

    /// Draw a border box.  `w` and `h` are the outer dimensions (including border).
    pub fn draw_box(&mut self, x: i32, y: i32, w: i32, h: i32, fg: (u8, u8, u8), bg: (u8, u8, u8)) {
        // Corners
        self.set(x,         y,         fg, bg, '┌');
        self.set(x + w - 1, y,         fg, bg, '┐');
        self.set(x,         y + h - 1, fg, bg, '└');
        self.set(x + w - 1, y + h - 1, fg, bg, '┘');
        // Horizontal edges
        for cx in (x + 1)..(x + w - 1) {
            self.set(cx, y,         fg, bg, '─');
            self.set(cx, y + h - 1, fg, bg, '─');
        }
        // Vertical edges
        for cy in (y + 1)..(y + h - 1) {
            self.set(x,         cy, fg, bg, '│');
            self.set(x + w - 1, cy, fg, bg, '│');
        }
    }
}
