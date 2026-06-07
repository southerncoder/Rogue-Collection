# Copilot Instructions — Rogue-rs

## Build, Test, Lint

All commands run from `rogue-rs/` (the Cargo workspace root). The game reads `assets/` at runtime so it must be the working directory.

```bash
# Run the bracket-lib desktop game
cargo run --bin rogue

# Run the macroquad desktop game
cargo run -p rogue-web

# Build the WASM binary
cargo build --release -p rogue-web --target wasm32-unknown-unknown

# Deploy WASM (copy output + serve)
cp target/wasm32-unknown-unknown/release/rogue-web.wasm rogue-web/web/
python -m http.server 8081 --directory rogue-web/web/

# Run all tests
cargo test --workspace

# Run a single test
cargo test -p rogue-cli --test bot_playthrough bot_descends_from_the_first_level

# Lint
cargo clippy --workspace
```

## Architecture

The workspace has four crates with a strict dependency hierarchy:

```
rogue-core        (no renderer, no ECS — pure Rogue rules + data)
    └── rogue-engine  (full game loop + ECS; no bracket-lib; compiles to wasm32)
            ├── rogue-cli   (bracket-lib desktop frontend — thin wrapper)
            └── rogue-web   (macroquad frontend — native + WASM)
```

### rogue-core
Pure-logic library: map generation (`gen/`), combat (`combat.rs`), dice parsing (`dice.rs`), RON data loading (`data.rs`). Has no rendering or input dependencies. Assets are embedded via `include_str!` for WASM compatibility (`data.rs` lines 165-167). `Dungeon::bundled()` embeds all level files at compile time.

### rogue-engine
The game loop that any frontend uses. Key surfaces:
- `game::Game::tick(Option<GameAction>) -> &FrameBuffer` — advance one frame
- `action::GameAction` — all player intents (frontends translate their key types into this)
- `framebuffer::FrameBuffer` — 80×28 cell grid frontends blit to screen
- Colors are always `(u8, u8, u8)` tuples — never bracket-lib `RGB`

All game logic changes (combat, rendering, autopilot, inventory, themes) go in `rogue-engine/src/game.rs` **and** its mirror `rogue-cli/src/game.rs` — these two files must stay in sync.

### rogue-cli
Thin bracket-lib wrapper. `main.rs` sets up BTerm; `game.rs` contains the full duplicated game logic (kept in sync with `rogue-engine/src/game.rs`). Uses `VirtualKeyCode` directly for input.

### rogue-web
macroquad frontend. `src/keys.rs` owns an `InputState` struct that handles hold-to-repeat for movement keys (200 ms initial delay, 60 ms repeat). `main.rs` is the entry point for both native and WASM.

## Key Conventions

### Dual-file game logic
`rogue-cli/src/game.rs` and `rogue-engine/src/game.rs` contain nearly identical game logic. **Any gameplay change must be applied to both files.** The engine version uses `self.fb.*` (FrameBuffer API) while the CLI version uses `ctx.*` (BTerm API). The engine version accepts `Option<GameAction>` and the CLI version matches on `ctx.key` (`VirtualKeyCode`).

### WASM constraints
- Never use `getrandom` with `features = ["js"]` — it pulls in wasm-bindgen which conflicts with miniquad's JS glue. Use `features = ["custom"]` in `rogue-web` instead (implementation in `main.rs`).
- `std::fs` operations are guarded with `#[cfg(not(target_arch = "wasm32"))]` in `scores.rs`.
- `Dungeon::load()` returns `None` on WASM; `Dungeon::bundled()` is the fallback.
- After any gameplay change, rebuild WASM: `cargo build --release -p rogue-web --target wasm32-unknown-unknown` and copy the `.wasm` to `rogue-web/web/`. Bump the `?v=N` cache-buster in `rogue-web/web/index.html`.

### Adding a dungeon level
1. Create `assets/levels/<name>.ron` — see existing files for format (`Fixed((rows: [...], legend: [...], dark: false))`).
2. Add `File("<name>.ron")` to `assets/dungeon.ron`.
3. Add the embedded constant + match arm to `rogue-core/src/gen/defs.rs` (`bundled_level_text` function) so WASM can load it.
4. Map rows must be exactly 80 characters wide. Player start = `@`, stairs = `>`, monsters by glyph `A`-`Z`, gold = `$`, random item = `*`.
5. The flood-fill connectivity test (`rogue-core` unit tests) will catch disconnected rooms.

### Theme system
Colors are `(u8, u8, u8)` throughout `rogue-engine`. The `Theme` struct (`rogue-engine/src/theme.rs`) applies tints for amber/green modes. `dim()` in `game.rs` dims explored-but-not-visible tiles. The `rogue-cli` has a parallel `theme.rs` that returns bracket-lib `RGB` values — keep these in sync when changing color values.

### Screen layout
```
Row 0        : last message
Rows 1-24    : map tiles  (map_width=80, map_height=24 per config.ron)
Row 25       : status bar (Level/HP/Str/Arm/Gold/Exp/Depth)
Row 26       : context hint (e.g. "Press > to descend")
Row 27       : key reminder footer
```
`SCREEN_WIDTH=80`, `SCREEN_HEIGHT=28`, `MAP_TOP=1` are constants in both game files.

### Inventory interaction
The inventory (`Mode::Inventory`) is interactive: letter keys (`a`-`z`) directly use items; digits (`0`-`9`) cover slots 26-35. Items display action hints (`[quaff]`, `[read]`, etc.). The CLI uses `vk_to_inv_idx(VirtualKeyCode)` and the engine uses `char_to_inv_idx(char)`.

### bot_playthrough tests
`rogue-cli/tests/bot_playthrough.rs` runs the autopilot headlessly. These tests load the real `assets/` dungeon and verify the bot can descend. Turn limits are generous (5 000 / 20 000) because the bot now fully explores before descending.
