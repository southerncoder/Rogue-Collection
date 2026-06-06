//! Keyboard input → [`GameAction`] translation for macroquad.
//!
//! Movement keys support hold-to-repeat, matching terminal behaviour:
//!   • first press fires immediately  
//!   • after 200 ms the key starts repeating at 60 ms intervals  
//! All other keys fire once per press (no OS-repeat bleed-through).

use macroquad::prelude::*;
use rogue_engine::action::GameAction;

/// Delay before auto-repeat starts (seconds).
const INITIAL_DELAY: f64 = 0.20;
/// Time between repeated actions while key is held (seconds).
const REPEAT_INTERVAL: f64 = 0.06;

/// Keys that auto-repeat while held.
const REPEATABLE: &[KeyCode] = &[
    KeyCode::Left,  KeyCode::Right, KeyCode::Up,   KeyCode::Down,
    KeyCode::H,     KeyCode::J,     KeyCode::K,    KeyCode::L,
    KeyCode::Y,     KeyCode::U,     KeyCode::B,    KeyCode::N,
    KeyCode::Kp4,   KeyCode::Kp6,   KeyCode::Kp8,  KeyCode::Kp2,
    KeyCode::Kp7,   KeyCode::Kp9,   KeyCode::Kp1,  KeyCode::Kp3,
    KeyCode::Period, KeyCode::Tab,
];

/// Persistent per-frame input state.
pub struct InputState {
    held:          Option<KeyCode>,
    held_since:    f64,
    last_fired_at: f64,
}

impl InputState {
    pub fn new() -> Self {
        Self { held: None, held_since: 0.0, last_fired_at: 0.0 }
    }

    /// Call once per frame; returns at most one action.
    pub fn poll(&mut self) -> Option<GameAction> {
        let now   = get_time();
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        // ── Initial press of a repeatable key ──────────────────────────────
        // is_key_pressed() fires exactly once per physical key-down, even
        // when the OS is sending repeat events — no bleed-through.
        for &key in REPEATABLE {
            if is_key_pressed(key) {
                self.held          = Some(key);
                self.held_since    = now;
                self.last_fired_at = now;
                return map_key(key, shift);
            }
        }

        // ── Auto-repeat while held ─────────────────────────────────────────
        if let Some(key) = self.held {
            if is_key_down(key) {
                // Don't auto-repeat shifted variants (e.g. Shift+. = descend).
                if !shift {
                    let elapsed    = now - self.held_since;
                    let since_last = now - self.last_fired_at;
                    if elapsed > INITIAL_DELAY && since_last > REPEAT_INTERVAL {
                        self.last_fired_at = now;
                        return map_key(key, false);
                    }
                }
            } else {
                self.held = None;
            }
        }

        // ── One-shot keys (menus, actions, letters) ────────────────────────
        if let Some(key) = get_last_key_pressed() {
            if !REPEATABLE.contains(&key) {
                return map_key(key, shift);
            }
            // Repeatable keys are handled above; skip any OS repeat events.
        }

        None
    }
}

fn map_key(key: KeyCode, shift: bool) -> Option<GameAction> {
    match key {
        // Movement
        KeyCode::Left  | KeyCode::H | KeyCode::Kp4 => Some(GameAction::Move(-1,  0)),
        KeyCode::Right | KeyCode::L | KeyCode::Kp6 => Some(GameAction::Move( 1,  0)),
        KeyCode::Up    | KeyCode::K | KeyCode::Kp8 => Some(GameAction::Move( 0, -1)),
        KeyCode::Down  | KeyCode::J | KeyCode::Kp2 => Some(GameAction::Move( 0,  1)),
        KeyCode::Y | KeyCode::Kp7 => Some(GameAction::Move(-1, -1)),
        KeyCode::U | KeyCode::Kp9 => Some(GameAction::Move( 1, -1)),
        KeyCode::B | KeyCode::Kp1 => Some(GameAction::Move(-1,  1)),
        KeyCode::N | KeyCode::Kp3 => Some(GameAction::Move( 1,  1)),
        // Actions
        KeyCode::Period => {
            if shift { Some(GameAction::Descend) } else { Some(GameAction::Wait) }
        }
        KeyCode::G => Some(GameAction::Pickup),
        KeyCode::Q => Some(GameAction::Quaff),
        KeyCode::E => Some(GameAction::Eat),
        KeyCode::R => {
            if shift { Some(GameAction::RemoveRing) } else { Some(GameAction::Read) }
        }
        KeyCode::P => Some(GameAction::PutOnRing),
        KeyCode::S => {
            if shift { Some(GameAction::Scores) } else { Some(GameAction::Search) }
        }
        KeyCode::T  => Some(GameAction::Throw),
        KeyCode::Tab => Some(GameAction::Explore),
        KeyCode::I  => Some(GameAction::Inventory),
        KeyCode::M  => Some(GameAction::MessageLog),
        KeyCode::A  => Some(GameAction::Autopilot),
        KeyCode::Slash  => Some(GameAction::Help),
        KeyCode::Escape => Some(GameAction::Quit),
        KeyCode::Enter | KeyCode::KpEnter => Some(GameAction::Confirm),
        KeyCode::PageUp   => Some(GameAction::ScrollUp),
        KeyCode::PageDown => Some(GameAction::ScrollDown),
        c if is_letter(c) && !shift => Some(GameAction::Char(key_to_char(c))),
        _ => None,
    }
}

fn is_letter(key: KeyCode) -> bool {
    matches!(key,
        KeyCode::A | KeyCode::B | KeyCode::C | KeyCode::D | KeyCode::E |
        KeyCode::F | KeyCode::G | KeyCode::H | KeyCode::I | KeyCode::J |
        KeyCode::K | KeyCode::L | KeyCode::M | KeyCode::N | KeyCode::O |
        KeyCode::P | KeyCode::Q | KeyCode::R | KeyCode::S | KeyCode::T |
        KeyCode::U | KeyCode::V | KeyCode::W | KeyCode::X | KeyCode::Y |
        KeyCode::Z
    )
}

fn key_to_char(key: KeyCode) -> char {
    match key {
        KeyCode::A => 'a', KeyCode::B => 'b', KeyCode::C => 'c',
        KeyCode::D => 'd', KeyCode::E => 'e', KeyCode::F => 'f',
        KeyCode::G => 'g', KeyCode::H => 'h', KeyCode::I => 'i',
        KeyCode::J => 'j', KeyCode::K => 'k', KeyCode::L => 'l',
        KeyCode::M => 'm', KeyCode::N => 'n', KeyCode::O => 'o',
        KeyCode::P => 'p', KeyCode::Q => 'q', KeyCode::R => 'r',
        KeyCode::S => 's', KeyCode::T => 't', KeyCode::U => 'u',
        KeyCode::V => 'v', KeyCode::W => 'w', KeyCode::X => 'x',
        KeyCode::Y => 'y', KeyCode::Z => 'z',
        _ => '?',
    }
}
