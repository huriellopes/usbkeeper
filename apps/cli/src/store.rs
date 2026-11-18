//! Onde fica o histórico e como gravar nele a partir dos comandos.
//!
//! Os testes rodam com sudo; sem cuidado o banco iria para /root. Com
//! `SUDO_USER`, usa a home desse usuário e devolve a posse dos arquivos a ele.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use serde_json::Value;
use usbkeeper_core::Device;
use usbkeeper_history::{DeviceInfo, History};

static DISABLED: AtomicBool = AtomicBool::new(false);

pub fn disable() {
    DISABLED.store(true, Ordering::Relaxed);
}

/// Usuário "de verdade" quando rodando via sudo: (home, uid, gid).
#[cfg(unix)]
fn sudo_user() -> Option<(PathBuf, u32, u32)> {
    let user = std::env::var("SUDO_USER").ok().filter(|u| u != "root")?;
    let uid = std::env::var("SUDO_UID").ok()?.parse().ok()?;
    let gid = std::env::var("SUDO_GID").ok()?.parse().ok()?;
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    let home = passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.first() == Some(&user.as_str()))
            .then(|| fields.get(5).map(PathBuf::from))
            .flatten()
    })?;
    Some((home, uid, gid))
}

#[cfg(not(unix))]
fn sudo_user() -> Option<(PathBuf, u32, u32)> {
    None
}

fn data_dir() -> Option<PathBuf> {
    if let Some((home, _, _)) = sudo_user() {
        return Some(home.join(".local/share/usbkeeper"));
    }
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("usbkeeper"));
    }
    if cfg!(target_os = "macos") {
        return std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/usbkeeper"));
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .map(|d| d.join("usbkeeper"))
}

pub fn db_path() -> Option<PathBuf> {
    std::env::var_os("USBKEEPER_DB")
        .map(PathBuf::from)
        .or_else(|| data_dir().map(|d| d.join("history.db")))
}

/// Devolve ao usuário do sudo a posse de um arquivo/diretório criado como root.
pub fn give_back(path: &Path) {
    #[cfg(unix)]
    if let Some((_, uid, gid)) = sudo_user() {
        let _ = std::os::unix::fs::chown(path, Some(uid), Some(gid));
    }
    #[cfg(not(unix))]
    let _ = path;
}

pub fn open() -> Result<History> {
    let path = db_path().context("não sei onde guardar o histórico (defina USBKEEPER_DB)")?;
    if let Some(dir) = path.parent()
        && !dir.exists()
    {
        std::fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
        give_back(dir);
    }
    let history =
        History::open(&path).with_context(|| format!("abrindo o histórico {}", path.display()))?;
    give_back(&path);
    Ok(history)
}

pub fn device_info(device: &Device) -> DeviceInfo {
    DeviceInfo {
        serial: device.serial.clone(),
        vendor_id: device.usb.as_ref().map(|u| u.vendor_id.clone()),
        product_id: device.usb.as_ref().map(|u| u.product_id.clone()),
        model: Some(device.display_name()),
        advertised_bytes: device.size_bytes,
    }
}

/// Grava um evento; problema no histórico vira aviso, nunca falha o comando.
pub fn record(device: &Device, kind: &str, verdict: Option<&str>, summary: &str, data: Value) {
    if DISABLED.load(Ordering::Relaxed) {
        return;
    }
    let result = open().and_then(|mut h| {
        h.record(
            &device_info(device),
            kind,
            verdict,
            summary,
            &data,
            usbkeeper_history::now(),
        )
        .context("gravando no histórico")
    });
    if let Err(e) = result {
        eprintln!("aviso: não gravei no histórico: {e:#}");
    }
}

/// Nome do veredito como gravado (snake_case do serde).
pub fn verdict_name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}
