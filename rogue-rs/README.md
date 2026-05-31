# Rogue — Rust edition

A modern, data‑driven reimplementation of the classic dungeon crawler
**Rogue**, written in Rust. The rules follow Unix **Rogue v5.4.2** (preserved
for reference under [`../legacy/`](../legacy)).

It is built to be two things at once:

1. **Playable** — `cargo run` drops you straight into the Dungeons of Doom.
2. **Easy to extend** — levels, monsters and items are plain data files
   ([RON](https://github.com/ron-rs/ron)). Adding content needs *no code
   changes*, and levels can be either **procedurally generated** *or*
   **hand‑authored** ASCII maps.

---

## Quick start

```bash
cd rogue-rs
cargo run            # play
cargo run -- --demo  # watch the autopilot bot play itself
cargo test           # run the test suite (incl. the bot playthrough)
cargo clippy         # lints
```

> Run from the `rogue-rs/` directory so the game can find the `assets/` folder.
> If `assets/` is missing, the game falls back to a fully procedural dungeon.

Requires a recent stable Rust toolchain and an OpenGL‑capable display
(rendering uses [`bracket-lib`](https://github.com/amethyst/bracket-lib)).

---

## Controls

| Action            | Keys                                             |
| ----------------- | ------------------------------------------------ |
| Move / attack     | `h j k l`, `y u b n` (diagonals), or arrow keys / numpad |
| Wait one turn     | `.`                                              |
| Descend stairs    | `>` (while standing on `>`)                       |
| Pick up item      | `g`                                              |
| Quaff healing     | `q`                                              |
| Eat food          | `e`                                              |
| Show help         | `?` (any key closes it)                          |
| Autopilot bot     | `A` (toggle — a pathfinding bot plays for you)   |
| Start / restart   | `Enter`                                          |
| Quit              | `Esc` (then confirm with `Y` / `Enter`)          |

**Watch the bot play.** Press `A` in game (or launch with `cargo run -- --demo`)
to hand control to a pathfinding autopilot. It routes to the down‑stairs each
level (fighting anything in the way) and grabs the Amulet when it reaches it —
handy for demoing or sanity‑checking that a level is actually completable. The
status line shows `[AUTO]` while it's driving; press `A` again to take over.

**Goal:** descend to the Amulet of Yendor (level 26), pick it up, and win.
Watch your **HP**, manage **hunger** (eat before you starve), and pick your
fights — monsters get nastier the deeper you go.

Map glyphs: `@` you · `A`–`Z` monsters · `.` floor · `#` wall/corridor ·
`+` door · `>` stairs down · `^` trap · `!` potion · `:` food · `)` weapon ·
`[` armor · `&` the Amulet.

---

## How to add a level

A whole dungeon is described by [`assets/dungeon.ron`](assets/dungeon.ron): an
ordered list of levels. Each entry is one of:

```ron
Procedural(())                 // generated with classic Rogue rules (defaults)
Procedural((depth: Some(5)))   // generated, but with depth-5 difficulty
Fixed(( rows: [ ... ] ))       // an inline hand-authored map
File("entrance.ron")           // load a level from assets/levels/entrance.ron
```

`repeat_last: true` keeps generating procedural levels forever once you pass the
last defined entry, so the dungeon never runs out.

### 1. Add a procedural level

Just add an entry to the `levels` list in `dungeon.ron`:

```ron
Procedural((
    depth: Some(8),          // optional: difficulty override (default = real depth)
    allow_dark: true,        // dark rooms appear deeper down
    allow_gone_rooms: true,  // some rooms become corridor junctions
)),
```

That's it — save and run. No recompile needed.

### 2. Add a hand‑authored (fixed) level

Create a file under [`assets/levels/`](assets/levels) — see
[`entrance.ron`](assets/levels/entrance.ron) and
[`vault.ron`](assets/levels/vault.ron) for worked examples — then reference it
from `dungeon.ron` with `File("yourlevel.ron")`.

A fixed level is just rows of text plus an optional legend:

```ron
Fixed((
    rows: [
        "  ----------    ",
        "  |..@...>.|    ",
        "  |..o....E|    ",
        "  ----------    ",
    ],
    legend: [
        // Override or extend the built-in characters.
        (ch: 'o', tile: Some(Floor), spawn: Some(Gold(Some(75)))),
    ],
    dark: false,
))
```

**Built‑in legend** (always available, override via `legend`):

| Char  | Meaning                         |
| ----- | ------------------------------- |
| ` `   | empty rock / void               |
| `.`   | floor                           |
| `#`   | corridor                        |
| `|` `-` | wall                          |
| `+`   | door                            |
| `>`   | stairs down                     |
| `<`   | stairs up                       |
| `^`   | trap                            |
| `@`   | player start                    |
| `$`   | gold (value rolled by depth)    |
| `*`   | a random item                   |
| `A`–`Z` | monster by its glyph          |

**Custom legend entries** map any character to a `tile` and/or a `spawn`:

- `tile`: `Empty`, `Floor`, `Wall`, `Passage`, `Door`, `StairsDown`,
  `StairsUp`, `Trap`.
- `spawn`: `Monster(Some('K'))` / `Monster(None)` (random), `Item`,
  `Gold(Some(75))` / `Gold(None)` (roll by depth), `Player`, `StairsDown`.

---

## How to add monsters or items

Edit the data files directly — they are loaded at runtime:

- [`assets/monsters.ron`](assets/monsters.ron) — the 26 archetypes (`A`–`Z`),
  with stats, flags, armor and damage strings like `"1x8"` or `"1x8/1x8/3x10"`.
- [`assets/items.ron`](assets/items.ron) — potion / scroll / ring / wand /
  weapon / armor tables and their spawn weights.
- [`assets/config.ron`](assets/config.ron) — tunables (map size, hunger,
  starting HP/strength/armor, amulet depth, …).

The default content is also embedded in the binary, so the game still runs if
the files are absent.

---

## Architecture

A small Cargo workspace:

```
rogue-rs/
├─ rogue-core/      # engine-agnostic library (no renderer)
│  ├─ data.rs       #   data-driven content (serde/RON) + loading
│  ├─ dice.rs       #   "NxM/..." damage parser
│  ├─ combat.rs     #   faithful 5.4.2 to-hit / damage (str tables)
│  ├─ map.rs        #   tile grid, visibility, rooms, spawns
│  ├─ rng.rs        #   Rogue-style rnd()/roll()/percent()
│  └─ gen/          #   level generation
│     ├─ procedural.rs  # faithful 3x3-room + MST-corridor generator
│     ├─ fixed.rs       # ASCII fixed-map loader
│     └─ defs.rs        # serde level/dungeon definitions
└─ rogue-cli/       # bracket-lib frontend (hecs ECS, input, rendering)
   ├─ components.rs
   └─ game.rs       # world building, turn loop, combat, UI
```

- **Engine:** [`bracket-lib`](https://github.com/amethyst/bracket-lib)
  (terminal rendering; native + WASM capable).
- **ECS:** [`hecs`](https://github.com/Ralith/hecs).
- **Content:** [`serde`](https://serde.rs/) + [`ron`](https://github.com/ron-rs/ron).

The procedural generator is a faithful port of `rooms.c` / `passages.c` /
`new_level.c`: a 3×3 grid of rooms connected by a hardcoded adjacency graph via
a spanning tree plus a few extra loop corridors, with dark rooms, "gone" rooms
(corridor junctions), traps, gold, items and a down staircase. Combat ports the
`str_plus` / `add_dam` tables and the `swing()` to‑hit roll from `fight.c`.

---

## Testing

```bash
cargo test     # rogue-core unit tests + rogue-cli gameplay smoke tests
```

Highlights:

- Every probability table is validated to sum to 100.
- Generated **and hand‑authored** levels are flood‑filled to prove the stairs
  (and every authored monster/item/gold spawn) are reachable from the player
  start — a quality gate that catches off‑by‑one corridors and disconnected
  rooms (20 procedural seeds + the example dungeon's first 8 levels).
- The example dungeon (incl. fixed levels) parses, resolves and builds.
- A headless **autopilot bot** plays the real dungeon end‑to‑end: one test
  proves it can descend from the first level, another that it pushes several
  levels deep. (See `rogue-cli/tests/bot_playthrough.rs`.)

---

## Status & scope

This is a faithful, playable core — not a 1:1 reproduction of every 5.4.2
feature. Implemented: movement, room/corridor lighting, bump combat, monster
AI (wake‑on‑sight chase), hunger, regeneration, XP/leveling, gold, item
pickup/use (healing potions, food, weapons, armor), descending, death/win.
Deliberately left for later: scroll/wand/ring effects, throwing, traps variety,
identifying items, and saving. The data‑driven design makes these additive.
