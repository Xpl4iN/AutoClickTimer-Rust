//! Internationalization (i18n) module for AutoClickTimer.
//! Supports German (DE) and English (EN) runtime switching.
//! No em dashes and no emojis.

use std::sync::atomic::{AtomicU8, Ordering};

pub const LANG_DE: u8 = 0;
pub const LANG_EN: u8 = 1;

static CURRENT_LANG: AtomicU8 = AtomicU8::new(LANG_DE);

pub fn set_language(lang: &str) {
    if lang.eq_ignore_ascii_case("en") {
        CURRENT_LANG.store(LANG_EN, Ordering::SeqCst);
    } else {
        CURRENT_LANG.store(LANG_DE, Ordering::SeqCst);
    }
}

#[allow(dead_code)]
pub fn get_language_code() -> &'static str {
    if CURRENT_LANG.load(Ordering::SeqCst) == LANG_EN {
        "en"
    } else {
        "de"
    }
}

pub fn t(key: &'static str) -> &'static str {
    let is_en = CURRENT_LANG.load(Ordering::SeqCst) == LANG_EN;
    lookup(key, is_en)
}

fn lookup(key: &'static str, is_en: bool) -> &'static str {
    match (key, is_en) {
        // App Header
        ("app_title", false) => "AutoClick Timer",
        ("app_title", true) => "AutoClick Timer",
        ("failsafe_tip", false) => "Notfall-Stop: Maus ganz oben-links in die Bildschirmecke schieben",
        ("failsafe_tip", true) => "Emergency Stop: Move mouse to the top-left screen corner",
        ("caffeine", false) => "Caffeine",
        ("caffeine", true) => "Caffeine",
        ("update_available", false) => "Update verfügbar",
        ("update_available", true) => "Update available",
        ("update_downloading", false) => "Wird geladen...",
        ("update_downloading", true) => "Downloading...",

        // Tray
        ("tray_show", false) => "Anzeigen",
        ("tray_show", true) => "Show",
        ("tray_stop", false) => "Stop",
        ("tray_stop", true) => "Stop",
        ("tray_quit", false) => "Beenden",
        ("tray_quit", true) => "Exit",

        // Form Tabs & Modes
        ("tab_action", false) => "Aktion erstellen",
        ("tab_action", true) => "Create Action",
        ("tab_presets", false) => "Vorlagen",
        ("tab_presets", true) => "Presets",
        ("mode_label", false) => "Modus:",
        ("mode_label", true) => "Mode:",
        ("mode_duration", false) => "Timer (Dauer)",
        ("mode_duration", true) => "Timer (Duration)",
        ("mode_clock", false) => "Uhrzeit",
        ("mode_clock", true) => "Clock Time",
        ("hours", false) => "Stunden",
        ("hours", true) => "Hours",
        ("minutes", false) => "Minuten",
        ("minutes", true) => "Minutes",
        ("seconds", false) => "Sekunden",
        ("seconds", true) => "Seconds",
        ("hour_single", false) => "Stunde",
        ("hour_single", true) => "Hour",
        ("minute_single", false) => "Minute",
        ("minute_single", true) => "Minute",
        ("second_single", false) => "Sekunde",
        ("second_single", true) => "Second",

        // Actions
        ("action_type_label", false) => "Aktionstyp:",
        ("action_type_label", true) => "Action Type:",
        ("act_enter", false) => "Enter",
        ("act_enter", true) => "Enter",
        ("act_click", false) => "Linksklick",
        ("act_click", true) => "Left Click",
        ("act_type", false) => "Prompt senden",
        ("act_type", true) => "Send Prompt",
        ("act_sleep", false) => "Sleep & Wake",
        ("act_sleep", true) => "Sleep & Wake",
        ("act_shutdown", false) => "Herunterfahren",
        ("act_shutdown", true) => "Shut Down",

        // Prompt & Sleep
        ("prompt_label", false) => "Prompt-Text (eingefügt + Enter gesendet):",
        ("prompt_label", true) => "Prompt Text (pasted + Enter sent):",
        ("sleep_config_title", false) => "Sleep-Konfiguration",
        ("sleep_config_title", true) => "Sleep Configuration",
        ("sleep_grace_label", false) => "Wartezeit vor Schlaf:",
        ("sleep_grace_label", true) => "Grace Period before Sleep:",
        ("postwake_label", false) => "Post-Wake-Verzögerung (Sek.):",
        ("postwake_label", true) => "Post-Wake Delay (sec):",

        // Target Window & Label
        ("target_window_label", false) => "Ziel-Fenster (Background-Input):",
        ("target_window_label", true) => "Target Window (Background Input):",
        ("global_window", false) => "(Global / Aktives Fenster)",
        ("global_window", true) => "(Global / Active Window)",
        ("refresh", false) => "Aktualisieren",
        ("refresh", true) => "Refresh",
        ("require_foreground", false) => "Zwingend in den Vordergrund holen",
        ("require_foreground", true) => "Force bring to foreground",
        ("label_title", false) => "Bezeichnung:",
        ("label_title", true) => "Label:",
        ("add_to_queue", false) => "+ Zur Warteschlange",
        ("add_to_queue", true) => "+ Add to Queue",

        // Default labels
        ("default_sleep_label", false) => "Sleep & Wake",
        ("default_sleep_label", true) => "Sleep & Wake",
        ("default_shutdown_label", false) => "Herunterfahren",
        ("default_shutdown_label", true) => "Shut Down",
        ("post_wake_enter", false) => "Enter nach Aufwachen",
        ("post_wake_enter", true) => "Enter after Wake",
        ("post_wake_click", false) => "Linksklick nach Aufwachen",
        ("post_wake_click", true) => "Left Click after Wake",

        // Presets
        ("presets_header", false) => "SCHNELL-VORLAGEN",
        ("presets_header", true) => "QUICK PRESETS",
        ("p1_title", false) => "Sleep & Wake + Enter",
        ("p1_title", true) => "Sleep & Wake + Enter",
        ("p1_desc", false) => "Rechner in Ruhezustand versetzen und zur Zielzeit mit Enter wecken.",
        ("p1_desc", true) => "Put computer to sleep and wake with Enter key at target time.",
        ("p2_title", false) => "Sleep & Wake + Linksklick",
        ("p2_title", true) => "Sleep & Wake + Left Click",
        ("p2_desc", false) => "Rechner in Ruhezustand versetzen und zur Zielzeit mit Klick wecken.",
        ("p2_desc", true) => "Put computer to sleep and wake with mouse click at target time.",
        ("p3_title", false) => "Timer + Herunterfahren",
        ("p3_title", true) => "Timer + Shut Down",
        ("p3_desc", false) => "Rechner nach Ablauf der eingestellten Zeit vollständig herunterfahren.",
        ("p3_desc", true) => "Safely shut down computer after the specified time elapsed.",
        ("add_preset_btn", false) => "+ Hinzufügen",
        ("add_preset_btn", true) => "+ Add",
        ("p4_title", false) => "Timer + Koffein (Bildschirm wach)",
        ("p4_title", true) => "Timer + Caffeine (Keep Awake)",
        ("p4_desc", false) => "Verhindert, dass der Bildschirm sich ausschaltet oder der PC in den Ruhezustand wechselt.",
        ("p4_desc", true) => "Prevents screen from turning off or PC from sleeping for the set duration.",
        ("p5_title", false) => "Timer + Enter",
        ("p5_title", true) => "Timer + Enter",
        ("p5_desc", false) => "Wartet die eingestellte Zeit ab und drückt dann die Enter-Taste.",
        ("p5_desc", true) => "Waits the set duration, then presses the Enter key.",

        // Queue Panel
        ("queue_header", false) => "WARTESCHLANGE",
        ("queue_header", true) => "QUEUE",
        ("empty_queue", false) => "Noch keine Aktionen - füge deine erste links hinzu.",
        ("empty_queue", true) => "Noch keine Aktionen - füge deine erste links hinzu.",
        ("start_btn", false) => "Starten",
        ("start_btn", true) => "Start",
        ("start_later_btn", false) => "Später...",
        ("start_later_btn", true) => "Later...",
        ("stop_btn", false) => "Stop",
        ("stop_btn", true) => "Stop",
        ("reset_btn", false) => "Reset",
        ("reset_btn", true) => "Reset",
        ("clear_btn", false) => "Leeren",
        ("clear_btn", true) => "Clear",
        ("save_btn", false) => "Speichern",
        ("save_btn", true) => "Save",
        ("load_btn", false) => "Laden",
        ("load_btn", true) => "Load",

        // Status
        ("status_done", false) => "Fertig",
        ("status_done", true) => "Done",
        ("status_waiting", false) => "Wartet",
        ("status_waiting", true) => "Waiting",
        ("status_running", false) => "Läuft",
        ("status_running", true) => "Running",
        ("status_grace", false) => "Vorbereitung",
        ("status_grace", true) => "Preparing",
        ("status_sleeping", false) => "Schläft...",
        ("status_sleeping", true) => "Sleeping...",
        ("status_post_wake", false) => "Aufgewacht",
        ("status_post_wake", true) => "Awake",
        ("status_awake_fallback", false) => "Wach (Fallback)",
        ("status_awake_fallback", true) => "Awake (Fallback)",
        ("stopped", false) => "Gestoppt.",
        ("stopped", true) => "Stopped.",
        ("failsafe_status", false) => "Failsafe!",
        ("failsafe_status", true) => "Failsafe!",
        ("ready", false) => "Bereit. Keine Aktionen ausgeführt.",
        ("ready", true) => "Ready. No actions executed.",

        // Log Panel
        ("log_title", false) => "Log",
        ("log_title", true) => "Log",
        ("log_cleared", false) => "Log geleert.",
        ("log_cleared", true) => "Log cleared.",
        ("log_queue_started", false) => "Warteschlange gestartet.",
        ("log_queue_started", true) => "Queue started.",
        ("log_reset", false) => "Zurückgesetzt.",
        ("log_reset", true) => "Reset.",
        ("log_queue_cleared", false) => "Warteschlange geleert.",
        ("log_queue_cleared", true) => "Queue cleared.",
        ("log_caffeine_on", false) => "Caffeine Mode aktiviert (Anti-Lock).",
        ("log_caffeine_on", true) => "Caffeine Mode enabled (Anti-Lock).",
        ("log_caffeine_off", false) => "Caffeine Mode deaktiviert.",
        ("log_caffeine_off", true) => "Caffeine Mode disabled.",

        // Errors & Validation
        ("err_title", false) => "Fehler",
        ("err_title", true) => "Error",
        ("err_admin_title", false) => "Administrator",
        ("err_admin_title", true) => "Administrator",
        ("err_admin_msg", false) => "Sleep & Wake benötigt Administratorrechte. Bitte als Administrator neu starten.",
        ("err_admin_msg", true) => "Sleep & Wake requires administrator privileges. Please restart as administrator.",
        ("err_time_zero", false) => "Zeit > 0 erforderlich.",
        ("err_time_zero", true) => "Time > 0 required.",
        ("err_time_past", false) => "Zielzeit liegt in der Vergangenheit.",
        ("err_time_past", true) => "Target time is in the past.",
        ("err_time_invalid", false) => "Ungültige Zeit-Eingabe.",
        ("err_time_invalid", true) => "Invalid time input.",
        ("err_prompt_missing", false) => "Prompt-Text fehlt.",
        ("err_prompt_missing", true) => "Prompt text is missing.",

        _ => key,
    }
}

pub fn fmt_time(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

pub fn fmt_short(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    let mut parts = Vec::new();
    if h > 0 {
        parts.push(format!("{}h", h));
    }
    if m > 0 {
        parts.push(format!("{}m", m));
    }
    if s > 0 || parts.is_empty() {
        parts.push(format!("{}s", s));
    }
    parts.join(" ")
}

pub fn format_clock_preview(delta_secs: u64, target_time: &str) -> String {
    let is_en = CURRENT_LANG.load(Ordering::SeqCst) == LANG_EN;
    if is_en {
        format!("-> in {} (at {})", fmt_short(delta_secs), target_time)
    } else {
        format!("-> in {} (um {})", fmt_short(delta_secs), target_time)
    }
}

