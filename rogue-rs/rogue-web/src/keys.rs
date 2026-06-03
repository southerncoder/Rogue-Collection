//! Keyboard input → [`GameAction`] translation for macroquad.

use macroquad::prelude::*;
use rogue_engine::action::GameAction;

/// Poll macroquad for one key press and convert it to a [`GameAction`], if any.
pub fn poll_action() -> Option<GameAction> {
    // Check modifier state once.
    let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

    if let Some(key) = get_last_key_pressed() {
        return map_key(key, shift);
    }
    None
}

fn map_key(key: KeyCode, shift: bool) -> Option<GameAction> {
    match key {
        // Movement — standard roguelike keys
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
        KeyCode::T => Some(GameAction::Throw),
        KeyCode::Tab => Some(GameAction::Explore),
        KeyCode::I => Some(GameAction::Inventory),
        KeyCode::M => Some(GameAction::MessageLog),
        KeyCode::A => Some(GameAction::Autopilot),
        KeyCode::Slash => Some(GameAction::Help),
        KeyCode::Escape => Some(GameAction::Quit),
        KeyCode::Enter | KeyCode::KpEnter => Some(GameAction::Confirm),
        // Scroll controls (used in message log)
        KeyCode::PageUp   => Some(GameAction::ScrollUp),
        KeyCode::PageDown => Some(GameAction::ScrollDown),
        // Item selection letters (a-z in inventory)
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
