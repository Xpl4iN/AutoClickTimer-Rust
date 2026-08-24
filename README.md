# AutoClick Timer (Rust Native Edition)

AutoClick Timer is a high-performance Windows desktop automation utility written in pure Rust with Slint and native Win32 APIs.

## Highlights & Performance

- **Ultra-Lightweight:** Single standalone `.exe` (~10 MB), consuming < 18 MB RAM (75% lower than Python/Tkinter).
- **Instant Launch:** Zero extraction delay, sub-30 ms cold startup.
- **Pure Native Execution:** Powered by Slint with hardware-accelerated rendering (DirectX / software fallback). Zero WebView2 / Chromium dependencies.
- **On-Demand Elevation (`asInvoker`):** Launches without annoying UAC prompts. Administrator elevation is requested on-demand only when configuring SYSTEM-level RTC wake tasks.
- **UAC Queue Restoration:** When elevation is requested, the pending queue item (including all configured parameters) is automatically serialized and restored in the elevated instance -- no need to re-enter anything after the UAC prompt.
- **Native OS Automation:**
  - Direct Win32 `SendInput` and background `PostMessageW` / `SendMessageW` targeting specific window handles without stealing focus.
  - Native Windows Power Management (`SetThreadExecutionState` for Caffeine keep-awake, `Powrprof.dll` for `SetSuspendState`, RTC wake timers).
  - Emergency Mouse Failsafe: instant abort when mouse reaches coordinate (0, 0).
- **Internationalization:** Runtime language toggle between German (DE) and English (EN).
- **Profile Persistence:** Compatible JSON profile save/load format (`.act`).

## Action Types

| Action | Description | Requires Admin |
|---|---|---|
| Enter | Press Enter key after countdown | No |
| Left Click | Left mouse click after countdown | No |
| Type | Type text string after countdown | No |
| Sleep & Wake | Suspend PC via RTC wake timer | Yes |
| Shut Down | System shutdown after countdown | No |
| Caffeine | Keep screen awake for set duration | No |

## Quick Presets

| Preset | Description |
|---|---|
| Sleep & Wake + Enter | Sleep PC, wake at target time, press Enter |
| Sleep & Wake + Left Click | Sleep PC, wake at target time, left click |
| Timer + Shut Down | Shut down PC after timer |
| Timer + Caffeine (Keep Awake) | Prevent sleep/screen-off for set duration |
| Timer + Enter | Wait timer, then press Enter (no sleep needed) |

## Building from Source

### Prerequisites
- Rust 1.80+ (`cargo`, `rustc`)
- Windows MSVC Build Tools

### Compile Release Binary
```bash
cargo build --release
```
The optimized executable will be generated at `target/release/autoclicktimer.exe`.

### Run Unit Tests
```bash
cargo test --lib
```

## License
MIT License
