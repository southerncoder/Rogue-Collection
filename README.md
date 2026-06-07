# Rogue Collection

This repository contains two parts:

## `rogue-rs/` — Modern Rust Rogue (active)

**Created by [SouthernCoder](https://github.com/southerncoder)** as a from-scratch,
data-driven reimplementation of Rogue in modern Rust. Developed using **AI agentic
coding** with [GitHub Copilot](https://github.com/features/copilot), using the legacy
C/C++ collection below as the behavioral and rules reference.

Built on [`bracket-lib`](https://github.com/amethyst/bracket-lib) (terminal rendering,
FOV, pathfinding), a lightweight ECS, and `serde`/RON for content.

Features:
- **Playable**: `cd rogue-rs && cargo run --bin rogue`
- **Web/WASM**: runs in the browser via macroquad — `cargo build -p rogue-web --target wasm32-unknown-unknown`
- **Easy to extend**: add dungeon levels by dropping RON files in `rogue-rs/assets/levels/` — procedural *or* hand-authored fixed maps. No recompile required for native.
- **Interactive inventory**, autopilot bot, multiple color themes (classic, amber, green, boxy, tiled)

The rules follow Unix **Rogue v5.4.2** as the canonical reference.

See [`rogue-rs/README.md`](rogue-rs/README.md) for full documentation and [`rogue-rs/docs/`](rogue-rs/docs/) for architecture, gameplay, and authoring guides.

## `legacy/` — Original C/C++ Collection (preserved for reference)

The `legacy/` tree is a verbatim preservation of the
**[Retro Rogue Collection](https://github.com/mikeyk730/Rogue-Collection)**
by **mikeyk730** (Mike Kamermans), kept here as the authoritative source for
original game rules, behavior, and data.

Six historical versions of Rogue are included:

| Version | Notes |
|---------|-------|
| PC Rogue v1.48 | Final PC release |
| PC Rogue v1.1 | Early PC port |
| Unix Rogue v5.4.2 | **Canonical reference for rogue-rs rules** |
| Unix Rogue v5.3 (NMT) | Modified at New Mexico Tech |
| Unix Rogue v5.2.1 | |
| Unix Rogue v3.6.3 | Earliest preserved version |

Also includes: SDL2 frontend, Qt/QML (RetroRogueCollection) frontend,
Rog-O-Matic AI, and build infrastructure.

See [`legacy/readme.md`](legacy/readme.md) and
[upstream repository](https://github.com/mikeyk730/Rogue-Collection).

---

## Licenses

### rogue-rs (Rust reimplementation)
MIT — see [`rogue-rs/`](rogue-rs/) (Cargo workspace `license = "MIT"`).

### legacy/ (Retro Rogue Collection — mikeyk730)
The C/C++ collection preserves multiple upstream licenses:

**Original Rogue game source** (all Unix versions):
> Copyright © 1980–1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman.
> BSD 3-Clause License — see `legacy/src/RogueVersions/Rogue_5_4_2/LICENSE.TXT`.

**Save/restore portions** (Nicholas J. Kisseberth):
> Copyright © 1999, 2000, 2005–2006 Nicholas J. Kisseberth. BSD 3-Clause.

**FreeSec encryption** (David Burren):
> Copyright © 1994 David Burren. BSD 3-Clause.

**Fonts:**
- IBM VGA font — `legacy/src/RogueCollectionQml/…/fonts/1985-ibm-pc-vga/LICENSE.TXT`
- IBM 3278 font — `legacy/src/RogueCollectionQml/…/fonts/1971-ibm-3278/LICENSE.txt`
- Modern Pro Font — `legacy/src/RogueCollectionQml/…/fonts/modern-pro-font-win-tweaked/LICENSE`
- Hermit font — `legacy/src/RogueCollectionQml/…/fonts/modern-hermit/LICENSE`

**SDL2 / SDL2_image / SDL2_ttf**: zlib license — see `legacy/lib/SDL2*/lib/*/LICENSE.*.txt`.

**NativeFileDialog**: `legacy/lib/NativeFileDialog/LICENSE.NativeFileDialog.txt`.

All original license texts are preserved verbatim in the `legacy/` subtree.

