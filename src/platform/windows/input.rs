//! Windows input automation: SendInput, SendMessage/PostMessage background injection,
//! and window enumeration.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, VIRTUAL_KEY, VK_CONTROL, VK_RETURN, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetClientRect, GetForegroundWindow, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, IsWindowVisible, PostMessageW, SendMessageW,
    SetCursorPos, SetForegroundWindow, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN,
    WM_LBUTTONUP,
};

/// Sends a simulated Enter key press globally.
pub fn send_enter_global() {
    send_key_pair(VK_RETURN);
}

/// Sends a simulated left mouse click at current cursor position.
pub fn send_click_global() {
    let inputs = [
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: windows::Win32::UI::Input::KeyboardAndMouse::MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_LEFTDOWN,
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
                    dwFlags: MOUSEEVENTF_LEFTUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Copies text to clipboard, simulates Ctrl+V paste, and presses Enter.
pub fn send_type_global(text: &str) -> Result<(), String> {
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

    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }

    thread::sleep(Duration::from_millis(350));
    send_enter_global();
    Ok(())
}

fn send_key_pair(vk: VIRTUAL_KEY) {
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

    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
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
    BOOL(1)
}

/// Send Enter to background HWND without stealing focus.
pub fn post_enter_to_hwnd(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(
            hwnd,
            WM_KEYDOWN,
            WPARAM(VK_RETURN.0 as usize),
            LPARAM(0),
        );
        thread::sleep(Duration::from_millis(50));
        let _ = PostMessageW(
            hwnd,
            WM_KEYUP,
            WPARAM(VK_RETURN.0 as usize),
            LPARAM(0),
        );
    }
}

/// Send Left Click to the center of background HWND client area.
pub fn post_click_to_hwnd(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_ok() {
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            let x = (w / 2) as i32;
            let y = (h / 2) as i32;
            let lparam = ((y << 16) | (x & 0xFFFF)) as isize;

            let _ = PostMessageW(hwnd, WM_LBUTTONDOWN, WPARAM(1), LPARAM(lparam));
            thread::sleep(Duration::from_millis(50));
            let _ = PostMessageW(hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(lparam));
        }
    }
}

/// Send text character by character via WM_CHAR, followed by Enter.
pub fn send_text_to_hwnd(hwnd: HWND, text: &str) {
    unsafe {
        for ch in text.chars() {
            let _ = SendMessageW(hwnd, WM_CHAR, WPARAM(ch as usize), LPARAM(0));
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(100));
        post_enter_to_hwnd(hwnd);
    }
}

/// Bring target HWND to foreground, dispatch action, then optionally restore previous foreground window.
pub fn execute_with_foreground(
    hwnd: HWND,
    action: &crate::models::ActionType,
    prompt: &str,
) -> Result<(), String> {
    unsafe {
        let prev_hwnd = GetForegroundWindow();
        let _ = SetForegroundWindow(hwnd);
        thread::sleep(Duration::from_millis(200));

        match action {
            crate::models::ActionType::Enter => {
                send_enter_global();
            }
            crate::models::ActionType::Click => {
                let mut rect = RECT::default();
                if GetWindowRect(hwnd, &mut rect).is_ok() {
                    let cx = rect.left + (rect.right - rect.left) / 2;
                    let cy = rect.top + (rect.bottom - rect.top) / 2;
                    let _ = SetCursorPos(cx, cy);
                    thread::sleep(Duration::from_millis(50));
                    send_click_global();
                } else {
                    send_click_global();
                }
            }
            crate::models::ActionType::Type => {
                let _ = send_type_global(prompt);
            }
            _ => {}
        }

        if !prev_hwnd.is_invalid() && prev_hwnd.0 != std::ptr::null_mut() && prev_hwnd != hwnd {
            thread::sleep(Duration::from_millis(200));
            let _ = SetForegroundWindow(prev_hwnd);
        }
    }

    Ok(())
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
    BOOL(1)
}
