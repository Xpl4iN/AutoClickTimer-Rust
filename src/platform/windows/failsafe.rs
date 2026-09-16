//! Emergency failsafe monitor.
//!
//! Remote desktop clients commonly move the pointer to (0, 0) while
//! connecting. Use an explicit keyboard chord so that reconnecting cannot
//! stop an unattended queue.

use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_F12, VK_SHIFT};

fn key_is_down(key: i32) -> bool {
    unsafe { (GetAsyncKeyState(key) as u16 & 0x8000) != 0 }
}

/// Checks whether the explicit emergency-stop chord is currently held.
pub fn is_failsafe_triggered() -> bool {
    key_is_down(VK_CONTROL.0 as i32)
        && key_is_down(VK_SHIFT.0 as i32)
        && key_is_down(VK_F12.0 as i32)
}
