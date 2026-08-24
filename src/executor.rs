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
    execute_with_foreground, find_window_by_title, post_click_to_hwnd, post_enter_to_hwnd,
    send_click_global, send_enter_global, send_text_to_hwnd, send_type_global,
};
use crate::platform::windows::power::{execute_sleep_with_retry, set_caffeine, shutdown_pc};

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
                handle_sleep_step(i, item, &stop_flag, &snapshot, &event_sink)
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
                    dispatch_with_retry(item, &event_sink);
                }
                done
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

    if stop_flag.load(Ordering::SeqCst) {
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
        snapshot,
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
            snapshot,
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
            snapshot,
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
                ActionType::Sleep    => return Ok(()),
                ActionType::Shutdown => { shutdown_pc(); return Ok(()); }
                ActionType::Caffeine => return Ok(()), // handled in run_worker
            }
        }
    }

    // Global execution
    match item.action {
        ActionType::Enter    => send_enter_global(),
        ActionType::Click    => send_click_global(),
        ActionType::Type     => send_type_global(&item.prompt)?,
        ActionType::Sleep    => {}
        ActionType::Shutdown => shutdown_pc(),
        ActionType::Caffeine => {} // handled separately in run_worker
    }

    Ok(())
}
