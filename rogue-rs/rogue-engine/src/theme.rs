//! Visual color themes for the Rogue renderer.
//!
//! This module is a platform-neutral copy of the one in `rogue-cli` — the only
//! difference is that all color values are plain `(u8, u8, u8)` tuples rather
//! than bracket-lib `RGB` values so that the engine can compile for any target.

/// Whether to use standard ASCII glyphs or CP437 box-drawing glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphStyle {
    /// Standard Rogue glyphs: `@`, `#`, `.`, `+`, `>`, etc.
    Ascii,
    /// CP437 box-drawing for walls, `·` floors, `░` passages, `☺` player.
    Boxy,
    /// Pixel-art tile sprites (requires a custom tileset font).
    Tiled,
}

/// A rendering color theme.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    tint: Option<(u8, u8, u8)>,
    pub glyph_style: GlyphStyle,
}

impl Theme {
    pub fn classic() -> Self { Self { tint: None, glyph_style: GlyphStyle::Ascii } }
    pub fn amber()   -> Self { Self { tint: Some((255, 180, 60)), glyph_style: GlyphStyle::Ascii } }
    pub fn green()   -> Self { Self { tint: Some((80, 255, 80)),  glyph_style: GlyphStyle::Ascii } }
    pub fn boxy()    -> Self { Self { tint: None, glyph_style: GlyphStyle::Boxy } }
    pub fn tiled()   -> Self { Self { tint: None, glyph_style: GlyphStyle::Tiled } }

    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "amber" => Self::amber(),
            "green" => Self::green(),
            "boxy"  => Self::boxy(),
            "tiled" => Self::tiled(),
            _       => Self::classic(),
        }
    }

    /// Convert a raw `(r, g, b)` color applying the tint if set.
    pub fn apply(&self, c: (u8, u8, u8)) -> (u8, u8, u8) {
        match self.tint {
            None => c,
            Some(tint) => {
                let l = (0.299 * c.0 as f32 + 0.587 * c.1 as f32 + 0.114 * c.2 as f32) / 255.0;
                (
                    (l * tint.0 as f32) as u8,
                    (l * tint.1 as f32) as u8,
                    (l * tint.2 as f32) as u8,
                )
            }
        }
    }

    pub fn use_boxy(&self) -> bool  { self.glyph_style == GlyphStyle::Boxy }
    pub fn use_tiled(&self) -> bool { self.glyph_style == GlyphStyle::Tiled }

    pub fn bg(&self)     -> (u8, u8, u8) { (0, 0, 0) }
    pub fn fg(&self)     -> (u8, u8, u8) { self.apply((255, 255, 255)) }
    pub fn header(&self) -> (u8, u8, u8) { self.apply((255, 220, 0)) }
    pub fn dim_ui(&self) -> (u8, u8, u8) { self.apply((140, 140, 140)) }
    pub fn accent(&self) -> (u8, u8, u8) { self.apply((0, 230, 230)) }
}
