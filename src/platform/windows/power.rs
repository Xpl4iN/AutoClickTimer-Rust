//! Windows power management, RTC wake scheduling, Caffeine keep-awake,
//! and on-demand administrator elevation.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOLEAN, HANDLE};
use windows::Win32::System::Power::{
    SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};
use windows::Win32::System::Threading::{
    CreateWaitableTimerExW, SetWaitableTimer, CREATE_WAITABLE_TIMER_MANUAL_RESET,
    TIMER_ALL_ACCESS,
};
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const TASK_NAME: &str = "SleepWakeTask";
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(2);
const POST_SUSPEND_WAIT: Duration = Duration::from_secs(5);
const MIN_CONFIRM_BUFFER_SECS: u64 = 30;

#[link(name = "Powrprof")]
unsafe extern "system" {
    fn SetSuspendState(bHibernate: BOOLEAN, bForce: BOOLEAN, bWakeupEventsDisabled: BOOLEAN) -> BOOLEAN;
}

/// Check if current process has Administrator privileges.
pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

/// Creates and configures a native Win32 RTC waitable timer configured to wake the PC.
/// Does not require Administrator privileges.
pub fn create_and_set_wake_timer(total_seconds: u64) -> Result<HANDLE, String> {
    unsafe {
        let handle = CreateWaitableTimerExW(
            None,
            windows::core::PCWSTR::null(),
            CREATE_WAITABLE_TIMER_MANUAL_RESET,
            TIMER_ALL_ACCESS.0,
        ).map_err(|e| format!("CreateWaitableTimer failed: {}", e))?;

        // 100-nanosecond intervals; negative indicates relative time from now
        let due_time: i64 = -((total_seconds as i64) * 10_000_000);

        SetWaitableTimer(
            handle,
            &due_time,
            0,
            None,
            None,
            true, // fResume = true: wakes system from suspend
        ).map_err(|e| {
            let _ = CloseHandle(handle);
            format!("SetWaitableTimer failed: {}", e)
        })?;

        Ok(handle)
    }
}

/// Relaunch current executable with UAC Administrator elevation.
/// Optionally forwards a base64-encoded JSON payload via `--pending-item`
/// so the elevated instance can restore the item into its queue on startup.
pub fn request_elevation_with_pending(pending_b64: Option<&str>) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_wide: Vec<u16> = current_exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let runas_wide: Vec<u16> = OsStr::new("runas").encode_wide().chain(Some(0)).collect();

    // Forward existing args (minus any old --pending-item), then append new payload.
    let existing: Vec<String> = std::env::args().skip(1).collect();
    let mut forwarded: Vec<String> = Vec::new();
    let mut skip_next = false;
    for arg in existing {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--pending-item" {
            skip_next = true;
            continue;
        }
        forwarded.push(arg);
    }

    if let Some(payload) = pending_b64 {
        forwarded.push("--pending-item".to_string());
        forwarded.push(payload.to_string());
    }

    let args_str = forwarded.join(" ");
    let args_wide: Vec<u16> = OsStr::new(&args_str).encode_wide().chain(Some(0)).collect();

    unsafe {
        let instance = ShellExecuteW(
            None,
            PCWSTR(runas_wide.as_ptr()),
            PCWSTR(exe_wide.as_ptr()),
            PCWSTR(args_wide.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );

        if instance.0 as isize > 32 {
            std::process::exit(0);
        } else {
            Err("UAC elevation was cancelled or failed".to_string())
        }
    }
}

/// Convenience wrapper -- elevate without a pending payload.
#[allow(dead_code)]
pub fn request_elevation() -> Result<(), String> {
    request_elevation_with_pending(None)
}

use std::sync::atomic::{AtomicBool, Ordering};

static CAFFEINE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Enable or disable native Windows Caffeine keep-awake.
pub fn set_caffeine(active: bool) {
    CAFFEINE_ACTIVE.store(active, Ordering::SeqCst);
    unsafe {
        if active {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
        } else {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

/// Check if Caffeine mode is currently active.
pub fn is_caffeine_active() -> bool {
    CAFFEINE_ACTIVE.load(Ordering::SeqCst)
}

/// Execute sleep sequence with retries, wake scheduling, and suspend detection.
pub fn execute_sleep_with_retry<F>(total_seconds: u64, mut log_fn: F) -> bool
where
    F: FnMut(&str),
{
    for attempt in 1..=MAX_RETRIES {
        log_fn(&format!("  -> Sleep attempt {}/{}...", attempt, MAX_RETRIES));

        let _ = configure_power_wake_timers();

        // 1. Set native Win32 RTC waitable wake timer (works without admin privileges)
        let timer_handle = match create_and_set_wake_timer(total_seconds) {
            Ok(h) => {
                log_fn("  -> Native Win32 RTC wake timer programmed.");
                Some(h)
            }
            Err(e) => {
                log_fn(&format!("  WARNING Native wake timer error: {}", e));
                None
            }
        };

        // 2. If elevated, also register scheduled task as backup
        if is_admin() {
            let wake_at = Local::now() + chrono::Duration::seconds(total_seconds as i64);
            if let Err(e) = schedule_wake_task(wake_at) {
                log_fn(&format!("  WARNING Task schedule failed: {}", e));
            }
        }

        let slept = suspend_and_detect(total_seconds);

        if let Some(h) = timer_handle {
            unsafe {
                let _ = CloseHandle(h);
            }
        }

        if slept {
            log_fn("  -> PC woke successfully.");
            return true;
        } else {
            log_fn(&format!(
                "  WARNING Attempt {}/{} failed: Suspend did not occur",
                attempt, MAX_RETRIES
            ));
            if attempt < MAX_RETRIES {
                thread::sleep(RETRY_DELAY);
            }
        }
    }
    false
}

fn configure_power_wake_timers() -> Result<(), String> {
    let settings = [
        ("SETACVALUEINDEX", "RTCWAKE", "1"),
        ("SETDCVALUEINDEX", "RTCWAKE", "1"),
        ("SETACVALUEINDEX", "7bc4a2f9-d8fc-4469-b07b-33eb785aaca0", "0"),
        ("SETDCVALUEINDEX", "7bc4a2f9-d8fc-4469-b07b-33eb785aaca0", "0"),
    ];

    for (verb, setting, val) in settings {
        let _ = Command::new("powercfg")
            .args([format!("/{}", verb), "SCHEME_CURRENT".into(), "SUB_SLEEP".into(), setting.into(), val.into()])
            .output();
    }

    let _ = Command::new("powercfg")
        .args(["/SETACTIVE", "SCHEME_CURRENT"])
        .output();

    Ok(())
}

fn schedule_wake_task(wake_at: DateTime<Local>) -> Result<(), String> {
    // Delete existing task
    let _ = Command::new("schtasks")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .output();

    let wake_str = wake_at.format("%Y-%m-%dT%H:%M:%S").to_string();

    // Session-reconnect PowerShell script payload
    let ps_payload = format!(
        "$sid = (Get-Process explorer -ErrorAction SilentlyContinue | Select-Object -First 1).SessionId; \
         if ($sid -ne $null) {{ tscon $sid /dest:console }}; \
         Unregister-ScheduledTask -TaskName '{}' -Confirm:$false",
        TASK_NAME
    );

    let encoded_payload = encode_ps(&ps_payload);

    let register_ps = format!(
        "$a = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument '-NoProfile -WindowStyle Hidden -EncodedCommand {}'; \
         $t = New-ScheduledTaskTrigger -Once -At '{}'; \
         $s = New-ScheduledTaskSettingsSet -WakeToRun -AllowStartIfOnBatteries; \
         Register-ScheduledTask -TaskName '{}' -Action $a -Trigger $t -Settings $s -User 'NT AUTHORITY\\SYSTEM' -Force",
        encoded_payload, wake_str, TASK_NAME
    );

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-EncodedCommand",
            &encode_ps(&register_ps),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Register wake task error: {}", err));
    }

    Ok(())
}

fn suspend_and_detect(_expected_sleep_secs: u64) -> bool {
    let t_start = Instant::now();

    // Trigger suspend directly via Powrprof.dll (no external PowerShell process needed)
    unsafe {
        let _ = SetSuspendState(BOOLEAN(0), BOOLEAN(0), BOOLEAN(0));
    }

    // Thread sleep continues across system sleep
    thread::sleep(POST_SUSPEND_WAIT);

    let elapsed = t_start.elapsed().as_secs();
    elapsed >= MIN_CONFIRM_BUFFER_SECS
}

fn encode_ps(script: &str) -> String {
    let utf16: Vec<u8> = script
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();

    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in utf16.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            out.push(CHARSET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(CHARSET[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Trigger system shutdown.
pub fn shutdown_pc() {
    let _ = Command::new("shutdown").args(["/s", "/t", "0"]).spawn();
}

/// Configures user-level settings to allow waking directly without requiring a password prompt.
/// Operates in standard user space (HKCU / powercfg) without requiring administrator rights.
pub fn configure_passwordless_wake() -> Result<String, String> {
    // 1. Disable screensaver password lock
    let _ = Command::new("reg")
        .args([
            "add",
            "HKCU\\Control Panel\\Desktop",
            "/v",
            "ScreenSaverIsSecure",
            "/t",
            "REG_SZ",
            "/d",
            "0",
            "/f",
        ])
        .output();

    // 2. Disable console lock on resume in current power scheme
    let _ = Command::new("powercfg")
        .args(["/SETACVALUEINDEX", "SCHEME_CURRENT", "SUB_NONE", "CONSOLELOCK", "0"])
        .output();
    let _ = Command::new("powercfg")
        .args(["/SETDCVALUEINDEX", "SCHEME_CURRENT", "SUB_NONE", "CONSOLELOCK", "0"])
        .output();
    let _ = Command::new("powercfg")
        .args(["/SETACTIVE", "SCHEME_CURRENT"])
        .output();

    Ok("Passwordless wake configured for current user session.".to_string())
}
