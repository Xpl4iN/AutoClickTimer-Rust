//! Emergency failsafe monitor (cursor at (0,0)).

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Checks if the mouse cursor is at the emergency abort coordinate (0, 0).
pub fn is_failsafe_triggered() -> bool {
    let mut pt = POINT::default();
    unsafe {
        if GetCursorPos(&mut pt).is_ok() {
            return pt.x == 0 && pt.y == 0;
        }
    }
    false
}
