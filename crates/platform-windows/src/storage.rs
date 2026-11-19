//! Discos do Windows a partir dos cmdlets de Storage do PowerShell
//! (`Get-Disk`, `Get-Partition`, `Get-Volume`), no papel do `lsblk -J`.
//!
//! O `ConvertTo-Json` do Windows PowerShell 5.1 tem manias: enums saem como
//! número ou nome, listas de um item viram objeto e campos somem como `null`.
//! A interpretação aqui aceita todas as formas.

use std::collections::HashMap;

use serde_json::Value;
use usbkeeper_core::{CoreError, Device, Partition, Transport, UsbInfo, UsbSpeed};

/// Script executado por `powershell.exe -NoProfile -NonInteractive -Command`.
/// Imprime um único objeto JSON `{system_drive, disks, partitions, volumes, parents}`.
pub const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
function Letter($c) { if ($c -and [int][char]$c -ne 0) { [string]$c } else { $null } }
$disks = @(Get-Disk | Select-Object Number, FriendlyName, Model, Manufacturer, SerialNumber, Size,
  @{n='BusType';e={[string]$_.BusType}}, @{n='PartitionStyle';e={[string]$_.PartitionStyle}},
  IsReadOnly, IsSystem, IsBoot, IsOffline, LogicalSectorSize, PhysicalSectorSize, Path)
$parts = @(Get-Partition -ErrorAction SilentlyContinue | Select-Object DiskNumber, PartitionNumber, Size,
  @{n='DriveLetter';e={Letter $_.DriveLetter}}, @{n='AccessPaths';e={@($_.AccessPaths)}})
$vols = @(Get-Volume -ErrorAction SilentlyContinue | Select-Object Path, FileSystem, FileSystemLabel, Size,
  @{n='DriveLetter';e={Letter $_.DriveLetter}})
$parents = @{}
foreach ($d in $disks) {
  if ($d.BusType -notmatch '^(USB|7)$' -or -not $d.Path) { continue }
  try {
    $id = (($d.Path -replace '^\\\\\?\\', '') -replace '#\{[^}]+\}$', '') -replace '#', '\'
    for ($i = 0; $i -lt 4 -and $id; $i++) {
      $p = (Get-PnpDeviceProperty -InstanceId $id -KeyName DEVPKEY_Device_Parent -ErrorAction Stop).Data
      if ($p -match '^USB\\VID_[0-9A-Fa-f]{4}&PID_[0-9A-Fa-f]{4}') { $parents["$($d.Number)"] = $p; break }
      $id = $p
    }
  } catch { }
}
[pscustomobject]@{ system_drive = $env:SystemDrive; disks = $disks; partitions = $parts;
  volumes = $vols; parents = $parents } | ConvertTo-Json -Depth 5 -Compress
"#;

/// Lista ou objeto único → lista; `null`/ausente → vazia.
fn items(v: Option<&Value>) -> Vec<&Value> {
    match v {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(Value::Null) | None => vec![],
        Some(other) => vec![other],
    }
}

fn text(v: &Value, key: &str) -> Option<String> {
    let s = match v.get(key)? {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!s.is_empty()).then_some(s)
}

fn num(v: &Value, key: &str) -> Option<u64> {
    match v.get(key)? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// `BusType` do MSFT_Disk, por nome ou pelo número do enum.
pub fn bus_type(v: &Value) -> Transport {
    let raw = match v {
        Value::String(s) => s.trim().to_ascii_lowercase(),
        Value::Number(n) => n.to_string(),
        _ => return Transport::Other,
    };
    match raw.as_str() {
        "usb" | "7" => Transport::Usb,
        "sata" | "ata" | "atapi" | "11" | "3" | "2" => Transport::Sata,
        "nvme" | "17" => Transport::Nvme,
        "sd" | "mmc" | "12" | "13" => Transport::Mmc,
        "scsi" | "sas" | "raid" | "iscsi" | "fibre channel" | "1" | "6" | "8" | "9" | "10" => {
            Transport::Scsi
        }
        "virtual" | "file backed virtual" | "14" | "15" => Transport::Virtual,
        _ => Transport::Other,
    }
}

fn partition_table(v: Option<&Value>) -> Option<String> {
    let raw = match v? {
        Value::String(s) => s.trim().to_ascii_lowercase(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    match raw.as_str() {
        "gpt" | "2" => Some("gpt".into()),
        "mbr" | "1" => Some("dos".into()),
        _ => None,
    }
}

fn fstype(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("unknown") || t.eq_ignore_ascii_case("raw") {
        return None;
    }
    Some(match t.to_ascii_lowercase().as_str() {
        "fat32" | "fat" | "fat16" | "fat12" => "vfat".to_string(),
        other => other.to_string(),
    })
}

fn usb_ids(parent: &str) -> Option<(String, String)> {
    let up = parent.to_ascii_uppercase();
    let vid = up.split("VID_").nth(1)?.get(..4)?.to_string();
    let pid = up.split("PID_").nth(1)?.get(..4)?.to_string();
    Some((vid.to_ascii_lowercase(), pid.to_ascii_lowercase()))
}

fn letter_of(mount: &str) -> Option<char> {
    let mut c = mount.chars();
    let l = c.next()?;
    (l.is_ascii_alphabetic() && c.next() == Some(':')).then(|| l.to_ascii_uppercase())
}

pub fn parse(json: &str) -> Result<Vec<Device>, CoreError> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| CoreError::Backend(format!("saída do PowerShell ilegível: {e}")))?;
    let system_letter = root
        .get("system_drive")
        .and_then(Value::as_str)
        .and_then(letter_of);

    let volumes: HashMap<String, &Value> = items(root.get("volumes"))
        .into_iter()
        .filter_map(|v| Some((text(v, "Path")?.to_ascii_lowercase(), v)))
        .collect();
    let by_letter: HashMap<char, &Value> = items(root.get("volumes"))
        .into_iter()
        .filter_map(|v| {
            Some((
                text(v, "DriveLetter")?.chars().next()?.to_ascii_uppercase(),
                v,
            ))
        })
        .collect();
    let parents = root.get("parents");

    let mut by_disk: HashMap<u64, Vec<(u64, Partition)>> = HashMap::new();
    for p in items(root.get("partitions")) {
        let (Some(disk), Some(number)) = (num(p, "DiskNumber"), num(p, "PartitionNumber")) else {
            continue;
        };
        let access: Vec<String> = items(p.get("AccessPaths"))
            .into_iter()
            .filter_map(|a| a.as_str().map(str::to_string))
            .collect();
        let letter = text(p, "DriveLetter")
            .and_then(|l| l.chars().next())
            .map(|l| l.to_ascii_uppercase());
        let mut volume_path = access
            .iter()
            .find(|a| a.starts_with(r"\\?\Volume"))
            .cloned();
        // Partições com letra nem sempre trazem o caminho \\?\Volume em
        // AccessPaths (visto no Windows real): junta também pela letra.
        let vol = volume_path
            .as_ref()
            .and_then(|vp| volumes.get(&vp.to_ascii_lowercase()))
            .or_else(|| letter.and_then(|l| by_letter.get(&l)))
            .copied();
        if volume_path.is_none() {
            volume_path = vol.and_then(|v| text(v, "Path"));
        }
        let mut mountpoints = Vec::new();
        if let Some(l) = letter {
            mountpoints.push(format!(r"{l}:\"));
        }
        for a in &access {
            if !a.starts_with(r"\\?\") && !mountpoints.iter().any(|m| m.eq_ignore_ascii_case(a)) {
                mountpoints.push(a.clone());
            }
        }
        let part = Partition {
            name: format!("PhysicalDrive{disk}-p{number}"),
            path: volume_path.unwrap_or_else(|| format!(r"\\.\PhysicalDrive{disk}-p{number}")),
            size_bytes: num(p, "Size").unwrap_or(0),
            fstype: vol
                .and_then(|v| text(v, "FileSystem"))
                .and_then(|f| fstype(&f)),
            label: vol.and_then(|v| text(v, "FileSystemLabel")),
            mountpoints,
        };
        by_disk.entry(disk).or_default().push((number, part));
    }

    let mut devices = Vec::new();
    for d in items(root.get("disks")) {
        let Some(n) = num(d, "Number") else { continue };
        let mut parts = by_disk.remove(&n).unwrap_or_default();
        parts.sort_by_key(|(k, _)| *k);
        let partitions: Vec<Partition> = parts.into_iter().map(|(_, p)| p).collect();
        let transport = d.get("BusType").map(bus_type).unwrap_or(Transport::Other);
        let system_disk = flag(d, "IsSystem")
            || flag(d, "IsBoot")
            || partitions
                .iter()
                .flat_map(|p| &p.mountpoints)
                .any(|m| system_letter.is_some() && letter_of(m) == system_letter);
        let usb = (transport == Transport::Usb).then(|| {
            let (vendor_id, product_id) = parents
                .and_then(|p| p.get(n.to_string()))
                .and_then(Value::as_str)
                .and_then(usb_ids)
                .unwrap_or_default();
            UsbInfo {
                vendor_id,
                product_id,
                manufacturer: None,
                product: None,
                usb_version: None,
                speed: UsbSpeed::Unknown,
            }
        });
        let sector = |k: &str| num(d, k).filter(|s| *s > 0).unwrap_or(512) as u32;
        devices.push(Device {
            name: format!("PhysicalDrive{n}"),
            path: format!(r"\\.\PhysicalDrive{n}"),
            size_bytes: num(d, "Size").unwrap_or(0),
            transport,
            removable: matches!(transport, Transport::Usb | Transport::Mmc),
            hotplug: matches!(transport, Transport::Usb | Transport::Mmc),
            read_only: flag(d, "IsReadOnly"),
            vendor: text(d, "Manufacturer"),
            model: text(d, "FriendlyName").or_else(|| text(d, "Model")),
            serial: text(d, "SerialNumber"),
            partition_table: partition_table(d.get("PartitionStyle")),
            logical_sector: sector("LogicalSectorSize"),
            physical_sector: sector("PhysicalSectorSize"),
            system_disk,
            mountpoints: vec![],
            partitions,
            usb,
        });
    }
    devices.sort_by_key(|d| d.name[13..].parse::<u64>().unwrap_or(u64::MAX));
    Ok(devices)
}

/// Aceita `1`, `pd1`, `PhysicalDrive1`, `\\.\PhysicalDrive1` ou uma letra (`E`, `E:`, `E:\`).
pub fn find<'a>(devices: &'a [Device], query: &str) -> Option<&'a Device> {
    let q = query.trim();
    let lower = q.to_ascii_lowercase();
    let number = lower
        .strip_prefix(r"\\.\")
        .unwrap_or(&lower)
        .strip_prefix("physicaldrive")
        .or_else(|| lower.strip_prefix("pd"))
        .unwrap_or(&lower);
    if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
        let name = format!("PhysicalDrive{number}");
        return devices.iter().find(|d| d.name == name);
    }
    let letter = match q.trim_end_matches('\\') {
        l if l.len() == 1 => l.chars().next(),
        l if l.len() == 2 && l.ends_with(':') => l.chars().next(),
        _ => None,
    }
    .filter(char::is_ascii_alphabetic)?
    .to_ascii_uppercase();
    devices
        .iter()
        .find(|d| d.all_mountpoints().any(|m| letter_of(m) == Some(letter)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use usbkeeper_core::Transport;

    fn desktop() -> Vec<Device> {
        parse(include_str!("../tests/fixtures/desktop.json")).unwrap()
    }

    #[test]
    fn maps_three_disks() {
        let d = desktop();
        assert_eq!(d.len(), 3);
        let nvme = &d[0];
        assert_eq!(
            (nvme.name.as_str(), nvme.path.as_str()),
            ("PhysicalDrive0", r"\\.\PhysicalDrive0")
        );
        assert_eq!(nvme.transport, Transport::Nvme);
        assert!(nvme.system_disk && !nvme.removable);
        assert_eq!(nvme.partition_table.as_deref(), Some("gpt"));
        assert_eq!(
            nvme.serial.as_deref(),
            Some("E823_8FA6_BF53_0001_001B_448B_4A1C_7E33.")
        );
        assert_eq!(nvme.partitions.len(), 3);
        assert_eq!(nvme.partitions[2].mountpoints, vec![r"C:\".to_string()]);
        assert_eq!(nvme.partitions[2].fstype.as_deref(), Some("ntfs"));
        assert_eq!(nvme.partitions[2].label.as_deref(), Some("Windows"));
        assert!(nvme.partitions[1].fstype.is_none() && nvme.partitions[1].mountpoints.is_empty());
    }

    #[test]
    fn usb_pendrive() {
        let pen = &desktop()[1];
        assert_eq!(pen.transport, Transport::Usb);
        assert!(pen.removable && pen.hotplug && !pen.system_disk);
        assert_eq!(pen.vendor.as_deref(), Some("Kingston"));
        assert_eq!(pen.model.as_deref(), Some("Kingston DataTraveler 3.0"));
        assert_eq!(
            pen.serial.as_deref(),
            Some("60A44C4138B3E361B9750A3F"),
            "série aparada"
        );
        assert_eq!(
            pen.partition_table.as_deref(),
            Some("dos"),
            "PartitionStyle numérico 1 = MBR"
        );
        let usb = pen.usb.as_ref().unwrap();
        assert_eq!(
            (usb.vendor_id.as_str(), usb.product_id.as_str()),
            ("0951", "1666")
        );
        let p = &pen.partitions[0];
        assert_eq!(p.name, "PhysicalDrive1-p1");
        assert_eq!(p.path, r"\\?\Volume{bbbb0001-0000-0000-0000-000000000000}\");
        assert_eq!(
            (p.fstype.as_deref(), p.label.as_deref()),
            (Some("vfat"), Some("MEU PEN"))
        );
        assert_eq!(p.mountpoints, vec![r"E:\".to_string()]);
    }

    #[test]
    fn virtual_disk_and_folder_mount() {
        let vhd = &desktop()[2];
        assert_eq!(vhd.transport, Transport::Virtual);
        assert_eq!(
            (vhd.vendor.as_deref(), vhd.serial.as_deref()),
            (Some("Msft"), None)
        );
        assert_eq!(vhd.physical_sector, 4096);
        assert_eq!(
            vhd.partitions[1].mountpoints,
            vec![r"V:\".to_string(), r"D:\montado\vhd\".to_string()]
        );
        assert!(vhd.usb.is_none());
    }

    #[test]
    fn single_objects_and_nulls() {
        let d = parse(include_str!("../tests/fixtures/single.json")).unwrap();
        assert_eq!(d.len(), 1);
        let ssd = &d[0];
        assert_eq!(ssd.transport, Transport::Sata, "BusType 11");
        assert!(ssd.system_disk, "contém a %SystemDrive%");
        assert_eq!((ssd.logical_sector, ssd.physical_sector), (512, 512));
        assert_eq!((ssd.vendor.as_deref(), ssd.serial.as_deref()), (None, None));
        assert_eq!(ssd.model.as_deref(), Some("Samsung SSD 870"));
        assert_eq!(ssd.partitions[0].mountpoints, vec![r"C:\".to_string()]);
        assert!(ssd.partitions[0].fstype.is_none());
    }

    #[test]
    fn find_by_number_name_path_and_letter() {
        let d = desktop();
        for q in [
            "1",
            "pd1",
            "PD1",
            "PhysicalDrive1",
            "physicaldrive1",
            r"\\.\PhysicalDrive1",
            "E:",
            "e:",
            r"E:\",
            "e",
        ] {
            assert_eq!(
                find(&d, q).map(|x| x.name.as_str()),
                Some("PhysicalDrive1"),
                "{q}"
            );
        }
        assert_eq!(
            find(&d, "c:").map(|x| x.name.as_str()),
            Some("PhysicalDrive0")
        );
        assert!(find(&d, "9").is_none() && find(&d, "Z:").is_none() && find(&d, "sdb").is_none());
    }

    #[test]
    fn bus_types_by_name_and_number() {
        for (v, t) in [
            ("USB", Transport::Usb),
            ("7", Transport::Usb),
            ("SD", Transport::Mmc),
            ("13", Transport::Mmc),
            ("ATA", Transport::Sata),
            ("SAS", Transport::Scsi),
            ("RAID", Transport::Scsi),
            ("15", Transport::Virtual),
            ("Virtual", Transport::Virtual),
            ("17", Transport::Nvme),
            ("Storage Spaces", Transport::Other),
        ] {
            assert_eq!(bus_type(&serde_json::json!(v)), t, "{v}");
        }
        assert_eq!(bus_type(&serde_json::json!(7)), Transport::Usb);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(parse("não é json").is_err());
        assert!(parse("{}").unwrap().is_empty());
    }
}
