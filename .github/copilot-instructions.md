# Copilot Instructions — Rogue-rs

## Attribution & Licenses

The `legacy/` directory is a verbatim preservation of the
**[Retro Rogue Collection](https://github.com/mikeyk730/Rogue-Collection)** by
**mikeyk730** (Mike Kamermans). It contains six historical Rogue versions, SDL2/Qt
frontends, and Rog-O-Matic. **Do not modify `legacy/` source files.**

Original Rogue game source:
> Copyright © 1980–1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman — BSD 3-Clause.
> See `legacy/src/RogueVersions/Rogue_5_4_2/LICENSE.TXT`.

The Rust reimplementation in `rogue-rs/` is MIT licensed and built from scratch using
Unix Rogue v5.4.2 as the canonical behavioral reference.

---

## Build, Test, Lint

All commands run from `rogue-rs/` (the Cargo workspace root). The game reads `assets/`
at runtime so this must be the working directory.

```bash
# Desktop game (bracket-lib)
cargo run --bin rogue
cargo run --bin rogue -- --demo   # autopilot demo mode

# macroquad native game
cargo run -p rogue-web

# WASM build
cargo build --release -p rogue-web --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/rogue-web.wasm rogue-web/web/
# Then bump ?v=N in rogue-web/web/index.html and serve:
python -m http.server 8081 --directory rogue-web/web/

# Tests
cargo test --workspace                                             # full suite
cargo test -p rogue-core                                          # data + map tests
cargo test -p rogue-cli --test bot_playthrough                    # headless bot tests
cargo test -p rogue-cli --test bot_playthrough bot_descends_from_the_first_level  # single test

# Lint
cargo clippy --workspace
```

---

## Architecture

The workspace has four crates with a strict dependency hierarchy:

```
rogue-core        (no renderer — pure Rogue rules, data, map, combat)
    └── rogue-engine  (game loop + ECS + FrameBuffer; compiles to wasm32)
            ├── rogue-cli   (bracket-lib desktop frontend — thin wrapper)
            └── rogue-web   (macroquad frontend — native + WASM)
```

### rogue-core
Pure-logic library. No ECS, no rendering. Key modules:
- `data.rs` — RON config/monsters/items, embedded with `include_str!`; `GameData::bundled()`
- `gen/defs.rs` — `Dungeon`, `LevelDef`, `DungeonFile`, `Dungeon::bundled()`, `Dungeon::load()`
- `gen/procedural.rs` — faithful port of `rooms.c`/`passages.c`: 3×3 room grid, MST corridors
- `gen/fixed.rs` — ASCII map loader; built-in character vocabulary + custom legend
- `map.rs` — `Map`, tile grid, FOV, pathfinding (`find_path`), spawn list
- `combat.rs` — `roll_attack()`, `str_plus`/`add_dam` tables from `fight.c`
- `geometry.rs` — `Point`, `Rect`

### rogue-engine
Complete game in one crate; no bracket-lib. Used by the WASM frontend.
- `game::Game::tick(Option<GameAction>) -> &FrameBuffer` — single entry point per frame
- `action::GameAction` — all player intents; frontends translate their key events into this
- `framebuffer::{Cell, FrameBuffer}` — 80×28 cell grid; `set()`, `set_raw()`, `print()`, `print_centered()`, `draw_box()`
- `theme::Theme` — color tints; colors are always `(u8, u8, u8)` — never bracket-lib `RGB`
- `components.rs` — hecs ECS component types (Position, Renderable, Stats, Monster, Item, …)
- `scores.rs` — high score persistence; `std::fs` guarded with `#[cfg(not(target_arch = "wasm32"))]`

### rogue-cli
Thin bracket-lib wrapper. `game.rs` is a near-duplicate of `rogue-engine/src/game.rs` using
`ctx: &mut BTerm` instead of `Option<GameAction>`, and `to_cp437(ch)` instead of `fb.set()`.
`main.rs` builds the BTerm context.

### rogue-web
macroquad frontend. `keys.rs` owns `InputState` (hold-to-repeat: 200 ms delay, 60 ms repeat).
WASM entropy uses a custom `getrandom` backend seeded from miniquad's `now()` import.

---

## Key Conventions

### Dual-file game logic (critical)
`rogue-cli/src/game.rs` and `rogue-engine/src/game.rs` contain nearly identical game
logic. **Every gameplay change must be applied to both files.**

| Aspect | rogue-engine | rogue-cli |
|--------|-------------|-----------|
| Input | `Option<GameAction>` | `ctx.key` (`VirtualKeyCode`) |
| Rendering | `self.fb.print(...)` | `ctx.print_color(...)` |
| Set cell | `self.fb.set(x, y, fg, bg, ch)` | `ctx.set(x, y, fg, bg, to_cp437(ch))` |
| Tiled cell | `self.fb.set_raw(x, y, fg, bg, u16)` | `ctx.set(x, y, fg, bg, u16)` |
| Close inventory | `GameAction::Cancel` / `GameAction::Quit` | `VirtualKeyCode::Escape` |

### Screen layout
```
Row 0:      last message (MSG_ROW)
Rows 1-24:  map tiles    (MAP_TOP=1, map_height=24 per config.ron)
Row 25:     status bar   (Level/HP/Str/Arm/Gold/Exp/Depth)
Row 26:     context hint (standing on stairs, item, etc.)
Row 27:     key reminder footer
```
`SCREEN_WIDTH=80`, `SCREEN_HEIGHT=28`, `MAP_TOP=1` — constants in both game files.

### Game loop / Mode state machine
`Mode` enum drives tick(): `Title → Playing → Help | Inventory | MessageLog | ConfirmQuit | Dead | Won | Scores`.
`Mode::Inventory` is interactive (letter → use item; j/k scroll; Esc close). `Mode::MessageLog` scrolls with j/k.

### ECS
Uses `hecs`. The player entity is stored as `pub player: Entity` on `Game`. Queries use `world.query::<(&Component, …)>().iter()`. Component types all live in `components.rs`.

### WASM constraints
- **Never** use `getrandom = { features = ["js"] }` — pulls in wasm-bindgen which conflicts with miniquad's JS glue. Use `features = ["custom"]` (implementation in `rogue-web/src/main.rs`).
- `Dungeon::load()` returns `None` on WASM; the game always falls back to `Dungeon::bundled()` which embeds all level files via `include_str!` in `rogue-core/src/gen/defs.rs`.
- `std::fs` in `scores.rs` is behind `#[cfg(not(target_arch = "wasm32"))]`.
- After any change: rebuild WASM, copy `.wasm` to `rogue-web/web/`, bump `?v=N` in `index.html`.

### Adding a dungeon level
1. Create `assets/levels/<name>.ron` — rows must be exactly **80 chars wide**.
2. Add `File("<name>.ron")` to `assets/dungeon.ron`.
3. Add embedded constant + match arm to `bundled_level_text()` in `rogue-core/src/gen/defs.rs`.
4. The flood-fill reachability test catches disconnected rooms automatically.

**Built-in map characters:**

| Char | Meaning |
|------|---------|
| ` ` | void |
| `.` | floor |
| `#` | corridor/passage |
| `\|` `-` | wall |
| `+` | door |
| `>` `<` | stairs down/up |
| `^` | trap |
| `@` | player start |
| `$` | gold |
| `*` | random item |
| `A`–`Z` | monster by glyph |

Custom legend entries can override any character: `(ch: 'o', tile: Some(Floor), spawn: Some(Gold(Some(75))))`.

### Themes / Color system
`Theme` (in both `rogue-engine/src/theme.rs` and `rogue-cli/src/theme.rs`) applies optional tints. Colors are `(u8, u8, u8)` everywhere. Available themes: `classic` (white), `amber` (255,180,60 tint), `green` (80,255,80 tint), `boxy` (CP437 box-drawing chars), `tiled` (pixel-art sprites from `rogue_tiles.png`).

`dim()` in `game.rs` scales explored-but-not-visible tiles to 55% brightness. Keep color values in sync between both game files.

### Inventory system
- Up to 26 items: labels `a)`–`z)`; overflow slots 26–35: labels `0)`–`9)`
- `use_inventory_item(idx)` dispatches to the correct action per `ItemKind`
- Action hints displayed: `[quaff]` `[eat]` `[read]` `[wield]` `[wear]` `[put on]` `[zap]`
- Key translation: `vk_to_inv_idx(VirtualKeyCode, scroll)` in rogue-cli; `char_to_inv_idx(char, scroll)` in rogue-engine

### Autopilot bot
Priority order each turn:
1. Emergency heal (HP < 50%)
2. Pickup item at current tile
3. Explore unrevealed frontier (BFS) — skipped when HP < 35%
4. Engage nearest visible monster
5. Collect nearest floor item
6. Head to Amulet or stairs and descend

While running: renders a 4-line bordered log overlay in the bottom-right corner (rows 19-24, cols 42-79) using ASCII `+/-/:` border chars.

### bot_playthrough tests
`rogue-cli/tests/bot_playthrough.rs` runs the autopilot headlessly with a fixed seed against the real `assets/` dungeon. Turn limits: 5 000 (level 1 descent) and 20 000 (depth ≥ 2). These tests catch broken dungeon connectivity.

---

## Legacy Codebase Reference (`legacy/`)

**Do not modify.** Preserved from [github.com/mikeyk730/Rogue-Collection](https://github.com/mikeyk730/Rogue-Collection).

Useful as a reference when implementing Rogue mechanics:
- `legacy/src/RogueVersions/Rogue_5_4_2/` — the canonical C source for rogue-rs
- `legacy/docs/versions.md` — behavioral differences between all six versions
- `legacy/docs/rogomatic.md` — Rog-O-Matic AI documentation
- `legacy/src/RogueVersions/Rogue_5_4_2/LICENSE.TXT` — original game license (BSD 3-Clause, Michael Toy, Ken Arnold, Glenn Wichman)

The `rogue-core` combat module (`combat.rs`) is a direct port of `fight.c` strength tables and `swing()` to-hit logic. The procedural level generator (`gen/procedural.rs`) ports `rooms.c`/`passages.c`.

