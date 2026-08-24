//! GitHub release update checker and self-replacement updater.

use std::fs::File;
use std::io::Write;
use std::process::Command;

pub const CURRENT_VERSION: &str = "1.4.2";
pub const REPO: &str = "Xpl4iN/AutoClickTimer-Rust";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub tag: String,
    pub version: String,
    pub download_url: String,
    #[allow(dead_code)]
    pub release_url: String,
}

pub fn check_for_update(repo: &str, current_version: &str) -> Option<UpdateInfo> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .build();

    let response = agent
        .get(&url)
        .set("User-Agent", "AutoClickTimer-Rust-Updater/1.0")
        .call()
        .ok()?;

    let json: serde_json::Value = response.into_json().ok()?;
    let tag = json.get("tag_name")?.as_str()?.to_string();
    let version = tag.trim_start_matches('v').to_string();

    if parse_ver(&version) <= parse_ver(current_version) {
        return None;
    }

    let assets = json.get("assets")?.as_array()?;
    let exe_asset = assets.iter().find(|a| {
        a.get("name")
            .and_then(|n| n.as_str())
            .map(|n| n.to_lowercase().ends_with(".exe"))
            .unwrap_or(false)
    })?;

    let download_url = exe_asset.get("browser_download_url")?.as_str()?.to_string();
    let release_url = json.get("html_url")?.as_str()?.to_string();

    Some(UpdateInfo {
        tag,
        version,
        download_url,
        release_url,
    })
}

fn parse_ver(s: &str) -> (u32, u32, u32) {
    let parts: Vec<u32> = s
        .trim_start_matches('v')
        .split('.')
        .filter_map(|x| x.parse().ok())
        .collect();

    (
        parts.get(0).copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    )
}

pub fn download_and_apply<F>(info: &UpdateInfo, log_fn: F) -> Result<(), String>
where
    F: Fn(&str),
{
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let tmp_path = current_exe.with_extension("exe.new");
    let bat_path = current_exe.with_extension("update.bat");

    log_fn(&format!("Herunterladen von Version {}...", info.version));

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(60))
        .build();

    let resp = agent
        .get(&info.download_url)
        .set("User-Agent", "AutoClickTimer-Rust-Updater/1.0")
        .call()
        .map_err(|e| format!("Download error: {}", e))?;

    let mut reader = resp.into_reader();
    let mut file = File::create(&tmp_path).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    drop(file);

    log_fn("Download abgeschlossen. Update wird vorbereitet...");

    let bat_content = format!(
        "@echo off\r\n\
         timeout /t 2 /nobreak > nul\r\n\
         copy /y \"{}\" \"{}\"\r\n\
         if errorlevel 1 (\r\n\
           echo Update fehlgeschlagen.\r\n\
           pause\r\n\
           goto :eof\r\n\
         )\r\n\
         del \"{}\"\r\n\
         start \"\" \"{}\"\r\n\
         del \"%~f0\"\r\n",
        tmp_path.display(),
        current_exe.display(),
        tmp_path.display(),
        current_exe.display()
    );

    let mut bat_file = File::create(&bat_path).map_err(|e| e.to_string())?;
    bat_file.write_all(bat_content.as_bytes()).map_err(|e| e.to_string())?;
    drop(bat_file);

    log_fn("Neustart zum Anwenden des Updates...");

    let _ = Command::new("cmd.exe")
        .args(["/c", &bat_path.to_string_lossy()])
        .spawn();

    std::process::exit(0);
}
