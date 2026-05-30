# Rogue Collection

This repository now contains two parts:

## `rogue-rs/` — Modern Rust Rogue (active)

A from-scratch, data-driven reimplementation of Rogue in modern Rust, built on
[`bracket-lib`](https://github.com/amethyst/bracket-lib) (terminal rendering, FOV,
pathfinding), a lightweight ECS, and `serde`/RON for content.

Goals:
- **Playable**: `cd rogue-rs && cargo run`.
- **Easy to extend**: add new dungeon levels by dropping data files in
  `rogue-rs/assets/levels/` — supporting both **procedural** generation and
  **hand-authored fixed maps**. No recompile required.

The rules follow Unix **Rogue v5.4.2** as the canonical reference.

See [`rogue-rs/README.md`](rogue-rs/README.md) to play and to add levels.

## `legacy/` — Original C/C++ collection (preserved for reference)

The original _Retro Rogue Collection_ by mikeyk730: six historical versions of Rogue
(PC v1.48, PC v1.1, Unix v5.4.2, v5.3, v5.2.1, v3.6.3) plus the SDL2 and Qt/QML
frontends, Rog-O-Matic, and build files. This tree is kept intact (with git history)
as the authoritative source of the original game's rules and behavior. See
[`legacy/readme.md`](legacy/readme.md).
