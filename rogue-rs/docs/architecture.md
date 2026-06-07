# Architecture

## Crate Dependency Graph

```
rogue-core        (no renderer — pure Rogue rules, data, map, combat)
    └── rogue-engine  (game loop + ECS + FrameBuffer; compiles to wasm32)
            ├── rogue-cli   (bracket-lib desktop frontend — thin wrapper)
            └── rogue-web   (macroquad frontend — native + WASM)
```

---

## rogue-core

Engine-agnostic library. No ECS, no rendering, no bracket-lib.

| Module | Purpose | Legacy reference |
|--------|---------|-----------------|
| `data.rs` | RON config / monsters / items; `GameData::bundled()` embeds via `include_str!` | — |
| `gen/defs.rs` | `Dungeon`, `LevelDef`, `DungeonFile`; `Dungeon::bundled()` + `Dungeon::load()` | — |
| `gen/procedural.rs` | 3×3 room grid, MST corridors, dark rooms, "gone" rooms | `rooms.c`, `passages.c`, `new_level.c` |
| `gen/fixed.rs` | ASCII fixed-map loader; built-in character vocabulary + custom legend | — |
| `map.rs` | `Map`, tile grid, FOV, pathfinding (`find_path`), spawn list | — |
| `combat.rs` | `roll_attack()`, `str_plus`/`add_dam` tables, `swing()` to-hit | `fight.c` |
| `dice.rs` | Damage roll parser (`"2x4"`, `"1x8/1x8/3x10"`) | — |
| `geometry.rs` | `Point`, `Rect` | — |
| `rng.rs` | `RogueRng` trait; `rnd()`, `roll()`, `percent()` | `rogue.h` macros |

### Embedded assets
`data.rs` and `gen/defs.rs` both use `include_str!` to embed asset files at compile time so WASM can access them without a filesystem:

```rust
// data.rs
const CONFIG_RON:   &str = include_str!("../../assets/config.ron");
const MONSTERS_RON: &str = include_str!("../../assets/monsters.ron");
const ITEMS_RON:    &str = include_str!("../../assets/items.ron");

// gen/defs.rs
const DUNGEON_RON:    &str = include_str!("../../../assets/dungeon.ron");
const LEVEL_ROGUE:    &str = include_str!("../../../assets/levels/rogue.ron");
const LEVEL_ENTRANCE: &str = include_str!("../../../assets/levels/entrance.ron");
const LEVEL_VAULT:    &str = include_str!("../../../assets/levels/vault.ron");
```

`bundled_level_text(name)` in `defs.rs` maps level file names to embedded constants — update this when adding a new hand-authored level.

---

## rogue-engine

Complete self-contained game. No bracket-lib. Target: `wasm32-unknown-unknown`.

### Public API surface

```rust
use rogue_engine::{game::Game, action::GameAction};

let mut g = Game::new();
loop {
    let action: Option<GameAction> = /* translate input */;
    let fb: &FrameBuffer = g.tick(action);
    /* blit fb.cells to screen */
    if fb.wants_quit { break; }
}
```

### Key types

| Type | File | Notes |
|------|------|-------|
| `Game` | `game.rs` | Full ECS world, map, dungeon, log, mode state machine |
| `GameAction` | `action.rs` | All player intents — frontends map their key type to this |
| `FrameBuffer` | `framebuffer.rs` | 80×28 `Vec<Cell>`; `set()`, `set_raw()`, `print()`, `draw_box()` |
| `Theme` | `theme.rs` | Color tints; colors always `(u8,u8,u8)` |
| `Scores` / `ScoreEntry` | `scores.rs` | `std::fs` guarded for WASM |
| ECS components | `components.rs` | `Position`, `Renderable`, `Stats`, `Monster`, `Item`, `BlocksTile`, … |

### ECS (hecs)
- Library: [`hecs`](https://github.com/Ralith/hecs) 0.10
- Player entity stored as `game.player: Entity`
- Query pattern: `self.world.query::<(&Position, &Monster)>().iter()`
- Component types in `components.rs` — shared by both `rogue-engine` and `rogue-cli`

### Mode state machine
```
Title
  ↓ Enter
Playing ←──────────────────┐
  ├→ Help                  │ any key / Esc closes
  ├→ Inventory (scrollable)│
  ├→ MessageLog (scrollable)│
  ├→ ConfirmQuit           │
  ├→ Dead ──→ Scores       │
  └→ Won  ──→ Scores       │
                            └── Enter: new game
```

---

## rogue-cli

Thin bracket-lib wrapper. Two source files:

- `main.rs` — BTerm construction (font, window size, tiled/boxy modes)
- `game.rs` — **nearly identical to `rogue-engine/src/game.rs`** with BTerm API instead of FrameBuffer

### CLI vs Engine API differences

| Aspect | rogue-engine | rogue-cli |
|--------|-------------|-----------|
| Input param | `Option<GameAction>` | `ctx: &mut BTerm` (reads `ctx.key`) |
| Set glyph | `self.fb.set(x, y, fg, bg, ch: char)` | `ctx.set(x, y, fg, bg, to_cp437(ch))` |
| Set raw CP437 | `self.fb.set_raw(x, y, fg, bg, u16)` | `ctx.set(x, y, fg, bg, u16)` |
| Print text | `self.fb.print(x, y, fg, bg, s)` | `ctx.print_color(x, y, fg, bg, s)` |
| Print centered | `self.fb.print_centered(y, fg, bg, s)` | `ctx.print_color_centered(y, fg, bg, s)` |
| Draw box | `self.fb.draw_box(x, y, w, h, fg, bg)` | `ctx.draw_box(x, y, w-1, h-1, fg, bg)` |
| Close screen | `GameAction::Cancel` or `GameAction::Quit` | `VirtualKeyCode::Escape` |
| Clear | `self.fb.clear(bg)` | `ctx.cls()` |

---

## rogue-web

macroquad frontend; entry point for both native and WASM builds.

- `src/main.rs` — `#[macroquad::main(window_conf)]`, WASM entropy backend, framebuffer blitter
- `src/keys.rs` — `InputState` with hold-to-repeat (200 ms delay, 60 ms repeat interval)

### WASM entropy
`getrandom` with `features = ["custom"]` (NOT `"js"` — that pulls wasm-bindgen which conflicts with miniquad). Custom backend registered with `register_custom_getrandom!` in `main.rs`, seeded from miniquad's `now()` JS import + XorShift-64.

### Serving locally
```bash
cargo build --release -p rogue-web --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/rogue-web.wasm rogue-web/web/
# bump ?v=N in rogue-web/web/index.html
python -m http.server 8081 --directory rogue-web/web/
```
