//! Persistent, process-safe Windows power configuration for unattended remote work.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::System::Power::{
    PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex, PowerSetActiveScheme,
    PowerWriteACValueIndex, PowerWriteDCValueIndex,
};
use windows::Win32::System::Registry::HKEY;
use windows::core::GUID;

const SUB_SLEEP: &str = "238c9fa8-0aad-41ed-83f4-97be242c8f20";
const SUB_NONE: &str = "fea3413e-7e05-4911-9a71-700331f1c294";
const CONSOLELOCK: &str = "0e796bdb-100d-47d6-a2d5-f7d2daa51f51";
const STANDBY: &str = "29f6c1db-86da-48c5-9fdb-f2b67b1f44da";
const HIBERNATE: &str = "9d7815a6-7ee4-497e-8888-515a05f02364";
const SUB_BUTTONS: &str = "4f971e89-eebd-4455-a8de-9e59040e7347";
const LID: &str = "5ca83367-6e45-459f-a27b-476b1d01c936";
const SUB_VIDEO: &str = "7516b95f-f776-4464-8c53-06167f40cc99";
const DISPLAY: &str = "3c0bc021-c8a8-4e07-a973-6b14cbcb2b7e";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Values {
    ac: u32,
    dc: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    scheme: String,
    standby: Values,
    hibernate: Values,
    lid: Values,
    display: Values,
}

#[derive(Clone, Debug, Serialize)]
pub struct RemoteModeStatus {
    pub enabled: bool,
    pub effective: bool,
    pub snapshot_present: bool,
    pub original_scheme: Option<String>,
    pub active_scheme: String,
    pub discrepancies: Vec<String>,
    pub caffeine_active: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct WakeLockStatus {
    pub active_scheme: String,
    pub ac_requires_sign_in: bool,
    pub dc_requires_sign_in: bool,
}

impl WakeLockStatus {
    pub fn requires_sign_in(&self) -> bool {
        self.ac_requires_sign_in || self.dc_requires_sign_in
    }
}

trait PowerBackend {
    fn active_scheme(&self) -> Result<String, String>;
    fn read(&self, scheme: &str, subgroup: &str, setting: &str) -> Result<Values, String>;
    fn write(
        &self,
        scheme: &str,
        subgroup: &str,
        setting: &str,
        values: &Values,
    ) -> Result<(), String>;
    fn activate(&self, scheme: &str) -> Result<(), String>;
}

struct NativePower;
fn parse_guid(value: &str) -> Result<GUID, String> {
    if value.len() != 36
        || !value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err(format!("Invalid power-setting GUID: {value}"));
    }
    Ok(GUID::from(value))
}

fn power_result(operation: &str, code: u32) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!(
            "{operation} failed (Windows error {code}): {}",
            std::io::Error::from_raw_os_error(code as i32)
        ))
    }
}
impl PowerBackend for NativePower {
    fn active_scheme(&self) -> Result<String, String> {
        let mut pointer = std::ptr::null_mut();
        unsafe {
            power_result(
                "PowerGetActiveScheme",
                PowerGetActiveScheme(HKEY::default(), &mut pointer).0,
            )?;
            if pointer.is_null() {
                return Err("Windows returned no active power scheme".into());
            }
            let scheme = *pointer;
            let _ = LocalFree(HLOCAL(pointer.cast()));
            Ok(format!("{scheme:?}").to_ascii_lowercase())
        }
    }
    fn read(&self, scheme: &str, subgroup: &str, setting: &str) -> Result<Values, String> {
        let (scheme, subgroup, setting) = (
            parse_guid(scheme)?,
            parse_guid(subgroup)?,
            parse_guid(setting)?,
        );
        let (mut ac, mut dc) = (0, 0);
        unsafe {
            power_result(
                "PowerReadACValueIndex",
                PowerReadACValueIndex(
                    HKEY::default(),
                    Some(&scheme),
                    Some(&subgroup),
                    Some(&setting),
                    &mut ac,
                )
                .0,
            )?;
            power_result(
                "PowerReadDCValueIndex",
                PowerReadDCValueIndex(
                    HKEY::default(),
                    Some(&scheme),
                    Some(&subgroup),
                    Some(&setting),
                    &mut dc,
                ),
            )?;
        }
        Ok(Values { ac, dc })
    }
    fn write(&self, scheme: &str, subgroup: &str, setting: &str, v: &Values) -> Result<(), String> {
        let (scheme, subgroup, setting) = (
            parse_guid(scheme)?,
            parse_guid(subgroup)?,
            parse_guid(setting)?,
        );
        unsafe {
            power_result(
                "PowerWriteACValueIndex",
                PowerWriteACValueIndex(
                    HKEY::default(),
                    &scheme,
                    Some(&subgroup),
                    Some(&setting),
                    v.ac,
                )
                .0,
            )?;
            power_result(
                "PowerWriteDCValueIndex",
                PowerWriteDCValueIndex(
                    HKEY::default(),
                    &scheme,
                    Some(&subgroup),
                    Some(&setting),
                    v.dc,
                ),
            )?;
        }
        Ok(())
    }
    fn activate(&self, scheme: &str) -> Result<(), String> {
        let scheme = parse_guid(scheme)?;
        unsafe {
            power_result(
                "PowerSetActiveScheme",
                PowerSetActiveScheme(HKEY::default(), Some(&scheme)).0,
            )
        }
    }
}

struct FileLock {
    _file: File,
}
impl FileLock {
    fn acquire(path: PathBuf) -> Result<Self, String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .share_mode(0)
                .open(&path)
            {
                Ok(file) => return Ok(Self { _file: file }),
                Err(e) if e.raw_os_error() == Some(32) && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(50))
                }
                Err(e) => return Err(format!("remote mode lock unavailable: {e}")),
            }
        }
    }
}

fn state_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
    Ok(PathBuf::from(base).join("AutoClickTimer"))
}
fn with_lock<T>(f: impl FnOnce(&Path) -> Result<T, String>) -> Result<T, String> {
    let dir = state_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create state directory: {e}"))?;
    let _lock = FileLock::acquire(dir.join("remote-mode.lock"))?;
    f(&dir.join("remote-mode.json"))
}

pub fn status() -> Result<RemoteModeStatus, String> {
    with_lock(|p| status_with(&NativePower, p))
}

pub fn wake_lock_status() -> Result<WakeLockStatus, String> {
    wake_lock_status_with(&NativePower)
}

fn wake_lock_status_with(b: &impl PowerBackend) -> Result<WakeLockStatus, String> {
    let active_scheme = b.active_scheme()?;
    let values = b.read(&active_scheme, SUB_NONE, CONSOLELOCK)?;
    Ok(WakeLockStatus {
        active_scheme,
        ac_requires_sign_in: values.ac != 0,
        dc_requires_sign_in: values.dc != 0,
    })
}

pub fn set_enabled(enabled: bool) -> Result<RemoteModeStatus, String> {
    with_lock(|p| {
        set_enabled_with(&NativePower, p, enabled)?;
        status_with(&NativePower, p)
    })
}

/// Disable remote mode before an explicit suspend. It remains disabled after wake.
pub fn disable_before_suspend() -> Result<bool, String> {
    with_lock(|p| {
        if !p.exists() {
            return Ok(false);
        }
        set_enabled_with(&NativePower, p, false)?;
        Ok(true)
    })
}

fn capture(b: &impl PowerBackend) -> Result<Snapshot, String> {
    let scheme = b.active_scheme()?;
    Ok(Snapshot {
        version: 1,
        scheme: scheme.clone(),
        standby: b.read(&scheme, SUB_SLEEP, STANDBY)?,
        hibernate: b.read(&scheme, SUB_SLEEP, HIBERNATE)?,
        lid: b.read(&scheme, SUB_BUTTONS, LID)?,
        display: b.read(&scheme, SUB_VIDEO, DISPLAY)?,
    })
}

fn desired(s: &Snapshot) -> Snapshot {
    let mut d = s.clone();
    d.standby = Values { ac: 0, dc: 0 };
    d.hibernate = Values { ac: 0, dc: 0 };
    d.lid = Values { ac: 0, dc: 0 };
    d.display = Values {
        ac: if s.display.ac == 0 { 300 } else { s.display.ac },
        dc: if s.display.dc == 0 { 180 } else { s.display.dc },
    };
    d
}

fn apply(b: &impl PowerBackend, s: &Snapshot) -> Result<(), String> {
    if b.active_scheme()? != s.scheme {
        return Err("The active power plan changed. Turn remote mode off to restore its saved plan before enabling it on the new plan.".into());
    }
    let before = capture_scheme(b, &s.scheme)?;
    let writes = [
        (SUB_SLEEP, STANDBY, &s.standby),
        (SUB_SLEEP, HIBERNATE, &s.hibernate),
        (SUB_BUTTONS, LID, &s.lid),
        (SUB_VIDEO, DISPLAY, &s.display),
    ];
    for (sg, key, value) in writes {
        if let Err(e) = b.write(&s.scheme, sg, key, value) {
            let rollback = restore_settings(b, &before).err();
            return Err(match rollback {
                Some(r) => format!("{e}; rollback also failed: {r}"),
                None => e,
            });
        }
    }
    let activation = b.active_scheme().and_then(|active| {
        if active != s.scheme {
            Err("Power plan changed during enable; restoring captured settings".into())
        } else {
            b.activate(&s.scheme)
        }
    });
    if let Err(e) = activation {
        let rollback = restore_settings(b, &before).err();
        return Err(match rollback {
            Some(r) => format!("{e}; rollback also failed: {r}"),
            None => e,
        });
    }
    Ok(())
}
fn capture_scheme(b: &impl PowerBackend, scheme: &str) -> Result<Snapshot, String> {
    Ok(Snapshot {
        version: 1,
        scheme: scheme.into(),
        standby: b.read(scheme, SUB_SLEEP, STANDBY)?,
        hibernate: b.read(scheme, SUB_SLEEP, HIBERNATE)?,
        lid: b.read(scheme, SUB_BUTTONS, LID)?,
        display: b.read(scheme, SUB_VIDEO, DISPLAY)?,
    })
}
fn restore_settings(b: &impl PowerBackend, s: &Snapshot) -> Result<(), String> {
    let mut errors = Vec::new();
    for (sg, key, v) in [
        (SUB_SLEEP, STANDBY, &s.standby),
        (SUB_SLEEP, HIBERNATE, &s.hibernate),
        (SUB_BUTTONS, LID, &s.lid),
        (SUB_VIDEO, DISPLAY, &s.display),
    ] {
        if let Err(error) = b.write(&s.scheme, sg, key, v) {
            errors.push(error);
        }
    }
    match b.active_scheme() {
        Ok(active) if active == s.scheme => {
            if let Err(error) = b.activate(&active) {
                errors.push(error);
            }
        }
        Err(error) => errors.push(error),
        _ => {}
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
fn save(path: &Path, s: &Snapshot) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let mut file = File::create(&tmp).map_err(|e| format!("cannot create snapshot: {e}"))?;
    file.write_all(&serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?)
        .map_err(|e| format!("cannot write snapshot: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("cannot flush snapshot: {e}"))?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| format!("cannot commit snapshot: {e}"))
}
fn load(path: &Path) -> Result<Snapshot, String> {
    let snapshot: Snapshot =
        serde_json::from_slice(&fs::read(path).map_err(|e| format!("cannot read snapshot: {e}"))?)
            .map_err(|e| format!("invalid snapshot: {e}"))?;
    if snapshot.version != 1 {
        return Err(
            "Unsupported remote-mode snapshot version; saved settings were preserved".into(),
        );
    }
    Ok(snapshot)
}

fn set_enabled_with(b: &impl PowerBackend, path: &Path, enabled: bool) -> Result<(), String> {
    if enabled {
        if path.exists() {
            return apply(b, &desired(&load(path)?));
        }
        let original = capture(b)?;
        save(path, &original)?;
        if let Err(e) = apply(b, &desired(&original)) {
            return Err(e);
        }
    } else if path.exists() {
        let original = load(path)?;
        restore_settings(b, &original)?;
        fs::remove_file(path)
            .map_err(|e| format!("settings restored but snapshot cleanup failed: {e}"))?;
    }
    Ok(())
}

fn status_with(b: &impl PowerBackend, path: &Path) -> Result<RemoteModeStatus, String> {
    let active = b.active_scheme()?;
    let mut discrepancies = Vec::new();
    let caffeine_active = super::power::is_caffeine_active();
    let original = if path.exists() {
        Some(load(path)?)
    } else {
        None
    };
    if let Some(s) = &original {
        if active != s.scheme {
            discrepancies.push(format!(
                "active scheme differs from captured scheme {}",
                s.scheme
            ));
        }
        let actual = capture_scheme(b, &s.scheme)?;
        let d = desired(s);
        for (name, got, want) in [
            ("automatic sleep", actual.standby, d.standby),
            ("timed hibernate", actual.hibernate, d.hibernate),
            ("lid close", actual.lid, d.lid),
            ("display timeout", actual.display, d.display),
        ] {
            if got != want {
                discrepancies.push(format!(
                    "{name} differs: AC/DC {}/{} expected {}/{}",
                    got.ac, got.dc, want.ac, want.dc
                ));
            }
        }
    }
    if original.is_some() && caffeine_active {
        discrepancies
            .push("Caffeine is active and separately forces the display to remain on".to_string());
    }
    let enabled = original.is_some();
    let effective = enabled && discrepancies.is_empty();
    Ok(RemoteModeStatus {
        enabled,
        effective,
        snapshot_present: enabled,
        original_scheme: original.map(|s| s.scheme),
        active_scheme: active,
        discrepancies,
        caffeine_active,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    struct Mock {
        scheme: Mutex<String>,
        values: Mutex<HashMap<(String, String), Values>>,
        fail: Mutex<Option<String>>,
    }
    impl Mock {
        fn new() -> Self {
            let mut m = HashMap::new();
            for (sg, k, v) in [
                (SUB_SLEEP, STANDBY, Values { ac: 600, dc: 300 }),
                (SUB_SLEEP, HIBERNATE, Values { ac: 0, dc: 3600 }),
                (SUB_NONE, CONSOLELOCK, Values { ac: 1, dc: 1 }),
                (SUB_BUTTONS, LID, Values { ac: 1, dc: 1 }),
                (SUB_VIDEO, DISPLAY, Values { ac: 0, dc: 0 }),
            ] {
                m.insert((sg.into(), k.into()), v);
            }
            Self {
                scheme: Mutex::new("plan-a".into()),
                values: Mutex::new(m),
                fail: Mutex::new(None),
            }
        }
    }
    impl PowerBackend for Mock {
        fn active_scheme(&self) -> Result<String, String> {
            Ok(self.scheme.lock().unwrap().clone())
        }
        fn read(&self, _: &str, sg: &str, k: &str) -> Result<Values, String> {
            self.values
                .lock()
                .unwrap()
                .get(&(sg.into(), k.into()))
                .cloned()
                .ok_or("missing".into())
        }
        fn write(&self, _: &str, sg: &str, k: &str, v: &Values) -> Result<(), String> {
            if self.fail.lock().unwrap().as_deref() == Some(k) {
                return Err("injected".into());
            }
            self.values
                .lock()
                .unwrap()
                .insert((sg.into(), k.into()), v.clone());
            Ok(())
        }
        fn activate(&self, s: &str) -> Result<(), String> {
            *self.scheme.lock().unwrap() = s.into();
            Ok(())
        }
    }
    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("act-remote-{name}-{}.json", std::process::id()))
    }
    #[test]
    fn enable_is_idempotent_and_preserves_snapshot() {
        let p = temp("idem");
        let _ = fs::remove_file(&p);
        let b = Mock::new();
        set_enabled_with(&b, &p, true).unwrap();
        let first = fs::read(&p).unwrap();
        set_enabled_with(&b, &p, true).unwrap();
        assert_eq!(first, fs::read(&p).unwrap());
        let d = capture_scheme(&b, "plan-a").unwrap();
        assert_eq!(d.display, Values { ac: 300, dc: 180 });
        let _ = fs::remove_file(p);
    }
    #[test]
    fn disable_restores_original_without_stealing_current_plan() {
        let p = temp("restore");
        let _ = fs::remove_file(&p);
        let b = Mock::new();
        let original = capture(&b).unwrap();
        set_enabled_with(&b, &p, true).unwrap();
        *b.scheme.lock().unwrap() = "other-plan".into();
        set_enabled_with(&b, &p, false).unwrap();
        assert_eq!(
            capture_scheme(&b, "plan-a").unwrap().standby,
            original.standby
        );
        assert_eq!(b.active_scheme().unwrap(), "other-plan");
        assert!(!p.exists());
    }
    #[test]
    fn failed_enable_rolls_back_and_retains_recovery_snapshot() {
        let p = temp("rollback");
        let _ = fs::remove_file(&p);
        let b = Mock::new();
        let original = capture(&b).unwrap();
        *b.fail.lock().unwrap() = Some(HIBERNATE.into());
        assert!(set_enabled_with(&b, &p, true).is_err());
        *b.fail.lock().unwrap() = None;
        assert_eq!(capture(&b).unwrap().standby, original.standby);
        assert!(p.exists());
        let _ = fs::remove_file(p);
    }
    #[test]
    fn guid_validation_does_not_panic_on_corrupt_snapshot() {
        assert!(parse_guid("invalid").is_err());
        assert!(parse_guid("zzzzzzzz-0000-0000-0000-000000000000").is_err());
        assert!(parse_guid(SUB_SLEEP).is_ok());
    }
    #[test]
    fn wake_lock_status_reads_ac_and_dc_policy() {
        let b = Mock::new();
        let status = wake_lock_status_with(&b).unwrap();
        assert!(status.ac_requires_sign_in);
        assert!(status.dc_requires_sign_in);
        assert!(status.requires_sign_in());
    }
    #[test]
    fn reenable_does_not_switch_an_external_plan() {
        let p = temp("external");
        let _ = fs::remove_file(&p);
        let b = Mock::new();
        set_enabled_with(&b, &p, true).unwrap();
        *b.scheme.lock().unwrap() = "other-plan".into();
        assert!(set_enabled_with(&b, &p, true).is_err());
        assert_eq!(b.active_scheme().unwrap(), "other-plan");
        assert!(p.exists());
        let _ = fs::remove_file(p);
    }
    #[test]
    fn failed_restore_retains_snapshot_for_retry() {
        let p = temp("retry");
        let _ = fs::remove_file(&p);
        let b = Mock::new();
        set_enabled_with(&b, &p, true).unwrap();
        *b.fail.lock().unwrap() = Some(LID.into());
        assert!(set_enabled_with(&b, &p, false).is_err());
        assert!(p.exists());
        *b.fail.lock().unwrap() = None;
        set_enabled_with(&b, &p, false).unwrap();
        assert!(!p.exists());
        assert_eq!(
            b.read("plan-a", SUB_BUTTONS, LID).unwrap(),
            Values { ac: 1, dc: 1 }
        );
    }
    #[test]
    fn lock_file_can_be_reused_after_owner_exits() {
        let p = temp("lock");
        let first = FileLock::acquire(p.clone()).unwrap();
        assert!(
            OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .open(&p)
                .is_err()
        );
        drop(first);
        let second = FileLock::acquire(p.clone()).unwrap();
        drop(second);
        fs::remove_file(p).unwrap();
    }
}
