# Themes & Color System

## Overview

All colors in `rogue-engine` are `(u8, u8, u8)` RGB tuples — never bracket-lib `RGB`.
The `rogue-cli` has a parallel `theme.rs` that returns bracket-lib `RGB`; keep color values in sync.

---

## Theme variants

| Theme | Glyph style | Tint color |
|-------|-------------|-----------|
| `classic` | ASCII | none (white) |
| `amber` | ASCII | (255, 180, 60) |
| `green` | ASCII | (80, 255, 80) |
| `boxy` | CP437 box-drawing | none |
| `tiled` | Pixel-art sprites | none |

Switch in-game by setting `game.theme = Theme::from_name("amber")`.

---

## Tint math

Tint applies luminance-preserving recoloring:

```rust
let l = (0.299*r + 0.587*g + 0.114*b) / 255.0;   // perceived luminance
result = (l * tint.r, l * tint.g, l * tint.b)
```

---

## Theme helper colors

| Method | Returns |
|--------|---------|
| `theme.bg()` | `(0, 0, 0)` — always black |
| `theme.fg()` | tinted white — used for most text |
| `theme.header()` | tinted (255, 220, 0) — panel titles, banner |
| `theme.dim_ui()` | tinted (190, 190, 190) — footer hints, muted text |
| `theme.accent()` | tinted (0, 230, 230) — newest log line, scroll indicators |

---

## Map tile colors

Defined in `tile_render()` and `boxy_tile_render()` in `game.rs` (both files):

| Tile | Glyph | Color |
|------|-------|-------|
| Floor | `.` | (175, 175, 175) |
| Wall | `#` | (200, 165, 110) |
| Passage | `#` | (135, 135, 135) |
| Door | `+` | (210, 165, 85) |
| StairsDown/Up | `>`/`<` | (255, 255, 255) |
| Trap | `^` | (255, 110, 110) |

**Boxy theme** uses CP437 box-drawing characters:

| Constant | Value |
|----------|-------|
| `BOXY_WALL` | (210, 130, 50) |
| `BOXY_FLOOR` | (80, 230, 80) |
| `BOXY_PASS` | (120, 120, 120) |

Boxy walls use `boxy_wall_glyph()` to pick the correct box-drawing character based on cardinal neighbors (┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼ │ ─).

---

## Visibility dimming

Tiles that have been explored but are not currently visible are dimmed to 55%:

```rust
fn dim(c: (u8, u8, u8)) -> (u8, u8, u8) {
    ((c.0 as u16 * 55 / 100) as u8, ...)
}
```

---

## Entity colors

| Entity type | Color |
|-------------|-------|
| Player `@` | (255, 255, 0) — yellow |
| Monsters A–Z | `monster_color(idx, total)` — green (easy) → red (hard) |

`monster_color()`: `r = 150 + t*105`, `g = 240 - t*160`, `b = 110` where `t` = 0.0 (easiest) to 1.0 (hardest).

---

## Item colors

| Item kind | Glyph | Color |
|-----------|-------|-------|
| Potion / Heal | `!` | (255, 80, 255) |
| Food | `:` | (220, 180, 100) |
| Weapon | `)` | (200, 200, 240) |
| Armor | `[` | (180, 180, 230) |
| Amulet | `&` | (255, 255, 0) |
| Scroll | `?` | (240, 240, 200) |
| Ring | `=` | (220, 200, 80) |
| Wand | `/` | (180, 230, 255) |
| Trinket | `?` | (150, 220, 150) |

---

## Tiled theme sprite layout

The tiled theme uses `rogue_tiles.png` (256×256, 16×16 per glyph). Sprite indices:

| Index | Sprite |
|-------|--------|
| 0–127 | Standard CP437 / boxy font glyphs |
| 128–153 | Monsters A–Z |
| 154 | Player `@` |
| 155 | Wall |
| 156 | Floor |
| 157 | Passage |
| 158 | Door |
| 159 | Stairs (up and down share one sprite) |
| 160 | Trap |
| 161 | Amulet `&` |
| 162 | Food `:` |
| 163 | Gold `$` |
| 164 | Potion `!` |
| 165 | Ring `=` |
| 166 | Scroll `?` |
| 167 | Wand `/` |
| 168 | Weapon `)` |
| 169 | Armor `[` |

`tiled_entity_glyph(ch)` maps entity render glyphs to sprite indices. `tiled_tile_render(tile)` maps `TileKind` to indices. These are `u16` values passed to `ctx.set()` / `fb.set_raw()`.
