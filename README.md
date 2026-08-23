# AutoClick Timer (Rust Native Edition)

AutoClick Timer is a high-performance Windows desktop automation utility written in pure Rust with Slint and native Win32 APIs.

## Highlights & Performance

- **Ultra-Lightweight:** Single standalone `.exe` (~10 MB), consuming < 18 MB RAM (75% lower than Python/Tkinter).
- **Instant Launch:** Zero extraction delay, sub-30 ms cold startup.
- **Pure Native Execution:** Powered by Slint with hardware-accelerated rendering (DirectX / software fallback). Zero WebView2 / Chromium dependencies.
- **On-Demand Elevation (`asInvoker`):** Launches without annoying UAC prompts. Administrator elevation is requested on-demand only when configuring SYSTEM-level RTC wake tasks.
- **Native OS Automation:**
  - Direct Win32 `SendInput` and background `PostMessageW` / `SendMessageW` targeting specific window handles without stealing focus.
  - Native Windows Power Management (`SetThreadExecutionState` for Caffeine keep-awake, `Powrprof.dll` for `SetSuspendState`, RTC wake timers).
  - Emergency Mouse Failsafe: instant abort when mouse reaches coordinate (0, 0).
- **Internationalization:** Runtime language toggle between German (DE) and English (EN).
- **Profile Persistence:** Compatible JSON profile save/load format (`.act`).

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
