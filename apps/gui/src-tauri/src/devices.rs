//! Leitura sem privilégio: discos, quem os usa e histórico.

use serde::Serialize;
use ts_rs::TS;
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

#[tauri::command]
pub fn list_devices() -> Result<Vec<DeviceView>, String> {
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
