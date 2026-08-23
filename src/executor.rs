//! Background Queue Executor for AutoClickTimer.
//! Runs queue items sequentially on a dedicated worker thread with monotonic timing,
//! action retries, failsafe monitoring, and thread-safe callbacks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};

use crate::i18n::fmt_time;
use crate::models::{ActionType, Item, ItemPhase, ItemStatus};
use crate::platform::windows::failsafe::is_failsafe_triggered;
use crate::platform::windows::input::{
    execute_with_foreground, find_window_by_title, post_click_to_hwnd, post_enter_to_hwnd,
    send_click_global, send_enter_global, send_text_to_hwnd, send_type_global,
};
use crate::platform::windows::power::{execute_sleep_with_retry, shutdown_pc};

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
    AllDone {
        total_items: usize,
    },
    Stopped,
    Failsafe,
    Log(String),
}

pub struct QueueExecutor {
    stop_flag: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
}

impl QueueExecutor {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
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

    pub fn start<F>(
        &mut self,
        queue: Vec<Item>,
        start_at: Option<DateTime<Local>>,
        event_sink: F,
    ) where
        F: Fn(ExecutorEvent) + Send + Sync + 'static,
    {
        if self.is_running() {
            return;
        }

        self.stop_flag.store(false, Ordering::SeqCst);
        let stop_flag = Arc::clone(&self.stop_flag);

        let handle = thread::spawn(move || {
            run_worker(queue, start_at, stop_flag, event_sink);
        });

        self.worker_handle = Some(handle);
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

fn run_worker<F>(
    mut queue: Vec<Item>,
    start_at: Option<DateTime<Local>>,
    stop_flag: Arc<AtomicBool>,
    event_sink: F,
) where
    F: Fn(ExecutorEvent) + Send + Sync + 'static,
{
    // Wait for scheduled start time if provided
    if let Some(target_time) = start_at {
        while Local::now() < target_time {
            if stop_flag.load(Ordering::SeqCst) {
                event_sink(ExecutorEvent::Stopped);
                return;
            }
            if is_failsafe_triggered() {
                event_sink(ExecutorEvent::Log(
                    "WARNUNG Failsafe ausgeloest -- Abbruch!".to_string(),
                ));
                event_sink(ExecutorEvent::Failsafe);
                return;
            }
            thread::sleep(Duration::from_millis(300));
        }
    }

    let total_items = queue.len();

    for i in 0..total_items {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        let item = &mut queue[i];
        item.status = ItemStatus::Running;

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
            handle_sleep_step(i, item, &stop_flag, &event_sink)
        } else {
            let done = countdown(i, item.total, item.total, ItemPhase::None, &stop_flag, &event_sink);
            if done {
                dispatch_with_retry(item, &event_sink);
            }
            done
        };

        if !completed || stop_flag.load(Ordering::SeqCst) {
            break;
        }

        item.rem = 0;
        item.status = ItemStatus::Done;
        event_sink(ExecutorEvent::StepDone { index: i });
        event_sink(ExecutorEvent::Log(format!("Schritt {} abgeschlossen.", i + 1)));
    }

    if stop_flag.load(Ordering::SeqCst) {
        event_sink(ExecutorEvent::Stopped);
    } else {
        event_sink(ExecutorEvent::AllDone { total_items });
    }
}

fn handle_sleep_step<F>(
    index: usize,
    item: &Item,
    stop_flag: &Arc<AtomicBool>,
    event_sink: &F,
) -> bool
where
    F: Fn(ExecutorEvent) + Send + Sync + 'static,
{
    let cfg = &item.sleep_cfg;

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
        event_sink,
    ) {
        return false;
    }

    if stop_flag.load(Ordering::SeqCst) {
        return false;
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
    });

    if slept {
        // Phase 3: Post-wake delay
        event_sink(ExecutorEvent::Log(format!(
            "  -> Aufgewacht. Post-Wake Verzoegerung: {}s",
            cfg.post_wake_delay
        )));
        countdown(
            index,
            cfg.post_wake_delay,
            cfg.post_wake_delay,
            ItemPhase::PostWake,
            stop_flag,
            event_sink,
        )
    } else {
        // Fallback: stay awake and count down full sleep duration
        event_sink(ExecutorEvent::Log(format!(
            "  -> Ruhezustand fehlgeschlagen. Bleibe wach und zaehle {} herunter.",
            fmt_time(item.total)
        )));
        countdown(
            index,
            item.total,
            item.total,
            ItemPhase::AwakeFallback,
            stop_flag,
            event_sink,
        )
    }
}

fn countdown<F>(
    index: usize,
    duration_secs: u64,
    phase_total: u64,
    phase: ItemPhase,
    stop_flag: &Arc<AtomicBool>,
    event_sink: &F,
) -> bool
where
    F: Fn(ExecutorEvent) + Send + Sync + 'static,
{
    let t0 = Instant::now();
    let total_duration = Duration::from_secs(duration_secs);

    while !stop_flag.load(Ordering::SeqCst) {
        if is_failsafe_triggered() {
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

fn dispatch_with_retry<F>(item: &Item, event_sink: &F)
where
    F: Fn(ExecutorEvent) + Send + Sync + 'static,
{
    for attempt in 1..=MAX_ACTION_RETRIES {
        match dispatch_single_action(item) {
            Ok(()) => {
                event_sink(ExecutorEvent::Log(format!(
                    "  -> Aktion '{}' ausgefuehrt.",
                    item.action.as_str()
                )));
                return;
            }
            Err(e) => {
                event_sink(ExecutorEvent::Log(format!(
                    "  WARNUNG Aktions-Versuch {}/{} fehlgeschlagen: {}",
                    attempt, MAX_ACTION_RETRIES, e
                )));
                if attempt < MAX_ACTION_RETRIES {
                    thread::sleep(ACTION_RETRY_DELAY);
                }
            }
        }
    }

    event_sink(ExecutorEvent::Log(format!(
        "  FEHLER Aktion nach {} Versuchen nicht ausfuehrbar -- uebersprungen.",
        MAX_ACTION_RETRIES
    )));
}

fn dispatch_single_action(item: &Item) -> Result<(), String> {
    thread::sleep(Duration::from_millis(200));

    let target_hwnd = if !item.target_window.is_empty() {
        find_window_by_title(&item.target_window)
    } else {
        None
    };

    if let Some(hwnd) = target_hwnd {
        if item.require_foreground {
            return execute_with_foreground(hwnd, &item.action, &item.prompt);
        } else {
            // Background dispatch without focus stealing
            match item.action {
                ActionType::Enter => {
                    post_enter_to_hwnd(hwnd);
                    return Ok(());
                }
                ActionType::Click => {
                    post_click_to_hwnd(hwnd);
                    return Ok(());
                }
                ActionType::Type => {
                    send_text_to_hwnd(hwnd, &item.prompt);
                    return Ok(());
                }
                ActionType::Sleep => return Ok(()),
                ActionType::Shutdown => {
                    shutdown_pc();
                    return Ok(());
                }
            }
        }
    }

    // Global execution
    match item.action {
        ActionType::Enter => send_enter_global(),
        ActionType::Click => send_click_global(),
        ActionType::Type => send_type_global(&item.prompt)?,
        ActionType::Sleep => {}
        ActionType::Shutdown => shutdown_pc(),
    }

    Ok(())
}
