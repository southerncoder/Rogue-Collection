# Level Authoring

Fixed (hand-authored) levels are defined in RON files under `assets/levels/`.
The dungeon order is set in `assets/dungeon.ron`.

---

## dungeon.ron

```ron
(
    name: "The Dungeons of Doom",
    levels: [
        File("rogue.ron"),      // hand-authored fixed level
        File("entrance.ron"),
        File("vault.ron"),
        Procedural(()),         // generated with classic Rogue algorithm
        Procedural((depth: Some(5))),  // procedural, depth-5 difficulty
    ],
    repeat_last: true,          // keep generating procedural levels after the last entry
)
```

---

## Level file format

```ron
Fixed((
    rows: [
        "                                                                                ",
        "  |------------|     |------------|",
        "  |............|     |............|",
        // ... exactly 80 characters per row, exactly map_height (24) rows
    ],
    legend: [
        // optional: override or extend built-in characters
        (ch: 'o', tile: Some(Floor), spawn: Some(Gold(Some(75)))),
        (ch: 'P', tile: Some(Floor), spawn: Some(Item)),
    ],
    dark: false,
))
```

**Every row must be exactly 80 characters.** The map height from `config.ron` is 24 rows.

---

## Built-in character vocabulary

| Char | Tile | Spawn |
|------|------|-------|
| ` ` | Empty (void) | — |
| `.` | Floor | — |
| `#` | Passage (corridor) | — |
| `\|` | Wall (vertical) | — |
| `-` | Wall (horizontal) | — |
| `+` | Door | — |
| `>` | StairsDown | — |
| `<` | StairsUp | — |
| `^` | Trap | — |
| `@` | Floor | Player start |
| `$` | Floor | Gold (value rolled by depth) |
| `*` | Floor | Random item |
| `A`–`Z` | Floor | Monster by that glyph |

---

## Custom legend entries

Override any character or add new ones:

```ron
legend: [
    // Fixed gold amount
    (ch: 'o', tile: Some(Floor), spawn: Some(Gold(Some(75)))),
    // Gold rolled by depth
    (ch: 'g', tile: Some(Floor), spawn: Some(Gold(None))),
    // Random item
    (ch: 'P', tile: Some(Floor), spawn: Some(Item)),
    // Specific monster
    (ch: 'D', tile: Some(Floor), spawn: Some(Monster(Some('D')))),
    // Random depth-appropriate monster
    (ch: 'm', tile: Some(Floor), spawn: Some(Monster(None))),
    // Explicit tile, no spawn
    (ch: 'x', tile: Some(StairsDown), spawn: None),
]
```

Available tile values: `Empty`, `Floor`, `Wall`, `Passage`, `Door`, `StairsDown`, `StairsUp`, `Trap`.

---

## Procedural level parameters

```ron
Procedural((
    depth: Some(8),          // optional: override difficulty (default = real depth)
    allow_dark: true,        // dark rooms appear deeper down (default: true)
    allow_gone_rooms: true,  // some rooms become corridor junctions (default: true)
))
```

---

## Adding a new level (step by step)

1. Create `assets/levels/<name>.ron` with the Fixed() definition above.
2. Add `File("<name>.ron")` to `assets/dungeon.ron` at the desired position.
3. **For WASM:** add to `rogue-core/src/gen/defs.rs`:
   ```rust
   const LEVEL_MYMAP: &str = include_str!("../../../assets/levels/mymap.ron");
   
   fn bundled_level_text(name: &str) -> Option<&'static str> {
       match name {
           "mymap.ron" => Some(LEVEL_MYMAP),
           // ...
       }
   }
   ```
4. Run `cargo test -p rogue-core` — the flood-fill reachability test catches disconnected rooms.
5. Run `cargo run --bin rogue` from `rogue-rs/` to play immediately (no recompile for assets on native).
6. Rebuild WASM if the level should appear in the browser: see [wasm.md](wasm.md).

---

## Design tips

- Rooms need `|`/`-` walls enclosing `.` floor cells. Doors `+` must be in wall cells adjacent to passages `#`.
- Two rooms sharing a wall row/column can be connected by replacing one wall cell with `+`.
- Two rooms with a gap need `#` corridor cells between them plus `+` doors in the wall cells at each end.
- Leave 2+ void columns between letter/shape groups for visual clarity.
- The player always starts at `@`. The game panics if no `@` is found.
- `>` (stairs) must be reachable from `@` — the flood-fill test enforces this.
