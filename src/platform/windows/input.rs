//! Windows input automation: SendInput, SendMessage/PostMessage background injection,
//! and window enumeration.

use std::ffi::OsString;
use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE,
    VK_ESCAPE, VK_F1, VK_F10, VK_F11, VK_F12, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8,
    VK_F9, VK_MENU, VK_RETURN, VK_SHIFT, VK_SPACE, VK_TAB, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetClientRect, GetCursorPos, GetForegroundWindow, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, IsWindowVisible, PostMessageW, SendMessageTimeoutW,
    SetCursorPos, SetForegroundWindow, SMTO_ABORTIFHUNG, SMTO_ERRORONEXIT, WM_CHAR, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
};

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenInputDesktop(dwFlags: u32, fInherit: BOOL, dwDesiredAccess: u32) -> windows::Win32::Foundation::HANDLE;
    fn SetThreadDesktop(hDesktop: windows::Win32::Foundation::HANDLE) -> BOOL;
    fn CloseDesktop(hDesktop: windows::Win32::Foundation::HANDLE) -> BOOL;
    fn GetUserObjectInformationW(
        hObj: windows::Win32::Foundation::HANDLE,
        nIndex: i32,
        pvInfo: *mut c_void,
        nLength: u32,
        lpnLengthNeeded: *mut u32,
    ) -> BOOL;
}

/// Return whether the interactive input desktop is the user's unlocked desktop.
/// A lock screen or secure prompt uses another desktop, so global input must wait.
pub fn is_input_desktop_unlocked() -> Result<bool, String> {
    const DESKTOP_READOBJECTS: u32 = 0x0001;
    const UOI_NAME: i32 = 2;

    unsafe {
        let desktop = OpenInputDesktop(0, BOOL(0), DESKTOP_READOBJECTS);
        if desktop.is_invalid() || desktop.0.is_null() {
            return Err(format!(
                "Cannot inspect the input desktop: {}",
                std::io::Error::last_os_error()
            ));
        }

        let mut name = [0u16; 64];
        let mut needed = 0u32;
        let result = GetUserObjectInformationW(
            desktop,
            UOI_NAME,
            name.as_mut_ptr().cast(),
            std::mem::size_of_val(&name) as u32,
            &mut needed,
        );
        let error = if result.0 == 0 {
            Some(std::io::Error::last_os_error())
        } else {
            None
        };
        let _ = CloseDesktop(desktop);

        if let Some(error) = error {
            return Err(format!("Cannot read the input desktop name: {error}"));
        }
        let length = name.iter().position(|&ch| ch == 0).unwrap_or(name.len());
        Ok(String::from_utf16_lossy(&name[..length]).eq_ignore_ascii_case("Default"))
    }
}

fn require_unlocked_input_desktop() -> Result<(), String> {
    if is_input_desktop_unlocked()? {
        Ok(())
    } else {
        Err("Windows is showing a lock or secure screen; global input was not sent".to_string())
    }
}

/// Get current mouse cursor screen coordinates (X, Y).
pub fn get_cursor_pos() -> (i32, i32) {
    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_ok() {
            return (pt.x, pt.y);
        }

        // If direct GetCursorPos failed (e.g. Access Denied when running from worker thread),
        // attach thread to the interactive input desktop:
        const DESKTOP_READOBJECTS: u32 = 0x0001;
        const DESKTOP_WRITEOBJECTS: u32 = 0x0080;
        let desktop = OpenInputDesktop(0, BOOL(0), DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS);
        if !desktop.is_invalid() && desktop.0 != std::ptr::null_mut() {
            let _ = SetThreadDesktop(desktop);
            let _ = CloseDesktop(desktop);
            if GetCursorPos(&mut pt).is_ok() {
                return (pt.x, pt.y);
            }
        }

        // Fallback 1: GetPhysicalCursorPos
        if windows::Win32::UI::WindowsAndMessaging::GetPhysicalCursorPos(&mut pt).is_ok() {
            return (pt.x, pt.y);
        }

        // Fallback 2: GetCursorInfo
        let mut ci = windows::Win32::UI::WindowsAndMessaging::CURSORINFO {
            cbSize: std::mem::size_of::<windows::Win32::UI::WindowsAndMessaging::CURSORINFO>() as u32,
            ..Default::default()
        };
        if windows::Win32::UI::WindowsAndMessaging::GetCursorInfo(&mut ci).is_ok() {
            return (ci.ptScreenPos.x, ci.ptScreenPos.y);
        }

        (pt.x, pt.y)
    }
}

/// Retrieve bounding rectangle (X, Y, Width, Height) of a window matching title.
pub fn get_window_rect_by_title(title: &str) -> Option<(i32, i32, i32, i32)> {
    let hwnd = find_window_by_title(title)?;
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let x = rect.left;
            let y = rect.top;
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            Some((x, y, w, h))
        } else {
            None
        }
    }
}

/// Sends a simulated Enter key press globally.
pub fn send_enter_global() -> Result<(), String> {
    send_key_pair(VK_RETURN)
}

/// Sends a simulated left mouse click at current cursor position.
pub fn send_click_global() -> Result<(), String> {
    send_mouse_event(MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
}

/// Sends a simulated right mouse click at current cursor position.
#[allow(dead_code)]
pub fn send_right_click_global() -> Result<(), String> {
    send_mouse_event(MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)
}

/// Sends a simulated middle mouse click at current cursor position.
#[allow(dead_code)]
pub fn send_middle_click_global() -> Result<(), String> {
    send_mouse_event(MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP)
}

/// Sends a simulated double left mouse click.
#[allow(dead_code)]
pub fn send_double_click_global() -> Result<(), String> {
    send_click_global()?;
    thread::sleep(Duration::from_millis(100));
    send_click_global()
}

/// Move mouse to (x, y) and trigger the specified button click.
#[allow(dead_code)]
pub fn send_click_at(x: i32, y: i32, button: &str) -> Result<(), String> {
    require_unlocked_input_desktop()?;
    unsafe {
        SetCursorPos(x, y).map_err(|error| format!("Could not move cursor to ({x}, {y}): {error}"))?;
    }
    thread::sleep(Duration::from_millis(50));
    match button.to_lowercase().as_str() {
        "right" => send_right_click_global(),
        "middle" => send_middle_click_global(),
        "double" => send_double_click_global(),
        _ => send_click_global(),
    }
}

fn send_mouse_event(down_flag: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS, up_flag: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS) -> Result<(), String> {
    let inputs = [
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: down_flag,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: up_flag,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    send_inputs_checked(&inputs, "Mouse click")
}

/// Copies text to clipboard, simulates Ctrl+V paste, and presses Enter.
pub fn send_type_global(text: &str) -> Result<(), String> {
    require_unlocked_input_desktop()?;
    if let Ok(mut clip) = arboard::Clipboard::new() {
        if let Err(e) = clip.set_text(text) {
            return Err(format!("Clipboard set error: {}", e));
        }
    } else {
        return Err("Failed to open clipboard".to_string());
    }

    thread::sleep(Duration::from_millis(300));

    // Ctrl+V key combination
    let inputs = [
        // Ctrl Down
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // V Down
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_V,
                    wScan: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // V Up
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_V,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // Ctrl Up
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    send_inputs_checked(&inputs, "Paste shortcut")?;

    thread::sleep(Duration::from_millis(350));
    send_enter_global()
}

/// Send a custom key combination (e.g. "ctrl+s", "alt+f4", "escape", "f5", "tab").
#[allow(dead_code)]
pub fn send_key_combination(combo: &str) -> Result<(), String> {
    let parts: Vec<String> = combo.split('+').map(|s| s.trim().to_lowercase()).collect();
    if parts.is_empty() {
        return Err("Empty key combination".to_string());
    }

    let mut down_inputs = Vec::new();
    let mut up_inputs = Vec::new();

    for part in &parts {
        let vk = match part.as_str() {
            "ctrl" | "control" => VK_CONTROL,
            "alt" | "menu" => VK_MENU,
            "shift" => VK_SHIFT,
            "enter" | "return" => VK_RETURN,
            "esc" | "escape" => VK_ESCAPE,
            "tab" => VK_TAB,
            "space" => VK_SPACE,
            "backspace" | "back" => VK_BACK,
            "delete" | "del" => VK_DELETE,
            "f1" => VK_F1, "f2" => VK_F2, "f3" => VK_F3, "f4" => VK_F4,
            "f5" => VK_F5, "f6" => VK_F6, "f7" => VK_F7, "f8" => VK_F8,
            "f9" => VK_F9, "f10" => VK_F10, "f11" => VK_F11, "f12" => VK_F12,
            s if s.len() == 1 => {
                let ch = s.chars().next().unwrap();
                if ch.is_ascii_alphabetic() {
                    VIRTUAL_KEY(ch.to_ascii_uppercase() as u16)
                } else if ch.is_ascii_digit() {
                    VIRTUAL_KEY(ch as u16)
                } else {
                    return Err(format!("Unsupported key in combination: '{}'", part));
                }
            }
            _ => return Err(format!("Unsupported key in combination: '{}'", part)),
        };

        down_inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });

        up_inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }

    up_inputs.reverse();

    send_inputs_checked(&down_inputs, "Key combination press")?;
    thread::sleep(Duration::from_millis(50));
    send_inputs_checked(&up_inputs, "Key combination release")?;

    Ok(())
}

fn send_key_pair(vk: VIRTUAL_KEY) -> Result<(), String> {
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    send_inputs_checked(&inputs, "Key press")
}

fn send_inputs_checked(inputs: &[INPUT], action: &str) -> Result<(), String> {
    require_unlocked_input_desktop()?;
    let inserted = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if inserted as usize == inputs.len() {
        Ok(())
    } else {
        Err(format!(
            "{action}: Windows accepted {inserted} of {} input events",
            inputs.len()
        ))
    }
}

/// Find a window by its exact or partial title.
pub fn find_window_by_title(title: &str) -> Option<HWND> {
    if title.is_empty() {
        return None;
    }

    let mut wide: Vec<u16> = title.encode_utf16().collect();
    wide.push(0);

    let hwnd_res = unsafe {
        FindWindowW(
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR(wide.as_ptr()),
        )
    };

    if let Ok(hwnd) = hwnd_res {
        if !hwnd.is_invalid() && hwnd.0 != std::ptr::null_mut() {
            return Some(hwnd);
        }
    }

    // Fallback: search via EnumWindows
    let mut found = None;
    let target = title.to_lowercase();

    unsafe {
        let _ = EnumWindows(
            Some(enum_find_proc),
            LPARAM(&mut (&target, &mut found) as *mut _ as isize),
        );
    }

    found
}

unsafe extern "system" fn enum_find_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let tuple = &mut *(lparam.0 as *mut (&String, &mut Option<HWND>));
        let (target, out) = tuple;

        if IsWindowVisible(hwnd).as_bool() {
            let len = GetWindowTextLengthW(hwnd);
            if len > 0 {
                let mut buf = vec![0u16; (len + 1) as usize];
                let read = GetWindowTextW(hwnd, &mut buf);
                if read > 0 {
                    let text = OsString::from_wide(&buf[..read as usize])
                        .to_string_lossy()
                        .to_lowercase();
                    if text.contains(target.as_str()) {
                        **out = Some(hwnd);
                        return BOOL(0); // stop enumeration
                    }
                }
            }
        }
    }
    BOOL(1)
}

/// Send Enter to background HWND without stealing focus.
pub fn post_enter_to_hwnd(hwnd: HWND) -> Result<(), String> {
    unsafe {
        PostMessageW(
            hwnd,
            WM_KEYDOWN,
            WPARAM(VK_RETURN.0 as usize),
            LPARAM(0),
        ).map_err(|error| format!("Could not post Enter key down: {error}"))?;
        thread::sleep(Duration::from_millis(50));
        PostMessageW(
            hwnd,
            WM_KEYUP,
            WPARAM(VK_RETURN.0 as usize),
            LPARAM(0),
        ).map_err(|error| format!("Could not post Enter key up: {error}"))?;
    }
    Ok(())
}

/// Send Left Click to the center of background HWND client area.
pub fn post_click_to_hwnd(hwnd: HWND) -> Result<(), String> {
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect)
            .map_err(|error| format!("Could not read target window client area: {error}"))?;
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 {
            return Err("Target window has no clickable client area".to_string());
        }
        let x = w / 2;
        let y = h / 2;
        let lparam = ((y << 16) | (x & 0xFFFF)) as isize;

        PostMessageW(hwnd, WM_LBUTTONDOWN, WPARAM(1), LPARAM(lparam))
            .map_err(|error| format!("Could not post mouse button down: {error}"))?;
        thread::sleep(Duration::from_millis(50));
        PostMessageW(hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(lparam))
            .map_err(|error| format!("Could not post mouse button up: {error}"))?;
    }
    Ok(())
}

/// Send text character by character via WM_CHAR, followed by Enter.
pub fn send_text_to_hwnd(hwnd: HWND, text: &str) -> Result<(), String> {
    unsafe {
        for ch in text.encode_utf16() {
            let accepted = SendMessageTimeoutW(
                hwnd,
                WM_CHAR,
                WPARAM(ch as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
                1_000,
                None,
            );
            if accepted.0 == 0 {
                return Err("Target window rejected or timed out while receiving text".to_string());
            }
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(100));
        post_enter_to_hwnd(hwnd)?;
    }
    Ok(())
}

/// Bring target HWND to foreground, dispatch action, then optionally restore previous foreground window.
pub fn execute_with_foreground(
    hwnd: HWND,
    action: &crate::models::ActionType,
    prompt: &str,
) -> Result<(), String> {
    require_unlocked_input_desktop()?;
    unsafe {
        let prev_hwnd = GetForegroundWindow();
        if !SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() != hwnd {
            return Err("Could not focus the target window".to_string());
        }
        thread::sleep(Duration::from_millis(200));
        if GetForegroundWindow() != hwnd {
            return Err("Target window lost focus before input was sent".to_string());
        }

        let result = (|| -> Result<(), String> { match action {
            crate::models::ActionType::Enter => {
                send_enter_global()
            }
            crate::models::ActionType::Click => {
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect)
                    .map_err(|error| format!("Could not read target window bounds: {error}"))?;
                let cx = rect.left + (rect.right - rect.left) / 2;
                let cy = rect.top + (rect.bottom - rect.top) / 2;
                SetCursorPos(cx, cy)
                    .map_err(|error| format!("Could not move cursor to target window: {error}"))?;
                thread::sleep(Duration::from_millis(50));
                send_click_global()
            }
            crate::models::ActionType::Type => {
                send_type_global(prompt)
            }
            _ => Err("This action cannot be sent to a foreground window".to_string()),
        } })();

        if !prev_hwnd.is_invalid() && prev_hwnd.0 != std::ptr::null_mut() && prev_hwnd != hwnd {
            thread::sleep(Duration::from_millis(200));
            let _ = SetForegroundWindow(prev_hwnd);
        }
        result
    }
}

/// Retrieve titles of all visible open windows.
pub fn get_open_windows() -> Vec<String> {
    let mut titles = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut titles as *mut _ as isize),
        );
    }
    titles
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let titles = &mut *(lparam.0 as *mut Vec<String>);

        if IsWindowVisible(hwnd).as_bool() {
            let len = GetWindowTextLengthW(hwnd);
            if len > 0 {
                let mut buf = vec![0u16; (len + 1) as usize];
                let read = GetWindowTextW(hwnd, &mut buf);
                if read > 0 {
                    let title = OsString::from_wide(&buf[..read as usize])
                        .to_string_lossy()
                        .to_string();
                    if !title.is_empty() && !titles.contains(&title) {
                        titles.push(title);
                    }
                }
            }
        }
    }
    BOOL(1)
}
