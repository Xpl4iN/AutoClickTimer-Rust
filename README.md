# AutoClick Timer (Rust Native Edition)

AutoClick Timer is a high-performance Windows desktop automation utility written in pure Rust with Slint and native Win32 APIs. It ships as a **single standalone `.exe`** with a modern GUI, a full headless CLI, and a native Model Context Protocol (MCP) server for AI agents -- no installation required.

## Highlights & Performance

- **Ultra-Lightweight:** Single standalone `.exe` (~10 MB), consuming < 18 MB RAM (75% lower than Python/Tkinter).
- **Instant Launch:** Zero extraction delay, sub-30 ms cold startup.
- **Pure Native Execution:** Powered by Slint with hardware-accelerated rendering (DirectX / software fallback). Zero WebView2 / Chromium dependencies.
- **Zero-Admin RTC Sleep & Wake (`asInvoker`):** Native user-mode Win32 waitable wake timers (`CreateWaitableTimerExW` with `fResume=true`) and suspend (`Powrprof.dll`) operate completely without administrator elevation or UAC prompts.
- **Password-Safe Windows Automation:**
  - Background Win32 `PostMessageW` / `SendMessageW` targeting specific window handles without stealing focus, functioning even when the machine is locked.
  - Sleep & Wake configures and verifies zero-password wake before suspending, then checks that the desktop is unlocked after wake. The queue stops before sending more input if either check fails. The setting can also be applied manually with `act configure-wake-lock`. Windows security policy can still require sign-in.
- **Native OS Automation:**
  - Direct Win32 `SendInput` and background window message injection.
  - Native Windows Power Management (`SetThreadExecutionState` for Caffeine keep-awake, `Powrprof.dll` for `SetSuspendState`, RTC wake timers).
  - Explicit Emergency Stop: instant abort from the Stop button or Ctrl+Shift+F12. Cursor movement to (0, 0) is ignored for RustDesk compatibility.
- **Unattended Safety:** Sleep is cancelled if neither a configured native wake timer nor a scheduled wake task is available. Missing target windows and Windows input API failures mark the queue as failed rather than completing the step. Windows accepting an input event does not prove that the target application acted on it.
- **Native MCP Server:** Built-in Model Context Protocol (MCP) server over `stdio` (`act mcp`) for direct integration with AI agents (Claude Desktop, Cursor, Antigravity, etc.). When the GUI is running, local CLI and stdio MCP calls are proxied to its shared queue and UI.
- **Internationalization:** Runtime language toggle between German (DE) and English (EN).
- **Profile Persistence:** Compatible JSON profile save/load format (`.act`).
- **Full CLI & MCP Parity:** Every GUI feature is accessible headlessly from PowerShell/cmd and via MCP tool calls.

## What's New in v1.6.2

- Pair a phone from the desktop GUI with a generated, saved key and scannable QR code. The bridge listens on the PC's Tailscale address when available.
- Download the Android app from the pairing panel. The phone can scan and save the connection details, then reconnect automatically on launch.

## What's New in v1.6.1

- Prepare passwordless wake before sleep so unattended input can reach the desktop after resume.
- Refuse to suspend when no wake source is armed.
- Fail the queue when a target window is missing or Windows rejects input, instead of silently continuing.

## What's New in v1.6.0

- **RustDesk-safe emergency stop:** Cursor movement to the top-left corner no longer stops a queue. Use the Stop button or Ctrl+Shift+F12.
- **Verified passwordless-wake configuration:** The wake configuration command now reports registry, power-plan, and activation failures instead of silently continuing.
- **Wake behavior clarified:** Windows policy can still require sign-in after wake even when passwordless wake is configured.

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

The GUI starts a loopback-only listener on port `7890` for local tools and, when Tailscale is available at launch, a second listener on the PC's Tailscale IP. Open **Pair phone** in the GUI to see the address, port, saved pairing key, and QR code. The key is generated once and saved under the current Windows user's local app data. `AUTOCLICKTIMER_MCP_API_KEY` remains an optional override. The Tailscale listener requires the key.

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
1. Open AutoClick Timer (`autoclicktimer.exe`) on your PC and select **Pair phone**. The panel also links to the Android APK download. For headless use, run `act mcp --tcp-port 7890 --api-key <secret>`.
2. Make sure both devices are on the same Tailscale network
3. Open AutoClick Remote on your phone and scan the QR code, or enter the displayed Tailscale IP, port `7890`, and pairing key.
4. Tap Connect if entering the details manually. The phone saves the details and reconnects on subsequent launches.

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
| `act_get_remote_mode` | Read saved remote-mode intent, effective power settings, and discrepancies | (none) |
| `act_set_remote_mode` | Enable remote mode or restore its saved settings | `enabled` (boolean) |
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

### `remote-mode` -- Persistent remote-agent power protection

```powershell
act remote-mode status
act remote-mode on
act remote-mode off
```

When enabled, Remote mode captures the active Windows power scheme and changes only automatic sleep, timed hibernate, lid-close action, and display timeout values. Automatic sleep and timed hibernate are disabled on AC and battery, lid close does nothing, and existing nonzero display timeouts are preserved. A scheme that previously had no display timeout receives 300 seconds on AC and 180 seconds on battery. Critical battery actions, wake passwords, screen-lock security, and other power settings are not changed.

The capture is stored at `%LOCALAPPDATA%\AutoClickTimer\remote-mode.json`. Turning Remote mode off restores those captured values on the original scheme. If you selected another power scheme in the meantime, AutoClickTimer does not switch you away from it. Status reports both the saved intent and whether the settings are currently effective, including any external changes or scheme mismatch.

An explicit scheduled sleep action first turns Remote mode off and restores its snapshot. It remains off after wake. Caffeine is a separate, stronger keep-awake request that also keeps the display on; status reports when Caffeine is active so this conflict is visible.

Caffeine status covers the current AutoClickTimer process; another process or app can independently keep the display on. Remote mode uses native Windows power APIs without simulated input or shell-process polling. Its saved configuration survives app exit; use the toggle, CLI, or MCP to turn it off. A failed change retains the snapshot for a later restore attempt.

Existing `act_execute_action` / `act_schedule_queue` sleep requests and MCP registration (`autoclicktimer.exe mcp`) remain compatible. Restart the MCP connection after updating the executable to discover the new tools. Local CLI commands and stdio MCP calls use the embedded GUI when it is running, so their queue state and actions appear in the UI; they fall back to headless execution when it is not.

This mode does not bypass Windows sign-in after a reboot or guarantee hardware wake from sleep. Keep the laptop ventilated when running with the lid closed.

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
