use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::Command;

use qrcode::{Color as QrColor, QrCode};
use rand::RngCore;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

const KEY_ENV: &str = "AUTOCLICKTIMER_MCP_API_KEY";
pub const PORT: u16 = 7890;

pub struct Pairing {
    pub key: String,
    pub tailscale_ip: Option<Ipv4Addr>,
}

fn key_path() -> Result<PathBuf, String> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or("LOCALAPPDATA is unavailable; remote pairing is disabled")?;
    Ok(PathBuf::from(local_app_data).join("AutoClickTimer").join("pairing-key.txt"))
}

pub fn read_key() -> Result<Option<String>, String> {
    if let Ok(key) = std::env::var(KEY_ENV) {
        if !key.trim().is_empty() {
            return Ok(Some(key));
        }
    }
    let path = key_path()?;
    match fs::read_to_string(path) {
        Ok(key) if !key.trim().is_empty() => Ok(Some(key.trim().to_string())),
        Ok(_) => Err("Stored pairing key is empty".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Could not read pairing key: {error}")),
    }
}

pub fn load_or_create_key() -> Result<String, String> {
    if let Some(key) = read_key()? {
        return Ok(key);
    }
    let path = key_path()?;
    fs::create_dir_all(path.parent().unwrap())
        .map_err(|error| format!("Could not create pairing directory: {error}"))?;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let key = bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(key.as_bytes())
                .map_err(|error| format!("Could not save pairing key: {error}"))?;
            file.sync_all().map_err(|error| format!("Could not sync pairing key: {error}"))?;
            Ok(key)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => read_key()?
            .ok_or_else(|| "Pairing key disappeared while loading".into()),
        Err(error) => Err(format!("Could not create pairing key: {error}")),
    }
}

pub fn tailscale_ip() -> Option<Ipv4Addr> {
    let candidates = ["tailscale.exe", r"C:\Program Files\Tailscale\tailscale.exe"];
    for candidate in candidates {
        let output = Command::new(candidate).args(["ip", "-4"]).output().ok();
        if let Some(output) = output.filter(|output| output.status.success()) {
            if let Ok(ip) = String::from_utf8(output.stdout) {
                if let Ok(ip) = ip.trim().parse() {
                    return Some(ip);
                }
            }
        }
    }
    None
}

pub fn load() -> Result<Pairing, String> {
    Ok(Pairing { key: load_or_create_key()?, tailscale_ip: tailscale_ip() })
}

pub fn qr_image(pairing: &Pairing) -> Result<Image, String> {
    let ip = pairing.tailscale_ip.ok_or("Tailscale is not connected")?;
    let payload = serde_json::json!({
        "app": "autoclicktimer",
        "host": ip.to_string(),
        "port": PORT,
        "key": pairing.key,
    }).to_string();
    let code = QrCode::new(payload.as_bytes()).map_err(|error| error.to_string())?;
    let modules = code.width();
    let scale = 4usize;
    let quiet = 4usize;
    let side = (modules + quiet * 2) * scale;
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(side as u32, side as u32);
    for (index, pixel) in pixels.make_mut_bytes().chunks_exact_mut(4).enumerate() {
        let x = (index % side) / scale;
        let y = (index / side) / scale;
        let dark = x >= quiet && y >= quiet && x < modules + quiet && y < modules + quiet
            && code[(x - quiet, y - quiet)] == QrColor::Dark;
        let shade = if dark { 0 } else { 255 };
        pixel.copy_from_slice(&[shade, shade, shade, 255]);
    }
    Ok(Image::from_rgba8(pixels))
}
