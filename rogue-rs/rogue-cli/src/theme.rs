//! Visual color themes for the Rogue terminal renderer.
//!
//! The **classic** theme uses the original multi-color palette.  The
//! **amber** and **green** themes mimic vintage CRT phosphor monitors by
//! converting every color to a single tinted hue while preserving relative
//! luminance, so gameplay cues (bright vs. dim tiles, remembered vs. visible)
//! remain legible.  The **boxy** theme uses CP437 box-drawing characters for
//! room walls, middle-dot floors, and light-shade passage tiles — matching
//! the PC/IBM-style screenshot from the legacy Retro Rogue Collection.

use bracket_lib::prelude::RGB;

/// Whether to use standard ASCII glyphs or CP437 box-drawing glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphStyle {
    /// Standard Rogue glyphs: `@`, `#`, `.`, `+`, `>`, etc.
    Ascii,
    /// CP437 box-drawing for walls, `·` floors, `░` passages, `☺` player.
    Boxy,
}

/// A rendering color theme.
///
/// Construct one of the named themes with [`Theme::classic`], [`Theme::amber`],
/// [`Theme::green`], [`Theme::boxy`], or look up by name with [`Theme::from_name`].
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// `None` → classic multi-colour mode.
    /// `Some(tint)` → monochrome: every color is mapped to `luminance * tint`.
    tint: Option<(u8, u8, u8)>,
    /// Controls tile and player glyph selection.
    pub glyph_style: GlyphStyle,
}

impl Theme {
    /// Original multi-color palette (default).
    pub fn classic() -> Self {
        Self { tint: None, glyph_style: GlyphStyle::Ascii }
    }

    /// Amber phosphor CRT — everything tinted warm amber.
    pub fn amber() -> Self {
        Self { tint: Some((255, 180, 60)), glyph_style: GlyphStyle::Ascii }
    }

    /// Green phosphor CRT — everything tinted green.
    pub fn green() -> Self {
        Self { tint: Some((80, 255, 80)), glyph_style: GlyphStyle::Ascii }
    }

    /// PC/IBM style: CP437 box-drawing walls, green dot floors, gray passages.
    pub fn boxy() -> Self {
        Self { tint: None, glyph_style: GlyphStyle::Boxy }
    }

    /// Look up a theme by name (case-insensitive); unknown names fall back to classic.
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "amber"  => Self::amber(),
            "green"  => Self::green(),
            "boxy"   => Self::boxy(),
            _        => Self::classic(),
        }
    }

    /// Convert a raw `(r, g, b)` color to an `RGB` value, applying the tint if set.
    ///
    /// In monochrome mode the perceived luminance of the input is preserved so
    /// that bright / dim distinctions remain visible.
    pub fn apply(&self, c: (u8, u8, u8)) -> RGB {
        match self.tint {
            None => RGB::from_u8(c.0, c.1, c.2),
            Some(tint) => {
                let l = (0.299 * c.0 as f32 + 0.587 * c.1 as f32 + 0.114 * c.2 as f32) / 255.0;
                RGB::from_u8(
                    (l * tint.0 as f32) as u8,
                    (l * tint.1 as f32) as u8,
                    (l * tint.2 as f32) as u8,
                )
            }
        }
    }

    /// Returns `true` when CP437 box-drawing glyphs should be used.
    pub fn use_boxy(&self) -> bool {
        self.glyph_style == GlyphStyle::Boxy
    }

    /// Background / clear color (always black).
    pub fn bg(&self) -> RGB { RGB::from_u8(0, 0, 0) }

    /// Normal foreground text.
    pub fn fg(&self) -> RGB { self.apply((255, 255, 255)) }

    /// Header / accent highlight (yellow in classic mode).
    pub fn header(&self) -> RGB { self.apply((255, 220, 0)) }

    /// Dimmed / secondary text (gray in classic mode).
    pub fn dim_ui(&self) -> RGB { self.apply((140, 140, 140)) }

    /// Cyan accent (used for special info lines in classic mode).
    pub fn accent(&self) -> RGB { self.apply((0, 230, 230)) }
}

