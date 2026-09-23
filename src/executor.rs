//! Background Queue Executor for AutoClickTimer.
//! Runs queue items sequentially on a dedicated worker thread with monotonic timing,
//! action retries, failsafe monitoring, and thread-safe callbacks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};

use crate::i18n::fmt_time;
use crate::models::{ActionType, Item, ItemPhase, ItemStatus, QueueItemSummary, QueueSnapshot};
use crate::platform::windows::failsafe::is_failsafe_triggered;
use crate::platform::windows::input::{
    execute_with_foreground, find_window_by_title, is_input_desktop_unlocked,
    post_click_to_hwnd, post_enter_to_hwnd, send_click_global, send_enter_global,
    send_text_to_hwnd, send_type_global,
};
use crate::platform::windows::power::{
    configure_passwordless_wake, execute_sleep_with_retry, set_caffeine, shutdown_pc,
};
use crate::platform::windows::remote_mode;

const MAX_ACTION_RETRIES: u32 = 3;
const ACTION_RETRY_DELAY: Duration = Duration::from_millis(1000);

#[derive(Debug, Clone)]
pub enum ExecutorEvent {
    Tick {
        index: usize,
        rem: u64,
        phase: ItemPhase,
        phase_total: u64,
    },
    #[allow(dead_code)]
    StepStart {
        index: usize,
        total_items: usize,
        label: String,
        duration_secs: u64,
    },
    StepDone {
        index: usize,
    },
    IterationStart {
        current_iteration: u32,
        total_iterations: u32,
    },
    AllDone {
        total_items: usize,
    },
    Stopped,
    Failed(String),
    Failsafe,
    Log(String),
}

pub struct QueueExecutor {
    stop_flag: Arc<AtomicBool>,
    snapshot: Arc<Mutex<QueueSnapshot>>,
    worker_handle: Option<JoinHandle<()>>,
}

impl QueueExecutor {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
            snapshot: Arc::new(Mutex::new(QueueSnapshot::default())),
            worker_handle: None,
        }
    }

    pub fn is_running(&self) -> bool {
        if let Some(handle) = &self.worker_handle {
            !handle.is_finished()
        } else {
            false
        }
    }

    pub fn get_snapshot(&self) -> QueueSnapshot {
        self.snapshot.lock().unwrap().clone()
    }

    pub fn start<F>(
        &mut self,
        queue: Vec<Item>,
        start_at: Option<DateTime<Local>>,
        repeat_count: u32,
        event_sink: F,
    ) where
        F: Fn(ExecutorEvent) + Send + Sync + 'static,
    {
        if self.is_running() {
            return;
        }

        self.stop_flag.store(false, Ordering::SeqCst);
        let stop_flag = Arc::clone(&self.stop_flag);
        let snapshot = Arc::clone(&self.snapshot);

        let handle = thread::spawn(move || {
            run_worker(queue, start_at, repeat_count, stop_flag, snapshot, event_sink);
        });

        self.worker_handle = Some(handle);
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Ok(mut snap) = self.snapshot.lock() {
            snap.is_running = false;
            snap.status = "stopped".to_string();
        }
    }
}

fn run_worker<F>(
    mut queue: Vec<Item>,
    start_at: Option<DateTime<Local>>,
    repeat_count: u32,
    stop_flag: Arc<AtomicBool>,
    snapshot: Arc<Mutex<QueueSnapshot>>,
    event_sink: F,
) where
    F: Fn(ExecutorEvent) + Send + Sync + 'static,
{
    let total_items = queue.len();
    let is_infinite = repeat_count == 0;
    let max_iterations = if is_infinite { u32::MAX } else { repeat_count.max(1) };

    // Initialize snapshot
    {
        let items_summary: Vec<QueueItemSummary> = queue
            .iter()
            .enumerate()
            .map(|(idx, it)| QueueItemSummary {
                index: idx,
                label: it.label.clone(),
                action: it.action.as_str().to_string(),
                total_seconds: it.total,
                target_window: it.target_window.clone(),
                status: it.status.as_str().to_string(),
            })
            .collect();

        if let Ok(mut snap) = snapshot.lock() {
            snap.is_running = true;
            snap.status = if start_at.is_some() { "scheduled".to_string() } else { "running".to_string() };
            snap.total_items = total_items;
            snap.current_index = 0;
            snap.current_iteration = 1;
            snap.total_iterations = repeat_count;
            snap.items = items_summary;
            if let Some(first) = queue.first() {
                snap.current_action = first.action.as_str().to_string();
                snap.current_label = first.label.clone();
                snap.target_window = first.target_window.clone();
                snap.remaining_seconds = first.total;
                snap.phase = String::new();
                snap.phase_total = first.total;
            }
        }
    }

    // Wait for scheduled start time if provided
    if let Some(target_time) = start_at {
        while Local::now() < target_time {
            if stop_flag.load(Ordering::SeqCst) {
                if let Ok(mut snap) = snapshot.lock() {
                    snap.is_running = false;
                    snap.status = "stopped".to_string();
                }
                event_sink(ExecutorEvent::Stopped);
                return;
            }
            if is_failsafe_triggered() {
                if let Ok(mut snap) = snapshot.lock() {
                    snap.is_running = false;
                    snap.status = "failsafe".to_string();
                }
                event_sink(ExecutorEvent::Log(
                    "WARNUNG Failsafe ausgeloest -- Abbruch!".to_string(),
                ));
                event_sink(ExecutorEvent::Failsafe);
                return;
            }
            thread::sleep(Duration::from_millis(300));
        }
        if let Ok(mut snap) = snapshot.lock() {
            snap.status = "running".to_string();
        }
    }

    let mut iteration: u32 = 1;
    let mut failure_reason: Option<String> = None;
    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        if !is_infinite && iteration > max_iterations {
            break;
        }

        {
            if let Ok(mut snap) = snapshot.lock() {
                snap.current_iteration = iteration;
            }
        }

        event_sink(ExecutorEvent::IterationStart {
            current_iteration: iteration,
            total_iterations: repeat_count,
        });

        if max_iterations > 1 || is_infinite {
            let iter_label = if is_infinite {
                format!("--- Durchlauf {} (Endlosschleife) ---", iteration)
            } else {
                format!("--- Durchlauf {}/{} ---", iteration, max_iterations)
            };
            event_sink(ExecutorEvent::Log(iter_label));
        }

        let mut iteration_completed_cleanly = true;

        for i in 0..total_items {
            if stop_flag.load(Ordering::SeqCst) {
                iteration_completed_cleanly = false;
                break;
            }

            let item = &mut queue[i];
            item.status = ItemStatus::Running;

            {
                if let Ok(mut snap) = snapshot.lock() {
                    snap.current_index = i;
                    snap.current_action = item.action.as_str().to_string();
                    snap.current_label = item.label.clone();
                    snap.target_window = item.target_window.clone();
                    snap.remaining_seconds = item.total;
                    snap.phase = String::new();
                    snap.phase_total = item.total;
                    if i < snap.items.len() {
                        snap.items[i].status = "running".to_string();
                    }
                }
            }

            event_sink(ExecutorEvent::StepStart {
                index: i,
                total_items,
                label: item.label.clone(),
                duration_secs: item.total,
            });

            event_sink(ExecutorEvent::Log(format!(
                "Schritt {}/{}: [{}] -- {}",
                i + 1,
                total_items,
                item.label,
                fmt_time(item.total)
            )));

            let completed = if item.action == ActionType::Sleep {
                match handle_sleep_step(i, item, &stop_flag, &snapshot, &event_sink) {
                    Ok(completed) => completed,
                    Err(error) => {
                        failure_reason = Some(error);
                        false
                    }
                }
            } else if item.action == ActionType::Caffeine {
                // Enable caffeine for the full duration, then disable
                set_caffeine(true);
                event_sink(ExecutorEvent::Log("  -> Caffeine aktiv: Bildschirm bleibt an.".to_string()));
                let done = countdown(i, item.total, item.total, ItemPhase::None, &stop_flag, &snapshot, &event_sink);
                set_caffeine(false);
                event_sink(ExecutorEvent::Log("  -> Caffeine beendet.".to_string()));
                done
            } else {
                let done = countdown(i, item.total, item.total, ItemPhase::None, &stop_flag, &snapshot, &event_sink);
                if done {
                    match dispatch_with_retry(item, &event_sink) {
                        Ok(()) => true,
                        Err(error) => {
                            failure_reason = Some(error);
                            false
                        }
                    }
                } else {
                    false
                }
            };

            if !completed || stop_flag.load(Ordering::SeqCst) {
                iteration_completed_cleanly = false;
                break;
            }

            item.rem = 0;
            item.status = ItemStatus::Done;
            {
                if let Ok(mut snap) = snapshot.lock() {
                    if i < snap.items.len() {
                        snap.items[i].status = "done".to_string();
                    }
                }
            }
            event_sink(ExecutorEvent::StepDone { index: i });
            event_sink(ExecutorEvent::Log(format!("Schritt {} abgeschlossen.", i + 1)));
        }

        if !iteration_completed_cleanly || stop_flag.load(Ordering::SeqCst) {
            break;
        }

        iteration += 1;
        if is_infinite || iteration <= max_iterations {
            for item in queue.iter_mut() {
                item.reset();
            }
            if let Ok(mut snap) = snapshot.lock() {
                for it in snap.items.iter_mut() {
                    it.status = "waiting".to_string();
                }
            }
            thread::sleep(Duration::from_millis(150));
        }
    }

    if let Some(error) = failure_reason {
        if let Ok(mut snap) = snapshot.lock() {
            snap.is_running = false;
            snap.status = "failed".to_string();
            snap.phase = String::new();
            snap.remaining_seconds = 0;
            let failed_index = snap.current_index;
            if let Some(item) = snap.items.get_mut(failed_index) {
                item.status = "failed".to_string();
            }
        }
        event_sink(ExecutorEvent::Failed(error));
    } else if stop_flag.load(Ordering::SeqCst) {
        if let Ok(mut snap) = snapshot.lock() {
            snap.is_running = false;
            snap.status = "stopped".to_string();
        }
        event_sink(ExecutorEvent::Stopped);
    } else {
        if let Ok(mut snap) = snapshot.lock() {
            snap.is_running = false;
            snap.status = "done".to_string();
            snap.remaining_seconds = 0;
            snap.phase = String::new();
        }
        event_sink(ExecutorEvent::AllDone { total_items });
    }
}


fn handle_sleep_step<F>(
    index: usize,
    item: &Item,
    stop_flag: &Arc<AtomicBool>,
    snapshot: &Arc<Mutex<QueueSnapshot>>,
    event_sink: &F,
) -> Result<bool, String>

where
    F: Fn(ExecutorEvent) + Send + Sync + 'static,
{
    let cfg = &item.sleep_cfg;

    if item.total == 0 {
        return Err("Sleep & Wake needs a duration greater than zero".to_string());
    }

    // Phase 1: Pre-sleep grace countdown
    event_sink(ExecutorEvent::Log(format!(
        "  -> Vorbereitung: {}s Wartezeit bevor PC in Ruhezustand geht.",
        cfg.pre_sleep_grace
    )));

    if !countdown(
        index,
        cfg.pre_sleep_grace,
        cfg.pre_sleep_grace,
        ItemPhase::Grace,
        stop_flag,
        snapshot,
        event_sink,
    ) {
        return Ok(false);
    }

    if stop_flag.load(Ordering::SeqCst) {
        return Ok(false);
    }

    // An explicit sleep request wins over persistent remote mode and does not
    // silently re-enable it after wake.
    match remote_mode::disable_before_suspend() {
        Ok(true) => event_sink(ExecutorEvent::Log("  -> Remote mode disabled; original power settings restored before sleep.".to_string())),
        Ok(false) => {}
        Err(e) => {
            event_sink(ExecutorEvent::Log(format!("  ERROR Remote mode could not be restored before sleep: {e}")));
            return Err(format!("Remote Mode restore failed before sleep: {e}"));
        }
    }

    // Remote mode restoration can reactivate the power scheme. Configure and
    // verify its wake policy only after that restoration, before suspending.
    configure_passwordless_wake().map_err(|error| {
        format!(
            "Sleep & Wake stopped before suspend: Windows could not be configured to resume without sign-in: {error}"
        )
    })?;
    event_sink(ExecutorEvent::Log(
        "  -> Passwordless wake configured and verified for the current power scheme.".to_string(),
    ));

    if stop_flag.load(Ordering::SeqCst) {
        return Ok(false);
    }

    // Phase 2: Suspend
    event_sink(ExecutorEvent::Tick {
        index,
        rem: item.total,
        phase: ItemPhase::Sleeping,
        phase_total: item.total,
    });

    event_sink(ExecutorEvent::Log(format!(
        "  -> Ruhezustand wird eingeleitet fuer {}...",
        fmt_time(item.total)
    )));

    let slept = execute_sleep_with_retry(item.total, |msg| {
        event_sink(ExecutorEvent::Log(msg.to_string()));
    }).map_err(|error| format!("Sleep & Wake stopped: {error}"))?;

    if slept {
        // Phase 3: Post-wake delay
        event_sink(ExecutorEvent::Log(format!(
            "  -> Aufgewacht. Post-Wake Verzoegerung: {}s",
            cfg.post_wake_delay
        )));
        let completed = countdown(
            index,
            cfg.post_wake_delay,
            cfg.post_wake_delay,
            ItemPhase::PostWake,
            stop_flag,
            snapshot,
            event_sink,
        );
        if completed {
            // Windows may need a moment to switch from its wake screen to the
            // user's desktop, especially when post_wake_delay is set to zero.
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    return Ok(false);
                }
                match is_input_desktop_unlocked() {
                    Ok(true) => break,
                    Ok(false) if Instant::now() < deadline => {}
                    Ok(false) => return Err(
                        "Windows is still showing a lock or secure screen after wake; queued input was stopped."
                            .to_string(),
                    ),
                    Err(_) if Instant::now() < deadline => {}
                    Err(error) => return Err(format!(
                        "Sleep & Wake cannot verify the unlocked desktop after wake: {error}"
                    )),
                }
                thread::sleep(Duration::from_millis(250));
            }
        }
        Ok(completed)
    } else {
        // Fallback: stay awake and count down full sleep duration
        event_sink(ExecutorEvent::Log(format!(
            "  -> Ruhezustand fehlgeschlagen. Bleibe wach und zaehle {} herunter.",
            fmt_time(item.total)
        )));
        Ok(countdown(
            index,
            item.total,
            item.total,
            ItemPhase::AwakeFallback,
            stop_flag,
            snapshot,
            event_sink,
        ))
    }
}

fn countdown<F>(
    index: usize,
    duration_secs: u64,
    phase_total: u64,
    phase: ItemPhase,
    stop_flag: &Arc<AtomicBool>,
    snapshot: &Arc<Mutex<QueueSnapshot>>,
    event_sink: &F,
) -> bool
where
    F: Fn(ExecutorEvent) + Send + Sync + 'static,
{
    let t0 = Instant::now();
    let total_duration = Duration::from_secs(duration_secs);

    while !stop_flag.load(Ordering::SeqCst) {
        if is_failsafe_triggered() {
            if let Ok(mut snap) = snapshot.lock() {
                snap.is_running = false;
                snap.status = "failsafe".to_string();
            }
            event_sink(ExecutorEvent::Log(
                "WARNUNG Failsafe ausgeloest -- Abbruch!".to_string(),
            ));
            event_sink(ExecutorEvent::Failsafe);
            stop_flag.store(true, Ordering::SeqCst);
            return false;
        }

        let elapsed = t0.elapsed();
        let rem = if elapsed >= total_duration {
            0
        } else {
            duration_secs - elapsed.as_secs()
        };

        {
            if let Ok(mut snap) = snapshot.lock() {
                snap.current_index = index;
                snap.remaining_seconds = rem;
                snap.phase = phase.as_str().to_string();
                snap.phase_total = phase_total;
            }
        }

        event_sink(ExecutorEvent::Tick {
            index,
            rem,
            phase,
            phase_total,
        });

        if elapsed >= total_duration {
            return true;
        }

        thread::sleep(Duration::from_millis(250));
    }

    false
}

enum DispatchFailure {
    Retryable(String),
    Terminal(String),
}

fn dispatch_with_retry<F>(item: &Item, event_sink: &F) -> Result<(), String>
where
    F: Fn(ExecutorEvent) + Send + Sync + 'static,
{
    let mut last_error = String::new();
    for attempt in 1..=MAX_ACTION_RETRIES {
        match dispatch_single_action(item) {
            Ok(()) => {
                event_sink(ExecutorEvent::Log(format!(
                    "  -> Aktion '{}' ausgefuehrt.",
                    item.action.as_str()
                )));
                return Ok(());
            }
            Err(DispatchFailure::Terminal(error)) => {
                return Err(format!("Action '{}' failed: {error}", item.label));
            }
            Err(DispatchFailure::Retryable(error)) => {
                event_sink(ExecutorEvent::Log(format!(
                    "  WARNUNG Aktions-Versuch {}/{} fehlgeschlagen: {}",
                    attempt, MAX_ACTION_RETRIES, error
                )));
                last_error = error;
                if attempt < MAX_ACTION_RETRIES {
                    thread::sleep(ACTION_RETRY_DELAY);
                }
            }
        }
    }

    Err(format!(
        "Action '{}' failed after {} attempts: {last_error}",
        item.label, MAX_ACTION_RETRIES
    ))
}

fn dispatch_single_action(item: &Item) -> Result<(), DispatchFailure> {
    thread::sleep(Duration::from_millis(200));

    if item.action == ActionType::Shutdown {
        return shutdown_pc().map_err(DispatchFailure::Terminal);
    }
    if item.action == ActionType::Sleep || item.action == ActionType::Caffeine {
        return Ok(());
    }

    let target_hwnd = if !item.target_window.is_empty() {
        Some(find_window_by_title(&item.target_window).ok_or_else(|| {
            DispatchFailure::Retryable(format!(
                "Target window '{}' was not found",
                item.target_window
            ))
        })?)
    } else {
        None
    };

    if let Some(hwnd) = target_hwnd {
        if item.require_foreground {
            return execute_with_foreground(hwnd, &item.action, &item.prompt)
                .map_err(DispatchFailure::Terminal);
        } else {
            // Background dispatch without focus stealing
            return match item.action {
                ActionType::Enter => post_enter_to_hwnd(hwnd),
                ActionType::Click => post_click_to_hwnd(hwnd),
                ActionType::Type => send_text_to_hwnd(hwnd, &item.prompt),
                _ => Ok(()),
            }
            .map_err(DispatchFailure::Terminal);
        }
    }

    // Global execution
    match item.action {
        ActionType::Enter    => send_enter_global(),
        ActionType::Click    => {
            let btn = item.click_btn.as_deref().unwrap_or("left");
            if let (Some(x), Some(y)) = (item.click_x, item.click_y) {
                crate::platform::windows::input::send_click_at(x, y, btn)
            } else {
                match btn.to_lowercase().as_str() {
                    "right" => crate::platform::windows::input::send_right_click_global(),
                    "double" => crate::platform::windows::input::send_double_click_global(),
                    "middle" => crate::platform::windows::input::send_middle_click_global(),
                    _ => send_click_global(),
                }
            }
        }
        ActionType::Type     => send_type_global(&item.prompt),
        ActionType::Sleep | ActionType::Shutdown | ActionType::Caffeine => Ok(()),
    }
    .map_err(DispatchFailure::Terminal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_target_fails_queue_without_sending_global_input() {
        let mut item = Item::new(0, ActionType::Enter);
        item.label = "targeted Enter".to_string();
        item.target_window = format!("AutoClickTimer_missing_window_{}", std::process::id());
        let snapshot = Arc::new(Mutex::new(QueueSnapshot::default()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let collected = Arc::clone(&events);

        run_worker(
            vec![item],
            None,
            1,
            Arc::new(AtomicBool::new(false)),
            Arc::clone(&snapshot),
            move |event| collected.lock().unwrap().push(event),
        );

        let result = snapshot.lock().unwrap();
        assert_eq!(result.status, "failed");
        assert_eq!(result.items[0].status, "failed");
        let events = events.lock().unwrap();
        assert!(events.iter().any(|event| matches!(event, ExecutorEvent::Failed(_))));
        assert!(!events.iter().any(|event| matches!(event, ExecutorEvent::AllDone { .. })));
    }
}
