//! Leitura sem privilégio: discos, quem os usa e histórico.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;
use ts_rs::TS;
use usbkeeper_core::advice::{Advice, BigFiles, DiskFacts, Usage};
use usbkeeper_core::safety::{self, Policy, Refusal};
use usbkeeper_core::{Device, Holder};
use usbkeeper_history::{DeviceInfo, DeviceSummary, Event, History, kind};

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DeviceView {
    pub device: Device,
    pub refusals: Vec<Refusal>,
    pub is_system: bool,
    pub confirmation_token: String,
    pub history_key: String,
    pub last_fake_verdict: Option<String>,
    pub last_health_verdict: Option<String>,
    #[ts(type = "number | null")]
    pub suggested_max_bytes: Option<u64>,
}

/// Desenvolvimento: lista loops e aceita alvos não-USB.
pub fn show_virtual() -> bool {
    std::env::var_os("USBKEEPER_SHOW_VIRTUAL").is_some()
}

fn info(d: &Device) -> DeviceInfo {
    DeviceInfo {
        serial: d.serial.clone(),
        vendor_id: d.usb.as_ref().map(|u| u.vendor_id.clone()),
        product_id: d.usb.as_ref().map(|u| u.product_id.clone()),
        model: Some(d.display_name()),
        advertised_bytes: d.size_bytes,
    }
}

pub fn device_view(
    device: Device,
    summaries: &[DeviceSummary],
    last_fake: Option<&Event>,
) -> DeviceView {
    let policy = Policy {
        allow_non_usb: show_virtual(),
    };
    let key = info(&device).key();
    let summary = summaries.iter().find(|s| s.device.key == key);
    DeviceView {
        refusals: safety::check_target(&device, policy),
        is_system: safety::system_mountpoint(&device).is_some(),
        confirmation_token: safety::confirmation_token(&device).to_string(),
        last_fake_verdict: summary.and_then(|s| s.last_fake_verdict.clone()),
        last_health_verdict: summary.and_then(|s| s.last_health_verdict.clone()),
        suggested_max_bytes: last_fake
            .filter(|e| e.verdict.as_deref() == Some("fake"))
            .and_then(|e| e.data.get("suggested_max_bytes")?.as_u64()),
        history_key: key,
        device,
    }
}

fn open_history() -> Option<History> {
    let path = std::env::var_os("USBKEEPER_DB")
        .map(std::path::PathBuf::from)
        .or_else(|| usbkeeper_history::default_db_path(None))?;
    path.exists().then(|| History::open(&path).ok()).flatten()
}

#[cfg(target_os = "linux")]
fn enumerate() -> Result<Vec<Device>, String> {
    use usbkeeper_core::Enumerator;
    usbkeeper_linux::LinuxEnumerator {
        include_virtual: show_virtual(),
        ..Default::default()
    }
    .list()
    .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
fn enumerate() -> Result<Vec<Device>, String> {
    Err("plataforma ainda não suportada pela interface".into())
}

/// Mesmo sem mudança aparente, relê de tempos em tempos (ex.: protetor de
/// escrita acionado, que não muda partições nem montagens).
const MAX_AGE: Duration = Duration::from_secs(30);

/// Cache da lista: o `lsblk` custa ~25 ms de CPU e a interface consulta a
/// cada 2 s; só vale rodar quando algo mudou.
#[derive(Default)]
pub struct DeviceCache(Mutex<Option<(String, Instant, Vec<DeviceView>)>>);

/// Assinatura barata do estado dos discos: partições (plugar/remover/trocar
/// mídia), montagens e a última gravação do histórico (vereditos).
fn signature() -> String {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let history = std::env::var_os("USBKEEPER_DB")
        .map(std::path::PathBuf::from)
        .or_else(|| usbkeeper_history::default_db_path(None))
        .and_then(|p| std::fs::metadata(p).ok()?.modified().ok())
        .map(|t| format!("{t:?}"))
        .unwrap_or_default();
    format!(
        "{}\n{}\n{}",
        read("/proc/partitions"),
        read("/proc/self/mounts"),
        history
    )
}

pub fn needs_refresh(cached: Option<(&str, Duration)>, current: &str) -> bool {
    match cached {
        Some((sig, age)) => sig != current || age >= MAX_AGE,
        None => true,
    }
}

#[tauri::command]
pub fn list_devices(cache: State<'_, DeviceCache>) -> Result<Vec<DeviceView>, String> {
    let sig = signature();
    let mut guard = cache.0.lock().map_err(|e| e.to_string())?;
    if let Some((old, at, views)) = guard.as_ref()
        && !needs_refresh(Some((old.as_str(), at.elapsed())), &sig)
    {
        return Ok(views.clone());
    }
    let views = build_views()?;
    *guard = Some((sig, Instant::now(), views.clone()));
    Ok(views)
}

fn build_views() -> Result<Vec<DeviceView>, String> {
    let devices = enumerate()?;
    let history = open_history();
    let summaries = history
        .as_ref()
        .and_then(|h| h.devices().ok())
        .unwrap_or_default();
    let mut views: Vec<DeviceView> = devices
        .into_iter()
        .map(|d| {
            let key = info(&d).key();
            let events = history
                .as_ref()
                .and_then(|h| h.events(&key).ok())
                .unwrap_or_default();
            let last_fake = events.iter().rev().find(|e| e.kind == kind::FAKE_CHECK);
            device_view(d, &summaries, last_fake)
        })
        .collect();
    // USB primeiro, sistema por último.
    views.sort_by_key(|v| {
        (
            v.is_system,
            v.device.transport != usbkeeper_core::Transport::Usb,
            v.device.name.clone(),
        )
    });
    Ok(views)
}

/// WSL2 não enxerga USB do Windows: a lista vazia mostra a dica do usbipd.
#[tauri::command]
pub fn usb_hint() -> Option<String> {
    #[cfg(target_os = "linux")]
    if usbkeeper_linux::wsl::running_in_wsl() {
        return Some(usbkeeper_linux::wsl::USBIPD_HINT.to_string());
    }
    None
}

#[tauri::command]
pub fn device_holders(path: String) -> Vec<Holder> {
    #[cfg(target_os = "linux")]
    {
        use usbkeeper_core::{Enumerator, Mounts};
        let e = usbkeeper_linux::LinuxEnumerator {
            include_virtual: true,
            ..Default::default()
        };
        if let Ok(d) = e.get(&path) {
            return usbkeeper_linux::LinuxMounts.holders(&d);
        }
    }
    let _ = path;
    Vec::new()
}

/// Fatos do histórico para o assistente: último fake-check falso
/// (capacidade real, limite sugerido) e último veredito de saúde.
fn history_facts(device: &Device) -> (Option<(u64, u64)>, Option<String>) {
    let events = open_history()
        .and_then(|h| h.events(&info(device).key()).ok())
        .unwrap_or_default();
    let fake = events
        .iter()
        .rev()
        .find(|e| e.kind == kind::FAKE_CHECK)
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
        .find(|e| e.kind == kind::HEALTH)
        .and_then(|e| e.verdict.clone());
    (fake, health)
}

/// Assistente de formatação (sem root: só lê o disco e o histórico).
#[tauri::command]
pub fn advise(device: String, usage: Usage, big_files: BigFiles) -> Result<Advice, String> {
    let dev = enumerate()?
        .into_iter()
        .find(|d| d.path == device)
        .ok_or_else(|| format!("{device} não está mais conectado"))?;
    let (fake, health) = history_facts(&dev);
    Ok(usbkeeper_core::advice::advise(
        usage,
        big_files,
        &DiskFacts::of(&dev, fake, health),
    ))
}

/// Só abre no gerenciador de arquivos um ponto de montagem do próprio
/// disco; nunca um caminho arbitrário vindo da interface.
pub fn allowed_mountpoint(device: &Device, mountpoint: &str) -> bool {
    mountpoint.starts_with('/') && device.all_mountpoints().any(|m| m == mountpoint)
}

fn find_device(path: &str) -> Result<Device, String> {
    enumerate()?
        .into_iter()
        .find(|d| d.path == path)
        .ok_or_else(|| format!("{path} não está mais conectado"))
}

/// Monta as partições (udisks, sem senha) para aparecerem no gerenciador de arquivos.
#[tauri::command]
pub fn mount_device(path: String) -> Result<Vec<String>, String> {
    let dev = find_device(&path)?;
    let refusals = safety::check_target(
        &dev,
        Policy {
            allow_non_usb: show_virtual(),
        },
    );
    if let Some(r) = refusals
        .iter()
        .find(|r| r.is_absolute() || matches!(r, Refusal::NotUsb { .. }))
    {
        return Err(r.message());
    }
    #[cfg(target_os = "linux")]
    {
        use usbkeeper_core::Mounts;
        usbkeeper_linux::LinuxMounts
            .mount_all(&dev)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    Err("montar ainda não é suportado nesta plataforma".into())
}

#[tauri::command]
pub fn open_folder(path: String, mountpoint: String) -> Result<(), String> {
    let dev = find_device(&path)?;
    if !allowed_mountpoint(&dev, &mountpoint) {
        return Err("esse caminho não é um ponto de montagem deste disco".into());
    }
    std::process::Command::new("xdg-open")
        .arg(&mountpoint)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("não foi possível abrir o gerenciador de arquivos: {e}"))
}

/// Lê `<imagem>.json` (modelo, tamanho, data, hash) para mostrar antes de restaurar.
#[tauri::command]
pub fn image_meta(path: String) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(format!("{path}.json")).ok()?;
    serde_json::from_str(&text).ok()
}

#[tauri::command]
pub fn history_events(key: String) -> Result<Vec<Event>, String> {
    match open_history() {
        None => Ok(Vec::new()),
        Some(h) => h.events(&key).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usbkeeper_core::{Partition, Transport};

    fn dev(transport: Transport, mount: &str) -> Device {
        Device {
            name: "sdb".into(),
            path: "/dev/sdb".into(),
            size_bytes: 64_000_000_000,
            transport,
            removable: true,
            hotplug: true,
            read_only: false,
            vendor: Some("Generic".into()),
            model: Some("Flash".into()),
            serial: Some("ABC".into()),
            partition_table: None,
            logical_sector: 512,
            physical_sector: 512,
            mountpoints: vec![],
            partitions: vec![Partition {
                name: "sdb1".into(),
                path: "/dev/sdb1".into(),
                size_bytes: 1,
                fstype: None,
                label: None,
                mountpoints: if mount.is_empty() {
                    vec![]
                } else {
                    vec![mount.into()]
                },
            }],
            usb: None,
        }
    }

    #[test]
    fn system_disk_is_flagged_and_token_is_serial() {
        let v = device_view(dev(Transport::Nvme, "/"), &[], None);
        assert!(v.is_system);
        assert_eq!(v.confirmation_token, "ABC");
        let v = device_view(dev(Transport::Usb, ""), &[], None);
        assert!(!v.is_system && v.refusals.is_empty());
    }

    #[test]
    fn only_own_mountpoints_can_be_opened() {
        let d = dev(Transport::Usb, "/media/ana/PEN");
        assert!(allowed_mountpoint(&d, "/media/ana/PEN"));
        assert!(!allowed_mountpoint(&d, "/etc"));
        assert!(!allowed_mountpoint(&d, "/media/ana/PEN/../../.."));
        assert!(!allowed_mountpoint(&d, "media/ana/PEN"));
    }

    #[test]
    fn refresh_only_on_change_or_age() {
        assert!(needs_refresh(None, "a"));
        assert!(!needs_refresh(Some(("a", Duration::from_secs(5))), "a"));
        assert!(needs_refresh(Some(("a", Duration::from_secs(5))), "b"));
        assert!(needs_refresh(Some(("a", MAX_AGE)), "a"));
    }

    #[test]
    fn suggested_max_comes_from_last_fake_event() {
        let ev = Event {
            id: 1,
            at: 1,
            kind: "fake_check".into(),
            verdict: Some("fake".into()),
            summary: String::new(),
            data: serde_json::json!({"suggested_max_bytes": 7_000_000_000u64}),
        };
        let v = device_view(dev(Transport::Usb, ""), &[], Some(&ev));
        assert_eq!(v.suggested_max_bytes, Some(7_000_000_000));
    }
}
