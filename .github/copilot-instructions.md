# Copilot Instructions — Rogue-rs

> Full docs: [`rogue-rs/docs/`](../rogue-rs/docs/) — architecture, level authoring, themes, gameplay, credits.

---

## Build & Test

Run all commands from `rogue-rs/` (the workspace root — assets are loaded relative to cwd).

```bash
cargo run --bin rogue                        # bracket-lib desktop
cargo run -p rogue-web                       # macroquad desktop
cargo test --workspace                       # full suite
cargo test -p rogue-cli --test bot_playthrough bot_descends_from_the_first_level  # single test
cargo clippy --workspace
```

**WASM build + deploy:**
```bash
cargo build --release -p rogue-web --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/rogue-web.wasm rogue-web/web/
# bump ?v=N in rogue-web/web/index.html, then serve rogue-web/web/
```

---

## Critical rules

### 1. Dual-file sync
`rogue-cli/src/game.rs` and `rogue-engine/src/game.rs` contain nearly identical game logic.
**Every gameplay change must be applied to both files.**

| | rogue-engine | rogue-cli |
|---|---|---|
| Input | `Option<GameAction>` | `ctx.key` (`VirtualKeyCode`) |
| Set cell | `self.fb.set(x, y, fg, bg, ch: char)` | `ctx.set(x, y, fg, bg, to_cp437(ch))` |
| Print | `self.fb.print(x, y, fg, bg, s)` | `ctx.print_color(x, y, fg, bg, s)` |

### 2. WASM constraints
- **Never** use `getrandom = { features = ["js"] }` — breaks WASM. Use `features = ["custom"]`.
- `std::fs` must be `#[cfg(not(target_arch = "wasm32"))]` guarded.
- New hand-authored levels need an `include_str!` constant + match arm in `rogue-core/src/gen/defs.rs` (`bundled_level_text`).
- After any engine change: rebuild WASM and bump the `?v=N` cache-buster in `index.html`.

### 3. Colors
All colors are `(u8, u8, u8)` — never bracket-lib `RGB`. Keep values in sync between both `theme.rs` files.

---

## Key file locations

| What | Where |
|------|-------|
| Game loop (engine) | `rogue-engine/src/game.rs` |
| Game loop (CLI) | `rogue-cli/src/game.rs` |
| Player input enum | `rogue-engine/src/action.rs` |
| Framebuffer API | `rogue-engine/src/framebuffer.rs` |
| ECS components | `rogue-engine/src/components.rs` |
| Color themes | `rogue-engine/src/theme.rs` / `rogue-cli/src/theme.rs` |
| Dungeon + level embed | `rogue-core/src/gen/defs.rs` |
| Web key input | `rogue-web/src/keys.rs` |
| Dungeon definition | `assets/dungeon.ron` |
| Level files | `assets/levels/*.ron` |

---

## Attribution
The `legacy/` directory is preserved from the **[Retro Rogue Collection](https://github.com/mikeyk730/Rogue-Collection)** by mikeyk730. Do not modify it. Original Rogue © 1980–1985 Michael Toy, Ken Arnold, Glenn Wichman (BSD 3-Clause). See [`rogue-rs/docs/credits-and-licenses.md`](../rogue-rs/docs/credits-and-licenses.md).