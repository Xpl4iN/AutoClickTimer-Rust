// Disable Windows console window in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod executor;
mod i18n;
mod models;
mod platform;
mod updater;

use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Local;
use slint::{Color, ModelRc, VecModel};

use crate::executor::{ExecutorEvent, QueueExecutor};
use crate::i18n::{fmt_time, set_language, t};
use crate::models::{ActionType, Item, ItemPhase, ItemStatus, SleepConfig};
use crate::platform::windows::input::get_open_windows;
use crate::platform::windows::power::{is_admin, request_elevation, set_caffeine};
use crate::updater::{check_for_update, download_and_apply, UpdateInfo, CURRENT_VERSION, REPO};

slint::include_modules!();

struct AppState {
    queue: Vec<Item>,
    log_lines: Vec<String>,
    update_info: Option<UpdateInfo>,
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = AppWindow::new()?;
    let window_handle = main_window.as_weak();

    let state = Arc::new(Mutex::new(AppState {
        queue: Vec::new(),
        log_lines: Vec::new(),
        update_info: None,
    }));

    let executor = Arc::new(Mutex::new(QueueExecutor::new()));

    // Populate initial window list
    refresh_window_list(&main_window);

    // ---- Event Callbacks ----

    // Add Item
    {
        let state = Arc::clone(&state);
        let window_handle = window_handle.clone();
        main_window.on_add_item(move |total, action_str, prompt, label, grace, post_wake, target_win, require_fg| {
            if total <= 0 {
                return;
            }

            let action = ActionType::from_str_loose(action_str.as_str());

            if action == ActionType::Sleep && !is_admin() {
                if let Err(e) = request_elevation() {
                    eprintln!("Elevation error: {}", e);
                }
                return;
            }

            let mut display_label = label.to_string();
            if display_label.is_empty() {
                display_label = match action {
                    ActionType::Enter => t("act_enter").to_string(),
                    ActionType::Click => t("act_click").to_string(),
                    ActionType::Type => t("act_type").to_string(),
                    ActionType::Sleep => t("default_sleep_label").to_string(),
                    ActionType::Shutdown => t("default_shutdown_label").to_string(),
                };
            }

            let mut item = Item::new(total as u64, action);
            item.prompt = prompt.to_string();
            item.label = display_label;
            item.sleep_cfg = SleepConfig {
                pre_sleep_grace: grace as u64,
                post_wake_delay: post_wake as u64,
            };

            let win_str = target_win.to_string();
            if win_str != "(Global / Aktives Fenster)" && win_str != "(Global / Active Window)" {
                item.target_window = win_str;
            }
            item.require_foreground = require_fg;

            let mut s = state.lock().unwrap();
            s.queue.push(item);

            if let Some(app) = window_handle.upgrade() {
                sync_queue_to_ui(&app, &s.queue);
            }
        });
    }

    // Add Preset
    {
        let state = Arc::clone(&state);
        let window_handle = window_handle.clone();
        main_window.on_add_preset(move |preset_type, total| {
            if total <= 0 {
                return;
            }

            let duration = total as u64;

            let mut s = state.lock().unwrap();
            match preset_type.as_str() {
                "shutdown" => {
                    let mut item = Item::new(duration, ActionType::Shutdown);
                    item.label = t("default_shutdown_label").to_string();
                    s.queue.push(item);
                }
                "enter" | "click" => {
                    if !is_admin() {
                        let _ = request_elevation();
                        return;
                    }

                    let mut sleep_item = Item::new(duration, ActionType::Sleep);
                    sleep_item.label = t("default_sleep_label").to_string();

                    let post_action = if preset_type.as_str() == "enter" {
                        ActionType::Enter
                    } else {
                        ActionType::Click
                    };

                    let mut post_item = Item::new(2, post_action);
                    post_item.label = if preset_type.as_str() == "enter" {
                        t("post_wake_enter").to_string()
                    } else {
                        t("post_wake_click").to_string()
                    };

                    s.queue.push(sleep_item);
                    s.queue.push(post_item);
                }
                _ => {}
            }

            if let Some(app) = window_handle.upgrade() {
                sync_queue_to_ui(&app, &s.queue);
            }
        });
    }

    // Remove Item
    {
        let state = Arc::clone(&state);
        let window_handle = window_handle.clone();
        main_window.on_remove_item(move |idx| {
            let index = idx as usize;
            let mut s = state.lock().unwrap();
            if index < s.queue.len() {
                s.queue.remove(index);
                if let Some(app) = window_handle.upgrade() {
                    sync_queue_to_ui(&app, &s.queue);
                }
            }
        });
    }

    // Start Queue
    {
        let state = Arc::clone(&state);
        let executor = Arc::clone(&executor);
        let window_handle = window_handle.clone();

        main_window.on_start_queue(move || {
            let q_clone = {
                let s = state.lock().unwrap();
                s.queue.clone()
            };

            if q_clone.is_empty() {
                return;
            }

            if let Some(app) = window_handle.upgrade() {
                app.set_is_running(true);
                app.set_status_text(t("status_running").into());
            }

            let handle = window_handle.clone();
            let state_ref = Arc::clone(&state);

            executor.lock().unwrap().start(q_clone, None, move |event| {
                let handle = handle.clone();
                let state_ref = Arc::clone(&state_ref);

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = handle.upgrade() {
                        handle_executor_event(&app, &state_ref, event);
                    }
                });
            });
        });
    }

    // Stop Queue
    {
        let executor = Arc::clone(&executor);
        main_window.on_stop_queue(move || {
            executor.lock().unwrap().stop();
        });
    }

    // Reset Queue
    {
        let state = Arc::clone(&state);
        let executor = Arc::clone(&executor);
        let window_handle = window_handle.clone();

        main_window.on_reset_queue(move || {
            executor.lock().unwrap().stop();
            let mut s = state.lock().unwrap();
            for item in s.queue.iter_mut() {
                item.reset();
            }
            if let Some(app) = window_handle.upgrade() {
                app.set_is_running(false);
                app.set_status_text("".into());
                sync_queue_to_ui(&app, &s.queue);
            }
        });
    }

    // Clear Queue
    {
        let state = Arc::clone(&state);
        let executor = Arc::clone(&executor);
        let window_handle = window_handle.clone();

        main_window.on_clear_queue(move || {
            executor.lock().unwrap().stop();
            let mut s = state.lock().unwrap();
            s.queue.clear();
            if let Some(app) = window_handle.upgrade() {
                app.set_is_running(false);
                app.set_status_text("".into());
                sync_queue_to_ui(&app, &s.queue);
            }
        });
    }

    // Toggle Caffeine
    {
        main_window.on_toggle_caffeine(move |active| {
            set_caffeine(active);
        });
    }

    // Switch Language
    {
        let state = Arc::clone(&state);
        let window_handle = window_handle.clone();
        main_window.on_switch_language(move |lang| {
            set_language(lang.as_str());
            if let Some(app) = window_handle.upgrade() {
                app.set_failsafe_text(t("failsafe_tip").into());
                let s = state.lock().unwrap();
                sync_queue_to_ui(&app, &s.queue);
            }
        });
    }

    // Refresh Windows
    {
        let window_handle = window_handle.clone();
        main_window.on_refresh_windows(move || {
            if let Some(app) = window_handle.upgrade() {
                refresh_window_list(&app);
            }
        });
    }

    // Clear Log
    {
        let state = Arc::clone(&state);
        let window_handle = window_handle.clone();
        main_window.on_clear_log(move || {
            let mut s = state.lock().unwrap();
            s.log_lines.clear();
            if let Some(app) = window_handle.upgrade() {
                let empty_model = Rc::new(VecModel::default());
                app.set_log_lines(ModelRc::from(empty_model));
                app.set_log_count(0);
                app.set_log_preview(t("log_cleared").into());
            }
        });
    }

    // Save Queue
    {
        let state = Arc::clone(&state);
        let window_handle = window_handle.clone();

        main_window.on_save_queue(move || {
            let file = rfd::FileDialog::new()
                .add_filter("AutoClickTimer Profile", &["act"])
                .save_file();

            if let Some(path) = file {
                let s = state.lock().unwrap();
                if let Ok(json) = serde_json::to_string_pretty(&s.queue) {
                    if let Ok(()) = std::fs::write(&path, json) {
                        append_log(&window_handle, &state, &format!("Profil gespeichert: {}", path.display()));
                    }
                }
            }
        });
    }

    // Load Queue
    {
        let state = Arc::clone(&state);
        let window_handle = window_handle.clone();

        main_window.on_load_queue(move || {
            let file = rfd::FileDialog::new()
                .add_filter("AutoClickTimer Profile", &["act"])
                .pick_file();

            if let Some(path) = file {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(loaded) = serde_json::from_str::<Vec<Item>>(&content) {
                        let mut s = state.lock().unwrap();
                        s.queue = loaded;
                        if let Some(app) = window_handle.upgrade() {
                            sync_queue_to_ui(&app, &s.queue);
                        }
                        append_log(&window_handle, &state, &format!("Profil geladen: {}", path.display()));
                    }
                }
            }
        });
    }

    // Apply Update
    {
        let state = Arc::clone(&state);
        let window_handle = window_handle.clone();

        main_window.on_apply_update(move || {
            let info_opt = {
                let s = state.lock().unwrap();
                s.update_info.clone()
            };

            if let Some(info) = info_opt {
                let handle = window_handle.clone();
                let state_clone = Arc::clone(&state);

                thread::spawn(move || {
                    let _ = download_and_apply(&info, |msg| {
                        append_log(&handle, &state_clone, msg);
                    });
                });
            }
        });
    }

    // Background Update Check (after 3 seconds)
    {
        let window_handle = window_handle.clone();
        let state = Arc::clone(&state);

        thread::spawn(move || {
            thread::sleep(Duration::from_secs(3));
            if let Some(info) = check_for_update(REPO, CURRENT_VERSION) {
                {
                    let mut s = state.lock().unwrap();
                    s.update_info = Some(info.clone());
                }

                let handle = window_handle.clone();
                let state_clone = Arc::clone(&state);

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = handle.upgrade() {
                        app.set_update_tag(info.tag.clone().into());
                        app.set_update_available(true);
                        append_log(&handle, &state_clone, &format!("Neue Version verfügbar: {}", info.tag));
                    }
                });
            }
        });
    }

    main_window.run()
}

fn refresh_window_list(app: &AppWindow) {
    let mut windows = vec!["(Global / Aktives Fenster)".to_string()];
    windows.extend(get_open_windows());

    let slint_strings: Vec<slint::SharedString> = windows.into_iter().map(|s| s.into()).collect();
    let model = Rc::new(VecModel::from(slint_strings));
    app.set_window_list(ModelRc::from(model));
}

fn sync_queue_to_ui(app: &AppWindow, queue: &[Item]) {
    let items: Vec<QueueItemData> = queue
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let meta = match item.action {
                ActionType::Sleep => format!(
                    "{} - Sleep (Pre: {}s, Post: {}s)",
                    fmt_time(item.total),
                    item.sleep_cfg.pre_sleep_grace,
                    item.sleep_cfg.post_wake_delay
                ),
                ActionType::Type => {
                    let prompt_snippet = if item.prompt.len() > 30 {
                        format!("{}...", &item.prompt[..30])
                    } else {
                        item.prompt.clone()
                    };
                    format!("{} - Type: \"{}\"", fmt_time(item.total), prompt_snippet)
                }
                _ => format!("{} - {}", fmt_time(item.total), item.action.as_str()),
            };

            let (status_text, countdown, cd_color, progress) = match item.status {
                ItemStatus::Done => (
                    t("status_done"),
                    t("status_done").to_string(),
                    Color::from_rgb_u8(74, 222, 128), // Success green
                    1.0f32,
                ),
                ItemStatus::Waiting => (
                    t("status_waiting"),
                    fmt_time(item.total),
                    Color::from_rgb_u8(139, 139, 153), // Muted grey
                    0.0f32,
                ),
                ItemStatus::Running => {
                    let elapsed = item.phase_total.saturating_sub(item.rem);
                    let prog = if item.phase_total > 0 {
                        (elapsed as f32) / (item.phase_total as f32)
                    } else {
                        0.0
                    };

                    let st_text = match item.phase {
                        ItemPhase::Grace => t("status_grace"),
                        ItemPhase::Sleeping => t("status_sleeping"),
                        ItemPhase::PostWake => t("status_post_wake"),
                        ItemPhase::AwakeFallback => t("status_awake_fallback"),
                        _ => t("status_running"),
                    };

                    (
                        st_text,
                        fmt_time(item.rem),
                        Color::from_rgb_u8(72, 145, 161), // Primary teal
                        prog.clamp(0.0, 1.0),
                    )
                }
            };

            QueueItemData {
                id: idx as i32,
                label: item.label.clone().into(),
                meta: meta.into(),
                countdown: countdown.into(),
                status_text: status_text.into(),
                progress,
                status: item.status.as_str().into(),
                countdown_color: cd_color,
            }
        })
        .collect();

    let model = Rc::new(VecModel::from(items));
    app.set_queue_items(ModelRc::from(model));
}

fn handle_executor_event(
    app: &AppWindow,
    state: &Arc<Mutex<AppState>>,
    event: ExecutorEvent,
) {
    match event {
        ExecutorEvent::Tick {
            index,
            rem,
            phase,
            phase_total,
        } => {
            let mut s = state.lock().unwrap();
            if let Some(item) = s.queue.get_mut(index) {
                item.rem = rem;
                item.phase = phase;
                item.phase_total = phase_total;
            }
            sync_queue_to_ui(app, &s.queue);
        }
        ExecutorEvent::StepStart { index, .. } => {
            let mut s = state.lock().unwrap();
            if let Some(item) = s.queue.get_mut(index) {
                item.status = ItemStatus::Running;
            }
            app.set_status_text(format!("Schritt {} läuft...", index + 1).into());
            sync_queue_to_ui(app, &s.queue);
        }
        ExecutorEvent::StepDone { index } => {
            let mut s = state.lock().unwrap();
            if let Some(item) = s.queue.get_mut(index) {
                item.status = ItemStatus::Done;
                item.rem = 0;
            }
            sync_queue_to_ui(app, &s.queue);
        }
        ExecutorEvent::AllDone { total_items } => {
            app.set_is_running(false);
            app.set_status_text(format!("Alle {} Aktionen abgeschlossen!", total_items).into());
            let s = state.lock().unwrap();
            sync_queue_to_ui(app, &s.queue);
        }
        ExecutorEvent::Stopped => {
            app.set_is_running(false);
            app.set_status_text(t("stopped").into());
        }
        ExecutorEvent::Failsafe => {
            app.set_is_running(false);
            app.set_status_text(t("failsafe_status").into());
        }
        ExecutorEvent::Log(msg) => {
            let ts = Local::now().format("%H:%M:%S").to_string();
            let line = format!("[{}] {}", ts, msg);
            let mut s = state.lock().unwrap();
            s.log_lines.push(line.clone());

            let count = s.log_lines.len() as i32;
            app.set_log_count(count);
            app.set_log_preview(line.into());

            let slint_lines: Vec<slint::SharedString> = s
                .log_lines
                .iter()
                .map(|s| s.as_str().into())
                .collect();
            let model = Rc::new(VecModel::from(slint_lines));
            app.set_log_lines(ModelRc::from(model));
        }
    }
}

fn append_log(
    window_handle: &slint::Weak<AppWindow>,
    state: &Arc<Mutex<AppState>>,
    msg: &str,
) {
    let ts = Local::now().format("%H:%M:%S").to_string();
    let line = format!("[{}] {}", ts, msg);

    let mut s = state.lock().unwrap();
    s.log_lines.push(line.clone());

    if let Some(app) = window_handle.upgrade() {
        let count = s.log_lines.len() as i32;
        app.set_log_count(count);
        app.set_log_preview(line.into());

        let slint_lines: Vec<slint::SharedString> = s
            .log_lines
            .iter()
            .map(|s| s.as_str().into())
            .collect();
        let model = Rc::new(VecModel::from(slint_lines));
        app.set_log_lines(ModelRc::from(model));
    }
}
