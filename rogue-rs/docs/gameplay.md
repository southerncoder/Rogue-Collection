# Gameplay Systems

## Screen layout

```
Row 0:    last message                    (MSG_ROW = 0)
Rows 1–24: map tiles                     (MAP_TOP = 1, map_height = 24)
Row 25:   status bar
Row 26:   context hint (stairs, item, …)
Row 27:   key reminder footer
```

Constants in both game files: `SCREEN_WIDTH = 80`, `SCREEN_HEIGHT = 28`, `MAP_TOP = 1`.

---

## Inventory system

Opening: press `i` while playing.

| Key | Action |
|-----|--------|
| `a`–`z` | Use item in slot 0–25 |
| `0`–`9` | Use item in slot 26–35 |
| `↑` / `k` | Scroll up |
| `↓` / `j` | Scroll down |
| `Esc` or `i` | Close (no turn consumed) |

Each item shows an action hint:

| Item kind | Hint |
|-----------|------|
| Heal / Potion | `[quaff]` |
| Food | `[eat]` |
| Scroll | `[read]` |
| Weapon | `[wield]` |
| Armor | `[wear]` |
| Ring | `[put on]` |
| Wand | `[zap]` |
| Amulet | `[AMULET]` |
| Trinket | `[?]` |

`use_inventory_item(idx)` dispatches to the correct action.

Key translation:
- rogue-engine: `char_to_inv_idx(char, scroll) -> Option<usize>`
- rogue-cli: `vk_to_inv_idx(VirtualKeyCode, scroll) -> Option<usize>`

---

## Autopilot bot

Toggle with `A`. While running: renders a 4-line log overlay in the bottom-right (ASCII `+/-/:` border, rows 19–24, cols 42–79). Newest line in accent color; older lines in dim_ui.

Priority each turn:

1. **Emergency heal** — quaff `Heal` / `Potion(Healing|ExtraHealing)` when HP < 50%
2. **Pickup** — collect item at current tile
3. **Explore** — BFS to nearest unrevealed frontier (skipped when HP < 35%); if monsters visible, engage nearest first
4. **Collect** — walk to nearest revealed floor item
5. **Descend** — head to Amulet of Yendor or down-stairs

Demo mode: `cargo run --bin rogue -- --demo` starts immediately with autopilot.

---

## Combat

`roll_attack(attacker, defender_armor, rng) -> Option<i32>`

Hit roll: `1d20 + level/2 + str_plus(strength) + hit_plus` vs `armor`.  
Damage: `roll damage dice + add_dam(strength) + dam_plus`.

Both tables ported directly from `fight.c` in Rogue v5.4.2.

---

## Hunger

- `food` counter decrements each turn (halved with SlowDigestion ring).
- Warning message at `food == 20`, fainting damage at `food <= 0` (20% chance per turn).
- `eat()` refills: `food = min(stomach_size, food + hunger_time)`.

From `config.ron`: `hunger_time: 1300`, `stomach_size: 2000`.

---

## Visibility (FOV)

`recompute_visibility()` casts Bresenham rays in all directions up to radius:
- Default radius: 5
- Blind status: radius 1
- Searching ring: radius 7

`reveal_line()` reveals tiles along each ray, stopping after the first opaque tile (walls are revealed but block further sight).

---

## Status effects

| Effect | Mechanic |
|--------|----------|
| Confused | 50% chance of random movement each turn; decrements each turn |
| Blind | FOV radius = 1; decrements each turn |
| Paralyzed | Skip turns; decrements each turn |
| Poisoned | Every 3 turns: -1 STR; decrements each turn |
| Haste | Double speed (simplified); decrements each turn |

---

## Monster AI

Each monster turn:
1. Wake if player is within Chebyshev distance 6 and in FOV
2. If awake and mean: attack if adjacent, else greedy-step toward player
3. `step_toward()` avoids walls and other monsters

Special attacks: `MonsterSpecial` enum — `StealGold`, `DrainLevel`, `DrainStrength`, `Poison`, `Confuse`, `Paralyze`, `Blind`.

---

## Item effects

### Scrolls
`MagicMapping` (reveal level), `Teleport`, `EnchantWeapon` (+1 hit/dam), `EnchantArmor` (-1 AC), `Aggravate` (wake all monsters), `Identify`.

### Potions
`Healing`, `ExtraHealing`, `Poison`, `GainStrength`, `RestoreStrength`, `SeeInvisible`, `Confusion`, `Blindness`, `Hallucination`, `HasteSelf`, `RaiseLevel`, `MonsterDetection`, `MagicDetection`, `Levitation`.

### Rings
`Protection` (+1 armor), `AddStrength` (+1 STR), `Dexterity` (+1 hit), `IncreaseDamage` (+1 dam), `Regeneration` (halve regen period), `SlowDigestion` (halve food consumption), `Searching` (FOV radius 7), `Teleportation` (random teleport ~1/100 turns — bad).

### Wands
`MagicMissile`, `Fire`, `Cold`, `Lightning`, `DrainLife`, `Slow`, `Fear`, `Confusion`, `Polymorph`, `TeleportAway`, `Haste`, `Light` (= magic mapping), `CancellationWand`.

---

## Score system

`Scores` in `rogue-engine/src/scores.rs`. Kept as JSON (`rogue_scores.json`) next to the binary on native. On WASM: `load()` returns empty, `save()` is a no-op. Top 20 entries sorted by depth then gold.

---

## XP and leveling

Doubling thresholds: level 2 at 10 XP, level 3 at 20, level 4 at 40, … (`need *= 2` each level). On level-up: `+1d8 * levels_gained` max HP.
