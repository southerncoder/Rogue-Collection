//! Platform-neutral input actions.
//!
//! Each frontend (bracket-lib, macroquad, …) translates its own key type into
//! one of these intent-level actions before calling [`crate::game::Game::tick`].

/// A single player intent, translated from a physical key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameAction {
    // Movement (delta x, delta y)
    Move(i32, i32),
    Wait,
    Descend,
    Pickup,
    Quaff,
    Eat,
    Read,
    Inventory,
    MessageLog,
    Drop,
    Wear,
    Wield,
    PutOnRing,
    RemoveRing,
    Throw,
    Search,
    Explore,
    Help,
    Quit,
    Scores,
    Autopilot,
    Restart,
    /// Confirm a yes/no prompt (also used as Enter / Return in menus).
    Confirm,
    /// Cancel / close overlay.
    Cancel,
    ScrollUp,
    ScrollDown,
    /// A bare letter (used to select items in inventory lists).
    Char(char),
}
