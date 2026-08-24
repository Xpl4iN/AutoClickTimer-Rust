//! Headless CLI for AutoClickTimer -- full GUI-parity mode.
//!
//! Entry point: `cli::run_cli()` -- called from `main()` when a known
//! subcommand is detected.  Never touches Slint.
//!
//! Subcommands
//! -----------
//!   act run   --profile <file.act> [--in <delay>] [--start-at <HH:MM:SS>]
//!   act add   <action> --after <time> [opts...]
//!   act queue --step <action:after[,k=v,...]> ... [--save <out.act>] [--in <delay>]
//!   act caffeine --for <duration>
//!   act list-windows
//!   act check-update [--apply]
//!   act version

use std::io::Write;
use std::path::PathBuf;
use std::process;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};

use crate::executor::{ExecutorEvent, QueueExecutor};
use crate::models::{ActionType, Item, SleepConfig};
use crate::platform::windows::input::get_open_windows;
use crate::platform::windows::power::{configure_passwordless_wake, set_caffeine};
use crate::updater::{check_for_update, download_and_apply, CURRENT_VERSION, REPO};

// ---------------------------------------------------------------------------
// Console attachment (Windows subsystem binary)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn attach_console() {
    use windows::core::w;
    use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Console::{
        AllocConsole, AttachConsole, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    unsafe {
        let attached = AttachConsole(ATTACH_PARENT_PROCESS).is_ok() || AllocConsole().is_ok();
        if attached {
            if let Ok(conout) = CreateFileW(
                w!("CONOUT$"),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            ) {
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, conout);
                let _ = SetStdHandle(STD_ERROR_HANDLE, conout);
            }
            if let Ok(conin) = CreateFileW(
                w!("CONIN$"),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            ) {
                let _ = SetStdHandle(STD_INPUT_HANDLE, conin);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn attach_console() {}

// ---------------------------------------------------------------------------
// clap CLI definition
// ---------------------------------------------------------------------------

/// AutoClickTimer -- headless CLI
#[derive(Parser)]
#[command(
    name = "act",
    version = CURRENT_VERSION,
    about = "AutoClickTimer -- headless CLI and Model Context Protocol (MCP) server.",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start Model Context Protocol (MCP) server over stdio for AI agents
    Mcp,

    /// Configure current user session to wake from sleep without password lockscreen
    #[command(name = "configure-wake-lock")]
    ConfigureWakeLock,

    /// Execute a saved .act profile headlessly
    Run {
        /// Path to an .act profile (JSON queue)
        #[arg(short, long, value_name = "FILE")]
        profile: PathBuf,

        /// Number of times to loop the queue (default: 1, 0 = infinite loop)
        #[arg(short = 'r', long, default_value = "1")]
        repeat: u32,

        /// Delay before starting (e.g. 30m, 1h). Alternative to --start-at.
        #[arg(long, value_name = "DURATION", conflicts_with = "start_at")]
        r#in: Option<String>,

        /// Absolute start time (HH:MM:SS). Waits until then before running.
        #[arg(long, value_name = "HH:MM:SS", conflicts_with = "in")]
        start_at: Option<String>,
    },

    /// Build and immediately run a single action
    Add {
        /// Action: enter | click | type | sleep | shutdown | caffeine
        action: CliAction,

        /// When to fire: duration (5s, 1m30s, 2h) or clock time (HH:MM:SS)
        #[arg(short = 't', long, value_name = "DURATION_OR_TIME")]
        after: String,

        /// Human-readable label
        #[arg(short, long)]
        label: Option<String>,

        /// Text to type (type action only)
        #[arg(short, long)]
        prompt: Option<String>,

        /// Target window title substring (empty = global/active)
        #[arg(short, long, value_name = "TITLE")]
        window: Option<String>,

        /// Bring target window to foreground before acting
        #[arg(long)]
        foreground: bool,

        /// Pre-sleep grace period in seconds (sleep action only)
        #[arg(long, default_value = "5")]
        grace: u64,

        /// Post-wake delay in seconds (sleep action only)
        #[arg(long, default_value = "30")]
        post_wake: u64,

        /// Number of times to repeat the action (default: 1, 0 = infinite loop)
        #[arg(short = 'r', long, default_value = "1")]
        repeat: u32,

        /// Delay before starting (e.g. 30m, 1h). Alternative to --start-at.
        #[arg(long, value_name = "DURATION", conflicts_with = "start_at")]
        r#in: Option<String>,

        /// Absolute start time (HH:MM:SS). Waits until then before running.
        #[arg(long, value_name = "HH:MM:SS", conflicts_with = "in")]
        start_at: Option<String>,
    },

    /// Build a multi-step queue from the shell without a profile file.
    ///
    /// Each --step uses the format:  action:after[,key=value,...]
    ///
    /// Keys: label=<text>  prompt=<text>  window=<title>
    ///       grace=<secs>  post-wake=<secs>  foreground
    ///
    /// Examples:
    ///   --step "click:5s"
    ///   --step "sleep:2h,grace=10,post-wake=30"
    ///   --step "type:10s,prompt=hello world,window=Notepad"
    ///   --step "shutdown:0s"
    Queue {
        /// One or more steps (repeatable)
        #[arg(long = "step", value_name = "ACTION:AFTER[,opts]", required = true)]
        steps: Vec<String>,

        /// Number of times to loop the queue (default: 1, 0 = infinite loop)
        #[arg(short = 'r', long, default_value = "1")]
        repeat: u32,

        /// Save the built queue to an .act profile (without --in/--start-at, saves only)
        #[arg(long, value_name = "FILE")]
        save: Option<PathBuf>,

        /// Delay before starting (e.g. 30m, 1h). Alternative to --start-at.
        #[arg(long, value_name = "DURATION", conflicts_with = "start_at")]
        r#in: Option<String>,

        /// Absolute start time (HH:MM:SS). Waits until then before running.
        #[arg(long, value_name = "HH:MM:SS", conflicts_with = "in")]
        start_at: Option<String>,
    },

    /// Reorder steps in a saved .act profile file
    Reorder {
        /// Path to .act profile
        #[arg(short, long, value_name = "FILE")]
        profile: PathBuf,

        /// Source index of item to move (0-indexed)
        #[arg(short, long)]
        from: usize,

        /// Destination index (0-indexed)
        #[arg(short, long)]
        to: usize,
    },

    /// Get current screen coordinates (X, Y) of the mouse cursor
    #[command(name = "get-cursor")]
    GetCursor,

    /// Get bounding rectangle (X, Y, Width, Height) of a visible window
    #[command(name = "get-window")]
    GetWindow {
        /// Target window title substring
        #[arg(short, long, value_name = "TITLE")]
        window: String,
    },

    /// Enable keep-awake (caffeine) for a set duration, then disable
    Caffeine {
        /// How long to keep the screen on (e.g. 2h, 90m, 3600s)
        #[arg(long, value_name = "DURATION")]
        r#for: String,
    },

    /// List titles of all currently visible windows
    #[command(name = "list-windows")]
    ListWindows,

    /// Check GitHub for a newer release
    CheckUpdate {
        /// Download and apply the update automatically if one is found
        #[arg(long)]
        apply: bool,
    },

    /// Print version string
    Version,
}

#[derive(Clone, ValueEnum)]
enum CliAction {
    Enter,
    Click,
    Type,
    Sleep,
    Shutdown,
    Caffeine,
}

impl From<CliAction> for ActionType {
    fn from(a: CliAction) -> Self {
        match a {
            CliAction::Enter    => ActionType::Enter,
            CliAction::Click    => ActionType::Click,
            CliAction::Type     => ActionType::Type,
            CliAction::Sleep    => ActionType::Sleep,
            CliAction::Shutdown => ActionType::Shutdown,
            CliAction::Caffeine => ActionType::Caffeine,
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run_cli() -> ! {
    let is_mcp = std::env::args().any(|a| a == "mcp");
    if !is_mcp {
        attach_console();
    }

    let cli = Cli::parse();

    match cli.command {
        // ------------------------------------------------------------------
        Commands::Mcp => {
            crate::mcp::run_mcp_server();
        }

        // ------------------------------------------------------------------
        Commands::ConfigureWakeLock => {
            match configure_passwordless_wake() {
                Ok(msg) => {
                    println!("{}", msg);
                    println!("Windows is configured to resume directly to user session without lock screen password.");
                    process::exit(0);
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    process::exit(1);
                }
            }
        }

        // ------------------------------------------------------------------
        Commands::Version => {
            println!("AutoClickTimer v{}", CURRENT_VERSION);
            process::exit(0);
        }

        // ------------------------------------------------------------------
        Commands::ListWindows => {
            let windows = get_open_windows();
            if windows.is_empty() {
                println!("No visible windows found.");
            } else {
                println!("{} visible window(s):\n", windows.len());
                for (i, title) in windows.iter().enumerate() {
                    println!("  {:>3}.  {}", i + 1, title);
                }
            }
            process::exit(0);
        }

        // ------------------------------------------------------------------
        Commands::CheckUpdate { apply } => {
            println!("AutoClickTimer v{} -- checking for updates...", CURRENT_VERSION);
            match check_for_update(REPO, CURRENT_VERSION) {
                None => {
                    println!("Already up to date.");
                    process::exit(0);
                }
                Some(info) => {
                    println!("New version available: {} (current: {})", info.tag, CURRENT_VERSION);
                    if apply {
                        println!("Downloading and applying update...");
                        let result = download_and_apply(&info, |msg| println!("  {}", msg));
                        if let Err(e) = result {
                            eprintln!("error: {}", e);
                            process::exit(1);
                        }
                        // download_and_apply exits on success
                    } else {
                        println!("Run with --apply to install automatically.");
                    }
                    process::exit(0);
                }
            }
        }

        // ------------------------------------------------------------------
        Commands::Caffeine { r#for: dur_str } => {
            let secs = parse_duration_str(&dur_str).unwrap_or_else(|| {
                eprintln!("error: invalid --for value '{}'. Use 2h / 90m / 3600s.", dur_str);
                process::exit(1);
            });
            if secs == 0 {
                eprintln!("error: --for duration must be > 0.");
                process::exit(1);
            }
            println!("Caffeine active for {}. Press Ctrl+C to cancel early.", fmt_duration(secs));
            set_caffeine(true);
            for remaining in (0..secs).rev() {
                print!("\r  Remaining: {}   ", fmt_duration(remaining));
                let _ = std::io::stdout().flush();
                std::thread::sleep(Duration::from_secs(1));
            }
            set_caffeine(false);
            println!("\nCaffeine ended.");
            process::exit(0);
        }

        // ------------------------------------------------------------------
        Commands::GetCursor => {
            let (x, y) = crate::platform::windows::input::get_cursor_pos();
            println!("Cursor position: X={}, Y={}", x, y);
            process::exit(0);
        }

        // ------------------------------------------------------------------
        Commands::GetWindow { window } => {
            match crate::platform::windows::input::get_window_rect_by_title(&window) {
                Some((x, y, w, h)) => {
                    println!("Window '{}': X={}, Y={}, Width={}, Height={}", window, x, y, w, h);
                    process::exit(0);
                }
                None => {
                    eprintln!("error: window matching '{}' not found.", window);
                    process::exit(1);
                }
            }
        }

        // ------------------------------------------------------------------
        Commands::Reorder { profile, from, to } => {
            let content = match std::fs::read_to_string(&profile) {
                Ok(c) => c,
                Err(e) => { eprintln!("error: cannot read '{}': {}", profile.display(), e); process::exit(1); }
            };
            let mut queue: Vec<Item> = match serde_json::from_str(&content) {
                Ok(q) => q,
                Err(e) => { eprintln!("error: invalid .act profile: {}", e); process::exit(1); }
            };
            if let Err(e) = crate::models::reorder_queue(&mut queue, from, to) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
            let json = serde_json::to_string_pretty(&queue)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); process::exit(1); });
            if let Err(e) = std::fs::write(&profile, &json) {
                eprintln!("error writing profile: {}", e);
                process::exit(1);
            }
            println!("Reordered item from index {} to {} in '{}'.", from, to, profile.display());
            process::exit(0);
        }

        // ------------------------------------------------------------------
        Commands::Run { profile, repeat, r#in: delay, start_at } => {
            let content = match std::fs::read_to_string(&profile) {
                Ok(c) => c,
                Err(e) => { eprintln!("error: cannot read '{}': {}", profile.display(), e); process::exit(1); }
            };
            let queue: Vec<Item> = match serde_json::from_str(&content) {
                Ok(q) => q,
                Err(e) => { eprintln!("error: invalid .act profile: {}", e); process::exit(1); }
            };
            if queue.is_empty() { eprintln!("error: profile contains no items."); process::exit(1); }
            let scheduled = resolve_start(delay.as_deref(), start_at.as_deref());
            run_queue(queue, scheduled, repeat);
        }

        // ------------------------------------------------------------------
        Commands::Add {
            action, after, label, prompt, window, foreground,
            grace, post_wake, repeat, r#in: delay, start_at,
        } => {
            let secs = parse_duration_or_clock(&after).unwrap_or_else(|| {
                eprintln!("error: invalid --after '{}'. Use 5s / 1m30s / 2h or HH:MM:SS.", after);
                process::exit(1);
            });
            if secs == 0 { eprintln!("error: --after must be > 0."); process::exit(1); }

            let action_type: ActionType = action.into();
            let lbl = label.unwrap_or_else(|| action_type.as_str().to_string());
            let mut item = Item::new(secs, action_type);
            item.label              = lbl;
            item.prompt             = prompt.unwrap_or_default();
            item.require_foreground = foreground;
            item.sleep_cfg          = SleepConfig { pre_sleep_grace: grace, post_wake_delay: post_wake };
            if let Some(w) = window { item.target_window = w; }

            let scheduled = resolve_start(delay.as_deref(), start_at.as_deref());
            run_queue(vec![item], scheduled, repeat);
        }

        // ------------------------------------------------------------------
        Commands::Queue { steps, repeat, save, r#in: delay, start_at } => {
            let mut queue: Vec<Item> = Vec::new();
            for (i, raw) in steps.iter().enumerate() {
                match parse_step(raw) {
                    Ok(item) => queue.push(item),
                    Err(e)   => { eprintln!("error in --step {}: {}", i + 1, e); process::exit(1); }
                }
            }
            if queue.is_empty() { eprintln!("error: no valid steps."); process::exit(1); }

            if let Some(path) = &save {
                let json = serde_json::to_string_pretty(&queue)
                    .unwrap_or_else(|e| { eprintln!("error: {}", e); process::exit(1); });
                std::fs::write(path, &json)
                    .unwrap_or_else(|e| { eprintln!("error writing profile: {}", e); process::exit(1); });
                println!("Profile saved: {}", path.display());
                // If no scheduling args, save-only mode
                if delay.is_none() && start_at.is_none() {
                    process::exit(0);
                }
            }

            let scheduled = resolve_start(delay.as_deref(), start_at.as_deref());
            run_queue(queue, scheduled, repeat);
        }
    }
}

// ---------------------------------------------------------------------------
// Queue runner (headless, stdout-only)
// ---------------------------------------------------------------------------

fn run_queue(queue: Vec<Item>, start_at: Option<chrono::DateTime<Local>>, repeat: u32) -> ! {
    let rep_label = if repeat == 0 {
        " (Loop: infinite)".to_string()
    } else if repeat > 1 {
        format!(" (Loop: {}x)", repeat)
    } else {
        String::new()
    };
    println!("AutoClickTimer v{} -- {} item(s) queued{}.", CURRENT_VERSION, queue.len(), rep_label);

    if let Some(t) = start_at {
        let wait = (t - Local::now()).num_seconds().max(0) as u64;
        println!("Scheduled start: {} (in {})", t.format("%H:%M:%S"), fmt_duration(wait));
    }

    println!("Press Ctrl+C to abort.\n");

    let exit_code = Arc::new(Mutex::new(0i32));
    let cb_code   = Arc::clone(&exit_code);

    let mut executor = QueueExecutor::new();
    executor.start(queue, start_at, repeat, move |event| handle_event(&event, &cb_code));

    loop {
        std::thread::sleep(Duration::from_millis(200));
        if !executor.is_running() { break; }
    }

    println!();
    process::exit(*exit_code.lock().unwrap());
}

fn handle_event(event: &ExecutorEvent, exit_code: &Arc<Mutex<i32>>) {
    match event {
        ExecutorEvent::Log(msg) => {
            println!("[{}] {}", Local::now().format("%H:%M:%S"), msg);
        }
        ExecutorEvent::StepStart { index, total_items, label, duration_secs } => {
            println!("\n[{}/{}] Starting: {} ({})", index + 1, total_items, label, fmt_duration(*duration_secs));
        }
        ExecutorEvent::Tick { rem, .. } => {
            print!("\r  Remaining: {}   ", fmt_duration(*rem));
            let _ = std::io::stdout().flush();
        }
        ExecutorEvent::StepDone { index }          => { println!("\n  Step {} done.", index + 1); }
        ExecutorEvent::AllDone { total_items }     => { println!("\nAll {} action(s) completed successfully.", total_items); }
        ExecutorEvent::Stopped                     => { println!("\nQueue stopped."); *exit_code.lock().unwrap() = 1; }
        ExecutorEvent::Failsafe                    => { println!("\nFAILSAFE triggered -- aborted."); *exit_code.lock().unwrap() = 2; }
    }
}

// ---------------------------------------------------------------------------
// --step DSL parser
//
// Format: action:after[,key=value,...]
// Keys:   label=  prompt=  window=  grace=  post-wake=  foreground
// Note:   "prompt=" is greedy -- captures the rest of the string.
// ---------------------------------------------------------------------------

fn parse_step(raw: &str) -> Result<Item, String> {
    let (head, opts_str) = match raw.split_once(',') {
        Some((h, t)) => (h, t),
        None          => (raw, ""),
    };

    let (action_str, after_str) = head
        .split_once(':')
        .ok_or_else(|| format!("missing ':' -- expected 'action:after', got '{}'", head))?;

    let secs = parse_duration_or_clock(after_str.trim())
        .ok_or_else(|| format!("invalid duration/time '{}'. Use 5s, 1m30s, 2h, or HH:MM:SS.", after_str.trim()))?;

    let action = ActionType::from_str_loose(action_str.trim());
    let mut item = Item::new(secs, action);
    item.label = action.as_str().to_string();

    let mut remaining = opts_str;
    loop {
        if remaining.is_empty() { break; }

        match remaining.split_once('=') {
            None => {
                // Boolean flag
                let (flag, tail) = remaining.split_once(',').unwrap_or((remaining, ""));
                match flag.trim() {
                    "foreground" => item.require_foreground = true,
                    other => return Err(format!("unknown option '{}' -- missing '='?", other)),
                }
                remaining = tail;
            }
            Some((key, rest)) => {
                if key.trim() == "prompt" {
                    item.prompt = rest.to_string();
                    break;
                }
                let (value, tail) = rest.split_once(',').unwrap_or((rest, ""));
                match key.trim() {
                    "label"     => item.label = value.to_string(),
                    "window"    => item.target_window = value.to_string(),
                    "grace"     => item.sleep_cfg.pre_sleep_grace = value.parse()
                                    .map_err(|_| format!("grace must be a number, got '{}'", value))?,
                    "post-wake" => item.sleep_cfg.post_wake_delay  = value.parse()
                                    .map_err(|_| format!("post-wake must be a number, got '{}'", value))?,
                    other       => return Err(format!("unknown option '{}'", other)),
                }
                remaining = tail;
            }
        }
    }

    Ok(item)
}

// ---------------------------------------------------------------------------
// Time / duration helpers
// ---------------------------------------------------------------------------

fn parse_duration_or_clock(s: &str) -> Option<u64> {
    parse_duration_str(s).or_else(|| {
        parse_clock_time(s).map(|dt| (dt - Local::now()).num_seconds().max(1) as u64)
    })
}

pub fn parse_duration_str(s: &str) -> Option<u64> {
    if let Ok(n) = s.parse::<u64>() { return Some(n); }
    let mut total: u64 = 0;
    let mut buf = String::new();
    let mut matched = false;
    for ch in s.chars() {
        match ch {
            '0'..='9' => buf.push(ch),
            'h' | 'H' => { total += buf.parse::<u64>().ok()? * 3600; buf.clear(); matched = true; }
            'm' | 'M' => { total += buf.parse::<u64>().ok()? * 60;   buf.clear(); matched = true; }
            's' | 'S' => { total += buf.parse::<u64>().ok()?;         buf.clear(); matched = true; }
            _ => return None,
        }
    }
    if !buf.is_empty() { total += buf.parse::<u64>().ok()?; matched = true; }
    if matched { Some(total) } else { None }
}

pub fn parse_clock_time(s: &str) -> Option<chrono::DateTime<Local>> {
    let p: Vec<&str> = s.splitn(3, ':').collect();
    if p.len() != 3 { return None; }
    let (h, m, sec): (u32, u32, u32) = (p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?);
    if h > 23 || m > 59 || sec > 59 { return None; }
    let now = Local::now();
    let naive = chrono::NaiveTime::from_hms_opt(h, m, sec)?;
    let mut dt = now.date_naive().and_time(naive).and_local_timezone(Local).single()?;
    if dt <= now { dt = dt + chrono::Duration::days(1); }
    Some(dt)
}

fn resolve_start(delay: Option<&str>, clock: Option<&str>) -> Option<chrono::DateTime<Local>> {
    if let Some(d) = delay {
        let secs = parse_duration_str(d).unwrap_or_else(|| {
            eprintln!("error: invalid --in value '{}'. Use 30m, 1h, etc.", d);
            process::exit(1);
        });
        Some(Local::now() + chrono::Duration::seconds(secs as i64))
    } else {
        clock.map(|c| parse_clock_time(c).unwrap_or_else(|| {
            eprintln!("error: invalid --start-at value '{}'. Use HH:MM:SS.", c);
            process::exit(1);
        }))
    }
}

fn fmt_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h {:02}m {:02}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}
