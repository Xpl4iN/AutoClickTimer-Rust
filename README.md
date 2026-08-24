# AutoClick Timer (Rust Native Edition)

AutoClick Timer is a high-performance Windows desktop automation utility written in pure Rust with Slint and native Win32 APIs. It ships as a **single standalone `.exe`** with both a GUI and a full headless CLI -- no installation required.

## Highlights & Performance

- **Ultra-Lightweight:** Single standalone `.exe` (~10 MB), consuming < 18 MB RAM (75% lower than Python/Tkinter).
- **Instant Launch:** Zero extraction delay, sub-30 ms cold startup.
- **Pure Native Execution:** Powered by Slint with hardware-accelerated rendering (DirectX / software fallback). Zero WebView2 / Chromium dependencies.
- **On-Demand Elevation (`asInvoker`):** Launches without UAC prompts. Administrator elevation is requested only when configuring SYSTEM-level RTC wake tasks.
- **UAC Queue Restoration:** When elevation is requested, the pending queue item (including all configured parameters) is automatically serialized and restored in the elevated instance.
- **Native OS Automation:**
  - Direct Win32 `SendInput` and background `PostMessageW` / `SendMessageW` targeting specific window handles without stealing focus.
  - Native Windows Power Management (`SetThreadExecutionState` for Caffeine keep-awake, `Powrprof.dll` for `SetSuspendState`, RTC wake timers).
  - Emergency Mouse Failsafe: instant abort when mouse reaches coordinate (0, 0).
- **Internationalization:** Runtime language toggle between German (DE) and English (EN).
- **Profile Persistence:** Compatible JSON profile save/load format (`.act`).
- **Full CLI:** Every GUI feature is accessible headlessly from PowerShell or cmd.

## What's New in v1.3.2

- **Headless CLI mode** -- all GUI actions accessible without opening a window.
- `queue` subcommand -- build multi-step queues directly from the shell.
- `caffeine` subcommand -- standalone keep-awake without opening the GUI.
- `list-windows` -- enumerate open window titles for use with `--window`.
- `check-update --apply` -- check for and install updates from the CLI.
- `--in <delay>` and `--start-at <HH:MM:SS>` on all execution subcommands.
- `--foreground` flag on `add` to bring target window to front before acting.

## Action Types

| Action | Description | Requires Admin |
|---|---|---|
| `enter` | Press Enter key after countdown | No |
| `click` | Left mouse click after countdown | No |
| `type` | Type text string after countdown | No |
| `sleep` | Suspend PC via RTC wake timer | **Yes** |
| `shutdown` | System shutdown after countdown | No |
| `caffeine` | Keep screen awake for set duration | No |

---

## CLI Usage

The same `autoclicktimer.exe` binary serves as a full CLI. No separate executable needed.

```powershell
act --help
act <subcommand> --help
```

### `run` -- Execute a saved profile headlessly

```powershell
act run --profile my.act
act run --profile my.act --in 30m          # start in 30 minutes
act run --profile my.act --start-at 23:00:00
```

### `add` -- Run a single action

Duration accepts: `5s`, `1m30s`, `2h`, bare seconds, or clock time `HH:MM:SS`.

```powershell
act add click    --after 5s
act add enter    --after 1m30s
act add shutdown --after 2h
act add type     --after 10s --prompt "hello world"
act add type     --after 10s --prompt "hello" --window "Notepad" --foreground
act add sleep    --after 2h  --grace 10 --post-wake 30
act add caffeine --after 1h

# Schedule start
act add click --after 5s --in 30m
act add click --after 5s --start-at 22:30:00
```

### `queue` -- Build a multi-step queue from the shell

Step format: `action:after[,key=value,...]`

Available keys: `label=` `prompt=` `window=` `grace=` `post-wake=` `foreground`

> `prompt=` is greedy -- it captures the rest of the step string, so keep it last.

```powershell
# Run a chain of steps immediately
act queue `
  --step "sleep:2h,grace=10,post-wake=30" `
  --step "click:5s"

# With text input targeting a specific window
act queue `
  --step "type:10s,prompt=hello world,window=Notepad" `
  --step "enter:2s"

# Save to a profile without running
act queue --step "sleep:2h" --step "click:5s" --save night.act

# Save and schedule
act queue --step "sleep:2h" --step "click:5s" --save night.act --in 30m
```

### `caffeine` -- Keep screen on for a duration

```powershell
act caffeine --for 2h
act caffeine --for 90m
```

### `list-windows` -- Show open window titles

Useful for targeting with `--window`.

```powershell
act list-windows
```

### `check-update` -- Update from the CLI

```powershell
act check-update           # check only
act check-update --apply   # download and install
```

### `version`

```powershell
act version
```

---

## GUI Quick Presets

| Preset | Description |
|---|---|
| Sleep & Wake + Enter | Sleep PC, wake at target time, press Enter |
| Sleep & Wake + Left Click | Sleep PC, wake at target time, left click |
| Timer + Shut Down | Shut down PC after timer |
| Timer + Caffeine (Keep Awake) | Prevent sleep/screen-off for set duration |
| Timer + Enter | Wait timer, then press Enter (no sleep needed) |

---

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

## Exit Codes (CLI)

| Code | Meaning |
|---|---|
| `0` | All actions completed successfully |
| `1` | Queue was stopped / update check found nothing |
| `2` | Failsafe triggered (mouse at 0,0) |

## License
MIT License
