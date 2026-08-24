# AutoClick Timer (Rust Native Edition)

AutoClick Timer is a high-performance Windows desktop automation utility written in pure Rust with Slint and native Win32 APIs. It ships as a **single standalone `.exe`** with a modern GUI, a full headless CLI, and a native Model Context Protocol (MCP) server for AI agents -- no installation required.

## Highlights & Performance

- **Ultra-Lightweight:** Single standalone `.exe` (~10 MB), consuming < 18 MB RAM (75% lower than Python/Tkinter).
- **Instant Launch:** Zero extraction delay, sub-30 ms cold startup.
- **Pure Native Execution:** Powered by Slint with hardware-accelerated rendering (DirectX / software fallback). Zero WebView2 / Chromium dependencies.
- **Zero-Admin RTC Sleep & Wake (`asInvoker`):** Native user-mode Win32 waitable wake timers (`CreateWaitableTimerExW` with `fResume=true`) and suspend (`Powrprof.dll`) operate completely without administrator elevation or UAC prompts.
- **Password-Safe Windows Automation:**
  - Background Win32 `PostMessageW` / `SendMessageW` targeting specific window handles without stealing focus, functioning even when the machine is locked.
  - Optional zero-password wake configuration (`act configure-wake-lock`) allowing the machine to wake directly to the unlocked desktop without password prompts.
- **Native OS Automation:**
  - Direct Win32 `SendInput` and background window message injection.
  - Native Windows Power Management (`SetThreadExecutionState` for Caffeine keep-awake, `Powrprof.dll` for `SetSuspendState`, RTC wake timers).
  - Emergency Mouse Failsafe: instant abort when mouse reaches coordinate (0, 0).
- **Native MCP Server:** Built-in Model Context Protocol (MCP) server over `stdio` (`act mcp`) for direct integration with AI agents (Claude Desktop, Cursor, Antigravity, etc.).
- **Internationalization:** Runtime language toggle between German (DE) and English (EN).
- **Profile Persistence:** Compatible JSON profile save/load format (`.act`).
- **Full CLI & MCP Parity:** Every GUI feature is accessible headlessly from PowerShell/cmd and via MCP tool calls.

## What's New in v1.5.1

- **Embedded TCP MCP Server in GUI:** The desktop GUI application automatically starts the background TCP MCP server on port `7890` at startup. No manual console launcher is required to use the mobile remote.
- **Real-Time Bi-Directional GUI Sync:** Mobile actions (toggling Caffeine, scheduling queues, executing single actions, cancelling) immediately update the desktop UI in real time (flipping toggles, showing queue items, rendering live countdowns, and streaming logs).
- **Desktop-Attached Cursor Resolution:** Fixed cursor coordinate query returning `(0, 0)` from background worker threads by introducing automatic interactive desktop input attachment (`OpenInputDesktop` + `SetThreadDesktop`) and multi-level DPI-aware fallbacks.
- **Live Caffeine State Telemetry:** `act_get_status` now exposes `caffeine_active` boolean state for instant mobile UI sync.

## What's New in v1.5.0

- **Tailscale Mobile Remote App (APK):** Complete Android companion app to control your Windows PC securely over Tailscale.
- **Remote Lockout Safeguards:** 2-step confirmation and safety checks before remote system suspend or shutdown.
- **Visual Action Grid & Queue Builder:** Mobile queue construction with sleep presets and hardware RTC wake timers.

## Action Types

| Action | Description | Requires Admin |
|---|---|---|
| `enter` | Press Enter key after countdown | No |
| `click` | Left mouse click after countdown | No |
| `type` | Type text string after countdown | No |
| `sleep` | Suspend PC via Win32 RTC wake timer | No |
| `shutdown` | System shutdown after countdown | No |
| `caffeine` | Keep screen awake for set duration | No |

---

## Model Context Protocol (MCP) for AI Agents

AutoClick Timer embeds a complete MCP `stdio` server directly into the binary. AI agents can use all desktop automation features through structured JSON tool calls without shell escaping issues.

### Starting the MCP Server (stdio -- for AI agents)

```powershell
act mcp
```

### Starting the MCP Server (TCP -- for mobile remote control over Tailscale)

```powershell
act mcp --tcp-port 7890
act mcp --tcp-port 7890 --api-key mysecret
```

When `--tcp-port` is provided the binary listens on `0.0.0.0:<port>` **in addition** to the stdio transport, accepting multiple concurrent clients. Each client speaks the same MCP JSON-RPC 2.0 protocol over a newline-delimited TCP stream.

**Authentication (optional but recommended):** If `--api-key` is set, every TCP client must send an `auth` message as its very first request:

```json
{"jsonrpc":"2.0","id":1,"method":"auth","params":{"key":"mysecret"}}
```

The server replies `{"result":{"authenticated":true}}` on success, or disconnects on failure. Tailscale's VPN already provides network-layer security -- the API key is an extra guard.

### Tailscale Remote Control (Android / iOS)

The companion **AutoClick Remote** Flutter app (`mobile/`) connects to the TCP MCP server over your Tailscale network and exposes full GUI parity on your phone:

- **Status screen** -- live queue progress, remaining time, cancel button
- **Quick Actions** -- one-tap click, enter, sleep, shutdown, type, caffeine toggle
- **Queue Builder** -- drag-to-reorder multi-step queues with repeat looping
- **Settings** -- cursor position, passwordless wake config, disconnect

**Setup:**
1. Open AutoClick Timer (`autoclicktimer.exe`) on your PC (or run headless via `act mcp --tcp-port 7890 --api-key <secret>`)
2. Make sure both devices are on the same Tailscale network
3. Open AutoClick Remote on your phone, enter your PC's Tailscale IP, port `7890` (and API key if configured)
4. Tap Connect

### MCP Configuration Example (Claude Desktop / Cursor / Antigravity)

```json
{
  "mcpServers": {
    "autoclicktimer": {
      "command": "C:\\path\\to\\autoclicktimer.exe",
      "args": ["mcp"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description | Parameters |
|---|---|---|
| `act_execute_action` | Execute a single action immediately or after countdown | `action`, `after`, `label`, `prompt`, `window`, `foreground`, `pre_sleep_grace`, `post_wake_delay`, `repeat_count`, `start_in`, `start_at`, `async_execution` |
| `act_schedule_queue` | Build and execute a multi-step queue | `steps` (array of step objects), `repeat_count`, `save_profile_path`, `start_in`, `start_at`, `async_execution` |
| `act_run_profile` | Execute a saved `.act` profile headlessly | `profile_path`, `repeat_count`, `start_in`, `start_at`, `async_execution` |
| `act_save_profile` | Validate and save steps to a `.act` profile file | `profile_path`, `steps` |
| `act_reorder_queue` | Reorder steps in a profile or validate move indices | `from_index`, `to_index`, `profile_path` |
| `act_get_status` | Query active queue progress, remaining seconds, iteration count, and phase in real time | (none) |
| `act_get_cursor_pos` | Query current screen coordinates (X, Y) of mouse cursor | (none) |
| `act_get_window_rect` | Query bounding box (X, Y, Width, Height) of a window by title | `window` |
| `act_cancel` | Immediately cancel active timer or queue | (none) |
| `act_list_windows` | Enumerate visible window titles for window-specific targeting | (none) |
| `act_set_caffeine` | Direct toggle of screen/sleep keep-awake mode | `active`, `duration_seconds` |
| `act_configure_passwordless_wake` | Configure user session to wake directly without password lock | (none) |

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
act run --profile my.act --repeat 5        # repeat queue 5 times
act run --profile my.act --repeat 0        # loop infinitely until stopped
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

# Schedule start and repeat
act add click --after 5s --repeat 3
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

# With repeat loop
act queue --step "click:2s" --step "enter:1s" --repeat 5

# Reorder steps in a saved profile
act reorder --profile my.act --from 2 --to 0

# Inspect mouse position and window bounds
act get-cursor
act get-window --window "Notepad"
```

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
