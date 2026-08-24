//! Emergency failsafe monitor (cursor at (0,0)).

use crate::platform::windows::input::get_cursor_pos;

/// Checks if the mouse cursor is at the emergency abort coordinate (0, 0).
pub fn is_failsafe_triggered() -> bool {
    let (x, y) = get_cursor_pos();
    x == 0 && y == 0
}
