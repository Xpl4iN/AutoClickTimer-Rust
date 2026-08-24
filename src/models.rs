//! Data models for AutoClickTimer.
//! Pure data structures and serialization/deserialization for `.act` profiles.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionType {
    Enter,
    Click,
    Type,
    Sleep,
    Shutdown,
    Caffeine,
}

impl ActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionType::Enter    => "enter",
            ActionType::Click    => "click",
            ActionType::Type     => "type",
            ActionType::Sleep    => "sleep",
            ActionType::Shutdown => "shutdown",
            ActionType::Caffeine => "caffeine",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "enter"    => ActionType::Enter,
            "click"    => ActionType::Click,
            "type"     => ActionType::Type,
            "sleep"    => ActionType::Sleep,
            "shutdown" => ActionType::Shutdown,
            "caffeine" => ActionType::Caffeine,
            _          => ActionType::Enter,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SleepConfig {
    /// Seconds to wait before issuing suspend.
    #[serde(default = "default_pre_sleep_grace")]
    pub pre_sleep_grace: u64,

    /// Seconds to wait after wake before the next queue item begins.
    #[serde(default = "default_post_wake_delay")]
    pub post_wake_delay: u64,
}

fn default_pre_sleep_grace() -> u64 {
    5
}

fn default_post_wake_delay() -> u64 {
    30
}

impl Default for SleepConfig {
    fn default() -> Self {
        Self {
            pre_sleep_grace: default_pre_sleep_grace(),
            post_wake_delay: default_post_wake_delay(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub total: u64,
    pub action: ActionType,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub sleep_cfg: SleepConfig,
    #[serde(default)]
    pub target_window: String,
    #[serde(default)]
    pub require_foreground: bool,

    // Runtime state (skipped during serialization)
    #[serde(skip)]
    pub status: ItemStatus,
    #[serde(skip)]
    pub rem: u64,
    #[serde(skip)]
    pub phase: ItemPhase,
    #[serde(skip)]
    pub phase_total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemStatus {
    #[default]
    Waiting,
    Running,
    Done,
}

impl ItemStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemStatus::Waiting => "waiting",
            ItemStatus::Running => "running",
            ItemStatus::Done => "done",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemPhase {
    #[default]
    None,
    Grace,
    Sleeping,
    PostWake,
    AwakeFallback,
}

impl ItemPhase {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemPhase::None => "",
            ItemPhase::Grace => "grace",
            ItemPhase::Sleeping => "sleeping",
            ItemPhase::PostWake => "post_wake",
            ItemPhase::AwakeFallback => "awake_fallback",
        }
    }
}

impl Item {
    pub fn new(total: u64, action: ActionType) -> Self {
        Self {
            total,
            action,
            prompt: String::new(),
            label: String::new(),
            sleep_cfg: SleepConfig::default(),
            target_window: String::new(),
            require_foreground: false,
            status: ItemStatus::Waiting,
            rem: total,
            phase: ItemPhase::None,
            phase_total: total,
        }
    }

    pub fn reset(&mut self) {
        self.status = ItemStatus::Waiting;
        self.rem = self.total;
        self.phase = ItemPhase::None;
        self.phase_total = self.total;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueItemSummary {
    pub index: usize,
    pub label: String,
    pub action: String,
    pub total_seconds: u64,
    pub target_window: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueSnapshot {
    pub is_running: bool,
    pub status: String,
    pub current_index: usize,
    pub total_items: usize,
    pub current_action: String,
    pub current_label: String,
    pub target_window: String,
    pub remaining_seconds: u64,
    pub phase: String,
    pub phase_total: u64,
    pub items: Vec<QueueItemSummary>,
}

impl Default for QueueSnapshot {
    fn default() -> Self {
        Self {
            is_running: false,
            status: "idle".to_string(),
            current_index: 0,
            total_items: 0,
            current_action: String::new(),
            current_label: String::new(),
            target_window: String::new(),
            remaining_seconds: 0,
            phase: String::new(),
            phase_total: 0,
            items: Vec::new(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization_compatibility() {
        let item = Item {
            total: 300,
            action: ActionType::Sleep,
            prompt: "hello".to_string(),
            label: "Ruhezustand".to_string(),
            sleep_cfg: SleepConfig {
                pre_sleep_grace: 10,
                post_wake_delay: 20,
            },
            target_window: "Notepad".to_string(),
            require_foreground: true,
            status: ItemStatus::Running,
            rem: 150,
            phase: ItemPhase::Grace,
            phase_total: 10,
        };

        let json = serde_json::to_string_pretty(&vec![&item]).unwrap();
        assert!(json.contains("\"total\": 300"));
        assert!(json.contains("\"action\": \"sleep\""));
        assert!(json.contains("\"pre_sleep_grace\": 10"));

        let loaded: Vec<Item> = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].total, 300);
        assert_eq!(loaded[0].action, ActionType::Sleep);
        assert_eq!(loaded[0].rem, 0); // skip initializes to 0/default
    }
}
