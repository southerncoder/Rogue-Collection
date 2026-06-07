# Credits & Licenses

## Legacy Codebase — Retro Rogue Collection

The `legacy/` directory is a verbatim preservation of the
**[Retro Rogue Collection](https://github.com/mikeyk730/Rogue-Collection)**
by **mikeyk730** (Mike Kamermans).

It contains:
- Six historical Rogue versions (PC v1.48, v1.1; Unix v5.4.2, v5.3, v5.2.1, v3.6.3)
- SDL2 frontend (RogueCollectionCpp)
- Qt/QML frontend (RetroRogueCollection)
- Rog-O-Matic AI
- Build infrastructure (Visual Studio, CMake)

**Do not modify `legacy/` source files.** The tree is preserved intact as the authoritative source for original game rules, behavior, data, and history.

---

## Rogue-rs — Modern Rust Reimplementation

MIT License — `rogue-rs/` (Cargo workspace `license = "MIT"`).

Built from scratch in Rust, using Unix Rogue v5.4.2 as the behavioral reference.

---

## Original Rogue Game

All Unix versions (`legacy/src/RogueVersions/Rogue_3_6_3/`, `Rogue_5_2_1/`, `Rogue_5_4_2/`):

> Rogue: Exploring the Dungeons of Doom  
> Copyright © 1980–1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman  
> All rights reserved.  
> BSD 3-Clause License — see `legacy/src/RogueVersions/Rogue_5_4_2/LICENSE.TXT`

**Save/restore state** (Nicholas J. Kisseberth):
> Copyright © 1999, 2000, 2005–2006 Nicholas J. Kisseberth  
> BSD 3-Clause — included in `Rogue_5_4_2/LICENSE.TXT` and `Rogue_3_6_3/LICENSE.TXT`

**FreeSec encryption** (David Burren):
> FreeSec: libcrypt — Copyright © 1994 David Burren  
> BSD 3-Clause — included in both version LICENSE.TXT files

---

## Fonts

| Font | Location | License |
|------|----------|---------|
| IBM PC VGA (1985) | `…/fonts/1985-ibm-pc-vga/` | See `LICENSE.TXT` |
| IBM 3278 (1971) | `…/fonts/1971-ibm-3278/` | See `LICENSE.txt` |
| Modern Pro Font | `…/fonts/modern-pro-font-win-tweaked/` | See `LICENSE` |
| Hermit | `…/fonts/modern-hermit/` | See `LICENSE` |

All font license texts are in `legacy/src/RogueCollectionQml/RetroRogueCollection/qml/fonts/`.

---

## Third-party libraries (legacy/)

| Library | License | Location |
|---------|---------|----------|
| SDL2 | zlib | Distributed binaries only |
| SDL2_image | zlib | `legacy/lib/SDL2_image-2.0.1/lib/*/LICENSE.zlib.txt` |
| SDL2_image (PNG) | PNG/zlib | `legacy/lib/SDL2_image-2.0.1/lib/*/LICENSE.png.txt` |
| SDL2_ttf | zlib | `legacy/lib/SDL2_ttf-2.0.14/lib/*/LICENSE.zlib.txt` |
| FreeType | FreeType | `legacy/lib/SDL2_ttf-2.0.14/lib/*/LICENSE.freetype.txt` |
| NativeFileDialog | See license | `legacy/lib/NativeFileDialog/LICENSE.NativeFileDialog.txt` |

---

## Rust crate licenses (rogue-rs/)

| Crate | License |
|-------|---------|
| bracket-lib | MIT |
| hecs | MIT/Apache-2.0 |
| macroquad | MIT/Apache-2.0 |
| serde | MIT/Apache-2.0 |
| ron | MIT/Apache-2.0 |
| rand | MIT/Apache-2.0 |
| getrandom | MIT/Apache-2.0 |
