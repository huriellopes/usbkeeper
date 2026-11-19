//! Onde fica o histórico e como gravar nele a partir dos comandos.
//!
//! Os testes rodam com sudo (ou pkexec, pela interface); sem cuidado o banco
//! iria para /root. Usa a home de quem chamou e devolve a posse dos arquivos.

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

/// Quem chamou de verdade quando o comando roda como root.
/// (`uid`/`gid` só servem para o chown, que só existe em Unix.)
#[cfg_attr(not(unix), allow(dead_code))]
pub struct Invoker {
    pub home: PathBuf,
    pub uid: u32,
    pub gid: u32,
}

pub enum Lookup<'a> {
    Name(&'a str),
    Uid(u32),
}

pub fn passwd_entry(passwd: &str, by: Lookup<'_>) -> Option<Invoker> {
    passwd.lines().find_map(|line| {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() < 6 {
            return None;
        }
        let uid: u32 = f[2].parse().ok()?;
        let hit = match by {
            Lookup::Name(n) => f[0] == n,
            Lookup::Uid(u) => uid == u,
        };
        hit.then(|| Invoker {
            home: PathBuf::from(f[5]),
            uid,
            gid: f[3].parse().unwrap_or(uid),
        })
    })
}

/// Via sudo (`SUDO_USER`) ou via pkexec (`PKEXEC_UID`, usado pela interface).
fn invoker() -> Option<Invoker> {
    if !cfg!(unix) {
        return None;
    }
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    if let Ok(user) = std::env::var("SUDO_USER")
        && user != "root"
    {
        return passwd_entry(&passwd, Lookup::Name(&user));
    }
    let uid: u32 = std::env::var("PKEXEC_UID").ok()?.parse().ok()?;
    (uid != 0)
        .then(|| passwd_entry(&passwd, Lookup::Uid(uid)))
        .flatten()
}

pub fn db_path() -> Option<PathBuf> {
    std::env::var_os("USBKEEPER_DB")
        .map(PathBuf::from)
        .or_else(|| {
            usbkeeper_history::default_db_path(invoker().as_ref().map(|i| i.home.as_path()))
        })
}

/// Devolve a quem chamou (sudo/pkexec) a posse de um arquivo criado como root.
pub fn give_back(path: &Path) {
    #[cfg(unix)]
    if let Some(i) = invoker() {
        let _ = std::os::unix::fs::chown(path, Some(i.uid), Some(i.gid));
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

/// Do histórico: (capacidade real, limite sugerido) do último fake-check, se
/// ele deu falso, e o último veredito de saúde. Não cria o banco só para ler.
pub fn last_facts(device: &Device) -> (Option<(u64, u64)>, Option<String>) {
    if !db_path().is_some_and(|p| p.exists()) {
        return (None, None);
    }
    let Ok(h) = open() else {
        return (None, None);
    };
    let Ok(events) = h.events(&device_info(device).key()) else {
        return (None, None);
    };
    let fake = events
        .iter()
        .rev()
        .find(|e| e.kind == usbkeeper_history::kind::FAKE_CHECK)
        .filter(|e| e.verdict.as_deref() == Some("fake"))
        .and_then(|e| {
            Some((
                e.data.get("real_bytes")?.as_u64()?,
                e.data.get("suggested_max_bytes")?.as_u64()?,
            ))
        });
    let health = events
        .iter()
        .rev()
        .find(|e| e.kind == usbkeeper_history::kind::HEALTH)
        .and_then(|e| e.verdict.clone());
    (fake, health)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWD: &str =
        "root:x:0:0:root:/root:/bin/bash\nana:x:1000:1000:Ana,,,:/home/ana:/bin/zsh\n";

    #[test]
    fn finds_user_by_name_and_uid() {
        let by_name = passwd_entry(PASSWD, Lookup::Name("ana")).unwrap();
        assert_eq!((by_name.uid, by_name.gid), (1000, 1000));
        assert_eq!(by_name.home, PathBuf::from("/home/ana"));
        let by_uid = passwd_entry(PASSWD, Lookup::Uid(1000)).unwrap();
        assert_eq!(by_uid.home, PathBuf::from("/home/ana"));
        assert!(passwd_entry(PASSWD, Lookup::Uid(4242)).is_none());
    }
}
