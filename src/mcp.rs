//! Model Context Protocol (MCP) Server for AutoClickTimer.
//!
//! Exposes complete GUI automation parity over standard MCP JSON-RPC 2.0 (stdio).
//! Specification version: 2024-11-05.
//!
//! Tools exposed:
//!   - `act_execute_action`: Run a single immediate or delayed action
//!   - `act_schedule_queue`: Build and run a multi-step queue
//!   - `act_run_profile`: Execute a saved .act profile file
//!   - `act_save_profile`: Validate and save a queue to an .act profile file
//!   - `act_get_status`: Query running queue state and progress in real time
//!   - `act_cancel`: Abort the currently running queue or timer
//!   - `act_list_windows`: Enumerate visible window titles for targeting
//!   - `act_set_caffeine`: Direct toggle of screen/sleep keep-awake mode
//!   - `act_configure_passwordless_wake`: Configure zero-password wake on current machine

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Local, NaiveTime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::executor::QueueExecutor;
use crate::models::{ActionType, Item, SleepConfig};
use crate::platform::windows::input::get_open_windows;
use crate::platform::windows::power::{configure_passwordless_wake, set_caffeine};
use crate::updater::CURRENT_VERSION;

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

pub struct McpServer {
    executor: Arc<Mutex<QueueExecutor>>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            executor: Arc::new(Mutex::new(QueueExecutor::new())),
        }
    }

    pub fn run(&self) -> ! {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[MCP] stdin error: {}", e);
                    break;
                }
            };

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("[MCP] parse error: {}", e);
                    let err_resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                            data: None,
                        }),
                    };
                    send_response(&mut stdout, &err_resp);
                    continue;
                }
            };

            if let Some(resp) = self.handle_request(request) {
                send_response(&mut stdout, &resp);
            }
        }

        std::process::exit(0);
    }

    fn handle_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id.clone();
        let method = req.method.as_str();

        match method {
            "initialize" => {
                let result = json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    },
                    "serverInfo": {
                        "name": "autoclicktimer-mcp",
                        "version": CURRENT_VERSION
                    }
                });
                Some(success_response(id, result))
            }
            "notifications/initialized" => {
                // Client initialized notification, no response required
                None
            }
            "ping" => Some(success_response(id, json!({}))),
            "tools/list" => {
                let tools = get_tool_definitions();
                Some(success_response(id, json!({ "tools": tools })))
            }
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                let tool_result = self.dispatch_tool(tool_name, args);
                match tool_result {
                    Ok(val) => {
                        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": text
                                }
                            ],
                            "isError": false
                        });
                        Some(success_response(id, result))
                    }
                    Err(err_msg) => {
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": err_msg
                                }
                            ],
                            "isError": true
                        });
                        Some(success_response(id, result))
                    }
                }
            }
            _ => {
                if id.is_some() {
                    Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Method not found: {}", method),
                            data: None,
                        }),
                    })
                } else {
                    None
                }
            }
        }
    }

    fn dispatch_tool(&self, name: &str, args: Value) -> Result<Value, String> {
        match name {
            "act_execute_action" => self.tool_execute_action(args),
            "act_schedule_queue" => self.tool_schedule_queue(args),
            "act_run_profile" => self.tool_run_profile(args),
            "act_save_profile" => self.tool_save_profile(args),
            "act_get_status" => self.tool_get_status(),
            "act_cancel" => self.tool_cancel(),
            "act_list_windows" => self.tool_list_windows(),
            "act_set_caffeine" => self.tool_set_caffeine(args),
            "act_configure_passwordless_wake" => self.tool_configure_passwordless_wake(),
            "act_get_cursor_pos" => self.tool_get_cursor_pos(),
            "act_get_window_rect" => self.tool_get_window_rect(args),
            "act_reorder_queue" => self.tool_reorder_queue(args),
            _ => Err(format!("Unknown tool: '{}'", name)),
        }
    }

    fn tool_get_cursor_pos(&self) -> Result<Value, String> {
        let (x, y) = crate::platform::windows::input::get_cursor_pos();
        Ok(json!({
            "x": x,
            "y": y
        }))
    }

    fn tool_get_window_rect(&self, args: Value) -> Result<Value, String> {
        let win = args.get("window").and_then(|v| v.as_str()).ok_or("Missing required parameter: 'window'")?;
        match crate::platform::windows::input::get_window_rect_by_title(win) {
            Some((x, y, w, h)) => Ok(json!({
                "found": true,
                "window": win,
                "x": x,
                "y": y,
                "width": w,
                "height": h
            })),
            None => Ok(json!({
                "found": false,
                "window": win,
                "message": format!("No visible window matching '{}' was found.", win)
            })),
        }
    }

    fn tool_reorder_queue(&self, args: Value) -> Result<Value, String> {
        let from = args.get("from_index").and_then(|v| v.as_u64()).ok_or("Missing required parameter: 'from_index'")? as usize;
        let to = args.get("to_index").and_then(|v| v.as_u64()).ok_or("Missing required parameter: 'to_index'")? as usize;

        if let Some(profile_str) = args.get("profile_path").and_then(|v| v.as_str()) {
            let path = PathBuf::from(profile_str);
            let content = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read profile '{}': {}", path.display(), e))?;
            let mut queue: Vec<Item> = serde_json::from_str(&content).map_err(|e| format!("Invalid profile format: {}", e))?;
            crate::models::reorder_queue(&mut queue, from, to)?;
            let json_str = serde_json::to_string_pretty(&queue).map_err(|e| format!("Failed to serialize profile: {}", e))?;
            std::fs::write(&path, json_str).map_err(|e| format!("Failed to write profile to '{}': {}", path.display(), e))?;
            Ok(json!({
                "status": "reordered",
                "profile_path": path.display().to_string(),
                "from_index": from,
                "to_index": to,
                "item_count": queue.len()
            }))
        } else {
            Ok(json!({
                "status": "validated",
                "from_index": from,
                "to_index": to
            }))
        }
    }

    fn tool_execute_action(&self, args: Value) -> Result<Value, String> {
        let action_str = args.get("action").and_then(|v| v.as_str()).ok_or("Missing required parameter: 'action'")?;
        let after_val = args.get("after").ok_or("Missing required parameter: 'after'")?;
        
        let secs = parse_seconds_from_value(after_val)?;
        if secs == 0 {
            return Err("'after' duration must be greater than 0 seconds.".to_string());
        }

        let action_type = ActionType::from_str_loose(action_str);
        let mut item = Item::new(secs, action_type);

        if let Some(lbl) = args.get("label").and_then(|v| v.as_str()) {
            item.label = lbl.to_string();
        } else {
            item.label = action_type.as_str().to_string();
        }

        if let Some(prompt) = args.get("prompt").and_then(|v| v.as_str()) {
            item.prompt = prompt.to_string();
        }

        if let Some(win) = args.get("window").and_then(|v| v.as_str()) {
            item.target_window = win.to_string();
        }

        if let Some(fg) = args.get("foreground").and_then(|v| v.as_bool()) {
            item.require_foreground = fg;
        }

        let grace = args.get("pre_sleep_grace").and_then(|v| v.as_u64()).unwrap_or(5);
        let post_wake = args.get("post_wake_delay").and_then(|v| v.as_u64()).unwrap_or(30);
        item.sleep_cfg = SleepConfig {
            pre_sleep_grace: grace,
            post_wake_delay: post_wake,
        };

        let repeat = args.get("repeat_count").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

        let start_in = args.get("start_in").and_then(|v| v.as_str());
        let start_at = args.get("start_at").and_then(|v| v.as_str());
        let scheduled_start = resolve_start_time(start_in, start_at)?;

        let async_exec = args.get("async_execution").and_then(|v| v.as_bool()).unwrap_or(true);

        self.start_queue(vec![item], scheduled_start, repeat, async_exec)
    }

    fn tool_schedule_queue(&self, args: Value) -> Result<Value, String> {
        let steps_val = args.get("steps").and_then(|v| v.as_array()).ok_or("Missing required parameter: 'steps' (array of step objects)")?;
        if steps_val.is_empty() {
            return Err("'steps' array cannot be empty.".to_string());
        }

        let mut queue = Vec::new();
        for (i, step) in steps_val.iter().enumerate() {
            let action_str = step.get("action").and_then(|v| v.as_str()).ok_or_else(|| format!("Step {} is missing 'action'", i + 1))?;
            let after_val = step.get("after").ok_or_else(|| format!("Step {} is missing 'after'", i + 1))?;
            let secs = parse_seconds_from_value(after_val)?;
            if secs == 0 {
                return Err(format!("Step {} 'after' must be > 0", i + 1));
            }

            let action_type = ActionType::from_str_loose(action_str);
            let mut item = Item::new(secs, action_type);

            if let Some(lbl) = step.get("label").and_then(|v| v.as_str()) {
                item.label = lbl.to_string();
            } else {
                item.label = action_type.as_str().to_string();
            }

            if let Some(prompt) = step.get("prompt").and_then(|v| v.as_str()) {
                item.prompt = prompt.to_string();
            }

            if let Some(win) = step.get("window").and_then(|v| v.as_str()) {
                item.target_window = win.to_string();
            }

            if let Some(fg) = step.get("foreground").and_then(|v| v.as_bool()) {
                item.require_foreground = fg;
            }

            let grace = step.get("pre_sleep_grace").and_then(|v| v.as_u64()).unwrap_or(5);
            let post_wake = step.get("post_wake_delay").and_then(|v| v.as_u64()).unwrap_or(30);
            item.sleep_cfg = SleepConfig {
                pre_sleep_grace: grace,
                post_wake_delay: post_wake,
            };

            queue.push(item);
        }

        if let Some(save_path) = args.get("save_profile_path").and_then(|v| v.as_str()) {
            let path = PathBuf::from(save_path);
            let json_str = serde_json::to_string_pretty(&queue).map_err(|e| format!("Failed to serialize profile: {}", e))?;
            std::fs::write(&path, json_str).map_err(|e| format!("Failed to write profile to '{}': {}", path.display(), e))?;
        }

        let repeat = args.get("repeat_count").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

        let start_in = args.get("start_in").and_then(|v| v.as_str());
        let start_at = args.get("start_at").and_then(|v| v.as_str());
        let scheduled_start = resolve_start_time(start_in, start_at)?;

        let async_exec = args.get("async_execution").and_then(|v| v.as_bool()).unwrap_or(true);

        self.start_queue(queue, scheduled_start, repeat, async_exec)
    }

    fn tool_run_profile(&self, args: Value) -> Result<Value, String> {
        let profile_str = args.get("profile_path").and_then(|v| v.as_str()).ok_or("Missing required parameter: 'profile_path'")?;
        let path = PathBuf::from(profile_str);
        let content = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read profile '{}': {}", path.display(), e))?;
        let queue: Vec<Item> = serde_json::from_str(&content).map_err(|e| format!("Invalid profile format: {}", e))?;
        if queue.is_empty() {
            return Err("Profile contains no items.".to_string());
        }

        let repeat = args.get("repeat_count").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

        let start_in = args.get("start_in").and_then(|v| v.as_str());
        let start_at = args.get("start_at").and_then(|v| v.as_str());
        let scheduled_start = resolve_start_time(start_in, start_at)?;

        let async_exec = args.get("async_execution").and_then(|v| v.as_bool()).unwrap_or(true);

        self.start_queue(queue, scheduled_start, repeat, async_exec)
    }

    fn tool_save_profile(&self, args: Value) -> Result<Value, String> {
        let profile_str = args.get("profile_path").and_then(|v| v.as_str()).ok_or("Missing required parameter: 'profile_path'")?;
        let steps_val = args.get("steps").and_then(|v| v.as_array()).ok_or("Missing required parameter: 'steps'")?;
        if steps_val.is_empty() {
            return Err("'steps' array cannot be empty.".to_string());
        }

        let mut queue = Vec::new();
        for (i, step) in steps_val.iter().enumerate() {
            let action_str = step.get("action").and_then(|v| v.as_str()).ok_or_else(|| format!("Step {} is missing 'action'", i + 1))?;
            let after_val = step.get("after").ok_or_else(|| format!("Step {} is missing 'after'", i + 1))?;
            let secs = parse_seconds_from_value(after_val)?;
            let action_type = ActionType::from_str_loose(action_str);
            let mut item = Item::new(secs, action_type);

            if let Some(lbl) = step.get("label").and_then(|v| v.as_str()) {
                item.label = lbl.to_string();
            } else {
                item.label = action_type.as_str().to_string();
            }

            if let Some(prompt) = step.get("prompt").and_then(|v| v.as_str()) {
                item.prompt = prompt.to_string();
            }

            if let Some(win) = step.get("window").and_then(|v| v.as_str()) {
                item.target_window = win.to_string();
            }

            if let Some(fg) = step.get("foreground").and_then(|v| v.as_bool()) {
                item.require_foreground = fg;
            }

            let grace = step.get("pre_sleep_grace").and_then(|v| v.as_u64()).unwrap_or(5);
            let post_wake = step.get("post_wake_delay").and_then(|v| v.as_u64()).unwrap_or(30);
            item.sleep_cfg = SleepConfig {
                pre_sleep_grace: grace,
                post_wake_delay: post_wake,
            };

            queue.push(item);
        }

        let path = PathBuf::from(profile_str);
        let json_str = serde_json::to_string_pretty(&queue).map_err(|e| format!("Failed to serialize profile: {}", e))?;
        std::fs::write(&path, json_str).map_err(|e| format!("Failed to write profile to '{}': {}", path.display(), e))?;

        Ok(json!({
            "status": "saved",
            "profile_path": path.display().to_string(),
            "item_count": queue.len()
        }))
    }

    fn tool_get_status(&self) -> Result<Value, String> {
        let snapshot = self.executor.lock().unwrap().get_snapshot();
        serde_json::to_value(&snapshot).map_err(|e| e.to_string())
    }

    fn tool_cancel(&self) -> Result<Value, String> {
        let executor = self.executor.lock().unwrap();
        executor.stop();
        let snapshot = executor.get_snapshot();
        Ok(json!({
            "status": "cancelled",
            "message": "Active queue or timer was stopped.",
            "snapshot": snapshot
        }))
    }

    fn tool_list_windows(&self) -> Result<Value, String> {
        let windows = get_open_windows();
        Ok(json!({
            "count": windows.len(),
            "windows": windows
        }))
    }

    fn tool_set_caffeine(&self, args: Value) -> Result<Value, String> {
        let active = args.get("active").and_then(|v| v.as_bool()).ok_or("Missing required parameter: 'active' (boolean)")?;
        let duration_secs = args.get("duration_seconds").and_then(|v| v.as_u64());

        set_caffeine(active);

        if active {
            if let Some(secs) = duration_secs {
                if secs > 0 {
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(secs));
                        set_caffeine(false);
                    });
                    return Ok(json!({
                        "active": true,
                        "duration_seconds": secs,
                        "message": format!("Caffeine active for {} seconds, then auto-disable.", secs)
                    }));
                }
            }
            Ok(json!({
                "active": true,
                "message": "Caffeine keep-awake enabled indefinitely."
            }))
        } else {
            Ok(json!({
                "active": false,
                "message": "Caffeine keep-awake disabled."
            }))
        }
    }

    fn tool_configure_passwordless_wake(&self) -> Result<Value, String> {
        match configure_passwordless_wake() {
            Ok(msg) => Ok(json!({
                "status": "configured",
                "message": msg,
                "details": "ScreenSaverIsSecure was set to 0 and power scheme console lock was configured for the current user session."
            })),
            Err(e) => Err(format!("Failed to configure passwordless wake: {}", e)),
        }
    }

    fn start_queue(
        &self,
        queue: Vec<Item>,
        start_at: Option<DateTime<Local>>,
        repeat: u32,
        async_exec: bool,
    ) -> Result<Value, String> {
        let mut executor = self.executor.lock().unwrap();

        if executor.is_running() {
            return Err("An automation queue is already running. Call 'act_cancel' first to stop it.".to_string());
        }

        let item_count = queue.len();
        let total_duration_secs: u64 = queue.iter().map(|it| it.total).sum();

        executor.start(queue, start_at, repeat, |_event| {
            // Background event sink
        });

        let initial_snapshot = executor.get_snapshot();

        if async_exec {
            Ok(json!({
                "status": "scheduled",
                "async": true,
                "item_count": item_count,
                "total_duration_seconds": total_duration_secs,
                "scheduled_start": start_at.map(|t| t.format("%H:%M:%S").to_string()),
                "snapshot": initial_snapshot
            }))
        } else {
            // Synchronous wait
            drop(executor);
            loop {
                std::thread::sleep(Duration::from_millis(300));
                let snap = self.executor.lock().unwrap().get_snapshot();
                if !snap.is_running {
                    return Ok(json!({
                        "status": snap.status,
                        "async": false,
                        "snapshot": snap
                    }));
                }
            }
        }
    }
}

fn success_response(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    }
}

fn send_response(stdout: &mut io::Stdout, resp: &JsonRpcResponse) {
    if let Ok(json_str) = serde_json::to_string(resp) {
        let _ = writeln!(stdout, "{}", json_str);
        let _ = stdout.flush();
    }
}

pub fn run_mcp_server() -> ! {
    let server = McpServer::new();
    server.run()
}

fn parse_seconds_from_value(val: &Value) -> Result<u64, String> {
    if let Some(num) = val.as_u64() {
        return Ok(num);
    }
    if let Some(s) = val.as_str() {
        return parse_duration_or_clock(s)
            .ok_or_else(|| format!("Invalid duration or clock format: '{}'. Use '5s', '10m', '2h', or 'HH:MM:SS'.", s));
    }
    Err(format!("Expected duration string or number, got: {}", val))
}

fn parse_duration_or_clock(s: &str) -> Option<u64> {
    let trimmed = s.trim();
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(secs);
    }

    if let Some(secs) = parse_duration_str(trimmed) {
        return Some(secs);
    }

    if let Ok(time) = NaiveTime::parse_from_str(trimmed, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(trimmed, "%H:%M"))
    {
        let now = Local::now();
        let target_today = now.date_naive().and_time(time);
        let mut target_dt = target_today.and_local_timezone(Local).single().unwrap_or(now);
        if target_dt <= now {
            target_dt = target_dt + chrono::Duration::days(1);
        }
        let delta = (target_dt - now).num_seconds();
        return Some(delta.max(1) as u64);
    }

    None
}

fn parse_duration_str(s: &str) -> Option<u64> {
    let s = s.trim().to_lowercase();
    let mut total_secs: u64 = 0;
    let mut current_digits = String::new();
    let mut matched_any = false;

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            current_digits.push(ch);
        } else {
            if current_digits.is_empty() {
                return None;
            }
            let n: u64 = current_digits.parse().ok()?;
            current_digits.clear();
            matched_any = true;
            match ch {
                'h' => total_secs = total_secs.checked_add(n.checked_mul(3600)?)?,
                'm' => total_secs = total_secs.checked_add(n.checked_mul(60)?)?,
                's' => total_secs = total_secs.checked_add(n)?,
                _ => return None,
            }
        }
    }

    if !current_digits.is_empty() {
        if !matched_any {
            return current_digits.parse::<u64>().ok();
        } else {
            return None;
        }
    }

    if matched_any {
        Some(total_secs)
    } else {
        None
    }
}

fn resolve_start_time(
    delay_str: Option<&str>,
    clock_str: Option<&str>,
) -> Result<Option<DateTime<Local>>, String> {
    if let Some(delay) = delay_str {
        let secs = parse_duration_str(delay)
            .ok_or_else(|| format!("Invalid delay format for start_in: '{}'", delay))?;
        return Ok(Some(Local::now() + chrono::Duration::seconds(secs as i64)));
    }

    if let Some(clock) = clock_str {
        let time = NaiveTime::parse_from_str(clock.trim(), "%H:%M:%S")
            .or_else(|_| NaiveTime::parse_from_str(clock.trim(), "%H:%M"))
            .map_err(|_| format!("Invalid start_at clock time: '{}'. Expected HH:MM:SS or HH:MM", clock))?;

        let now = Local::now();
        let mut target_dt = now.date_naive().and_time(time).and_local_timezone(Local).single().unwrap_or(now);
        if target_dt <= now {
            target_dt = target_dt + chrono::Duration::days(1);
        }
        return Ok(Some(target_dt));
    }

    Ok(None)
}

fn get_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "act_execute_action",
            "description": "Execute a single desktop action immediately or after a countdown. Supports click, enter, type, sleep, shutdown, and caffeine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["enter", "click", "type", "sleep", "shutdown", "caffeine"],
                        "description": "Action type to execute."
                    },
                    "after": {
                        "type": "string",
                        "description": "When to fire: duration (e.g. '5s', '1m30s', '2h', bare seconds) or clock time ('HH:MM:SS')."
                    },
                    "label": {
                        "type": "string",
                        "description": "Human-readable label for this action."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Text to type (required when action is 'type')."
                    },
                    "window": {
                        "type": "string",
                        "description": "Target window title substring. If empty, acts globally on active window."
                    },
                    "foreground": {
                        "type": "boolean",
                        "description": "If true, brings the target window to the foreground before acting."
                    },
                    "pre_sleep_grace": {
                        "type": "integer",
                        "description": "Pre-sleep grace period in seconds (sleep action only, default 5)."
                    },
                    "post_wake_delay": {
                        "type": "integer",
                        "description": "Post-wake delay in seconds (sleep action only, default 30)."
                    },
                    "repeat_count": {
                        "type": "integer",
                        "description": "Number of times to repeat the action (default: 1, 0 = infinite loop)."
                    },
                    "start_in": {
                        "type": "string",
                        "description": "Optional delay before starting the countdown (e.g. '30m', '1h')."
                    },
                    "start_at": {
                        "type": "string",
                        "description": "Optional absolute start time ('HH:MM:SS')."
                    },
                    "async_execution": {
                        "type": "boolean",
                        "description": "If true (default), starts in background and returns immediately. If false, blocks until done."
                    }
                },
                "required": ["action", "after"]
            }
        }),
        json!({
            "name": "act_schedule_queue",
            "description": "Build, validate, and execute a multi-step automation queue with structured step parameters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "description": "Array of step objects in execution order.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "enum": ["enter", "click", "type", "sleep", "shutdown", "caffeine"],
                                    "description": "Action type for this step."
                                },
                                "after": {
                                    "type": "string",
                                    "description": "Duration (e.g. '10s', '2h') or clock time ('HH:MM:SS')."
                                },
                                "label": { "type": "string" },
                                "prompt": { "type": "string" },
                                "window": { "type": "string" },
                                "foreground": { "type": "boolean" },
                                "pre_sleep_grace": { "type": "integer" },
                                "post_wake_delay": { "type": "integer" }
                            },
                            "required": ["action", "after"]
                        }
                    },
                    "repeat_count": {
                        "type": "integer",
                        "description": "Number of times to loop the entire queue (default: 1, 0 = infinite loop)."
                    },
                    "save_profile_path": {
                        "type": "string",
                        "description": "Optional file path to save the queue as a .act JSON profile."
                    },
                    "start_in": {
                        "type": "string",
                        "description": "Optional delay before queue start (e.g. '30m')."
                    },
                    "start_at": {
                        "type": "string",
                        "description": "Optional absolute start time ('HH:MM:SS')."
                    },
                    "async_execution": {
                        "type": "boolean",
                        "description": "If true (default), runs in background. If false, blocks until complete."
                    }
                },
                "required": ["steps"]
            }
        }),
        json!({
            "name": "act_run_profile",
            "description": "Execute a saved .act profile file headlessly.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "profile_path": {
                        "type": "string",
                        "description": "Absolute or relative path to the .act JSON profile file."
                    },
                    "repeat_count": {
                        "type": "integer",
                        "description": "Number of times to loop the profile queue (default: 1, 0 = infinite loop)."
                    },
                    "start_in": { "type": "string" },
                    "start_at": { "type": "string" },
                    "async_execution": { "type": "boolean" }
                },
                "required": ["profile_path"]
            }
        }),
        json!({
            "name": "act_save_profile",
            "description": "Validate and save a list of automation steps to a .act JSON profile file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "profile_path": {
                        "type": "string",
                        "description": "File path to save the .act profile."
                    },
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "enum": ["enter", "click", "type", "sleep", "shutdown", "caffeine"]
                                },
                                "after": { "type": "string" },
                                "label": { "type": "string" },
                                "prompt": { "type": "string" },
                                "window": { "type": "string" },
                                "foreground": { "type": "boolean" },
                                "pre_sleep_grace": { "type": "integer" },
                                "post_wake_delay": { "type": "integer" }
                            },
                            "required": ["action", "after"]
                        }
                    }
                },
                "required": ["profile_path", "steps"]
            }
        }),
        json!({
            "name": "act_reorder_queue",
            "description": "Reorder steps in a saved .act profile file or validate step move indices.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from_index": {
                        "type": "integer",
                        "description": "Source index of the item to move (0-indexed)."
                    },
                    "to_index": {
                        "type": "integer",
                        "description": "Destination index to move the item to (0-indexed)."
                    },
                    "profile_path": {
                        "type": "string",
                        "description": "Optional file path to .act profile to modify in-place."
                    }
                },
                "required": ["from_index", "to_index"]
            }
        }),
        json!({
            "name": "act_get_status",
            "description": "Query the current automation queue state, active step index, iteration counters, remaining countdown seconds, and phase in real time.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "act_get_cursor_pos",
            "description": "Query current screen coordinates (X, Y) of the Windows mouse cursor.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "act_get_window_rect",
            "description": "Query the screen bounding box coordinates (X, Y, Width, Height) of a window matching title substring.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "window": {
                        "type": "string",
                        "description": "Window title substring to search for."
                    }
                },
                "required": ["window"]
            }
        }),
        json!({
            "name": "act_cancel",
            "description": "Immediately cancel the active automation queue or countdown timer.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "act_list_windows",
            "description": "Enumerate titles of all visible open windows on the Windows desktop for window-specific targeting.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "act_set_caffeine",
            "description": "Directly enable or disable Windows Caffeine keep-awake (prevents screen lock and sleep).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "active": {
                        "type": "boolean",
                        "description": "True to keep awake, false to restore normal power management."
                    },
                    "duration_seconds": {
                        "type": "integer",
                        "description": "Optional duration in seconds to keep awake before auto-disabling."
                    }
                },
                "required": ["active"]
            }
        }),
        json!({
            "name": "act_configure_passwordless_wake",
            "description": "Configure the current Windows user session to wake directly without prompting for a lock screen password. Requires no admin privileges.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        })
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize_response() {
        let server = McpServer::new();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: None,
        };
        let resp = server.handle_request(req).unwrap();
        assert_eq!(resp.id, Some(json!(1)));
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], "autoclicktimer-mcp");
    }

    #[test]
    fn test_mcp_tools_list() {
        let server = McpServer::new();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = server.handle_request(req).unwrap();
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(tools.len() >= 8);
        assert!(tools.iter().any(|t| t["name"] == "act_execute_action"));
        assert!(tools.iter().any(|t| t["name"] == "act_schedule_queue"));
        assert!(tools.iter().any(|t| t["name"] == "act_get_status"));
        assert!(tools.iter().any(|t| t["name"] == "act_cancel"));
    }

    #[test]
    fn test_parse_seconds() {
        assert_eq!(parse_duration_str("5s"), Some(5));
        assert_eq!(parse_duration_str("1m30s"), Some(90));
        assert_eq!(parse_duration_str("2h"), Some(7200));
        assert_eq!(parse_duration_or_clock("45"), Some(45));
    }

    #[test]
    fn test_mcp_get_status_idle() {
        let server = McpServer::new();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(3)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "act_get_status",
                "arguments": {}
            })),
        };
        let resp = server.handle_request(req).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], false);
        let content_text = result["content"][0]["text"].as_str().unwrap();
        assert!(content_text.contains("\"is_running\": false"));
    }

    #[test]
    fn test_mcp_cancel_tool() {
        let server = McpServer::new();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(4)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "act_cancel",
                "arguments": {}
            })),
        };
        let resp = server.handle_request(req).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], false);
        let content_text = result["content"][0]["text"].as_str().unwrap();
        assert!(content_text.contains("\"status\": \"cancelled\""));
    }

    #[test]
    fn test_mcp_unknown_tool_error() {
        let server = McpServer::new();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(5)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "act_nonexistent_tool",
                "arguments": {}
            })),
        };
        let resp = server.handle_request(req).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
        let content_text = result["content"][0]["text"].as_str().unwrap();
        assert!(content_text.contains("Unknown tool"));
    }

    #[test]
    fn test_mcp_get_cursor_pos() {
        let server = McpServer::new();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(6)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "act_get_cursor_pos",
                "arguments": {}
            })),
        };
        let resp = server.handle_request(req).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"x\"") && text.contains("\"y\""));
    }

    #[test]
    fn test_mcp_reorder_queue_validation() {
        let server = McpServer::new();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(7)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "act_reorder_queue",
                "arguments": {
                    "from_index": 0,
                    "to_index": 1
                }
            })),
        };
        let resp = server.handle_request(req).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"status\": \"validated\""));
    }
}

