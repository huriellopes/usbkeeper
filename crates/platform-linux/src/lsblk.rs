//! Interpreta `lsblk -J -b -o <COLUMNS>`. Puro (recebe a string JSON) para
//! poder ser testado com fixtures sem tocar em discos reais.

use serde::{Deserialize, Deserializer};
use usbkeeper_core::{CoreError, Device, Partition, Transport};

pub const COLUMNS: &str = "NAME,PATH,TYPE,SIZE,TRAN,RM,HOTPLUG,VENDOR,MODEL,SERIAL,FSTYPE,LABEL,MOUNTPOINTS,RO,PTTYPE,LOG-SEC,PHY-SEC";

#[derive(Debug, Deserialize)]
struct Root {
    blockdevices: Vec<Node>,
}

#[derive(Debug, Deserialize)]
struct Node {
    name: String,
    path: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "flex_u64")]
    size: u64,
    tran: Option<String>,
    #[serde(default, deserialize_with = "flex_bool")]
    rm: bool,
    #[serde(default, deserialize_with = "flex_bool")]
    hotplug: bool,
    vendor: Option<String>,
    model: Option<String>,
    serial: Option<String>,
    fstype: Option<String>,
    label: Option<String>,
    #[serde(default)]
    mountpoints: Vec<Option<String>>,
    // lsblk antigo (< 2.37) só tem MOUNTPOINT no singular.
    mountpoint: Option<String>,
    #[serde(default, deserialize_with = "flex_bool")]
    ro: bool,
    pttype: Option<String>,
    #[serde(rename = "log-sec", default, deserialize_with = "flex_u64")]
    log_sec: u64,
    #[serde(rename = "phy-sec", default, deserialize_with = "flex_u64")]
    phy_sec: u64,
    #[serde(default)]
    children: Vec<Node>,
}

impl Node {
    fn own_mountpoints(&self) -> impl Iterator<Item = String> + '_ {
        self.mountpoints
            .iter()
            .flatten()
            .chain(self.mountpoint.iter())
            .filter(|m| !m.is_empty())
            .cloned()
    }

    /// Montagens deste nó e de toda a pilha acima dele (crypt, lvm, md...).
    fn subtree_mountpoints(&self) -> Vec<String> {
        let mut out: Vec<String> = self.own_mountpoints().collect();
        for child in &self.children {
            out.extend(child.subtree_mountpoints());
        }
        dedup(out)
    }

    fn path(&self) -> String {
        self.path
            .clone()
            .unwrap_or_else(|| format!("/dev/{}", self.name))
    }
}

fn dedup(mut v: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.retain(|m| seen.insert(m.clone()));
    v
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn transport(node: &Node) -> Transport {
    match node.tran.as_deref().map(str::to_ascii_lowercase).as_deref() {
        Some("usb") => Transport::Usb,
        Some("sata" | "ata") => Transport::Sata,
        Some("nvme") => Transport::Nvme,
        Some("mmc") => Transport::Mmc,
        Some("sas" | "spi" | "fc" | "iscsi" | "scsi") => Transport::Scsi,
        Some(_) => Transport::Other,
        None if node.kind == "loop"
            || ["loop", "zram", "ram", "nbd"]
                .iter()
                .any(|p| node.name.starts_with(p)) =>
        {
            Transport::Virtual
        }
        None => Transport::Other,
    }
}

fn to_device(node: Node) -> Device {
    let transport = transport(&node);
    let mut partitions = Vec::new();
    let mut stacked = node.own_mountpoints().collect::<Vec<_>>();
    for child in &node.children {
        if child.kind == "part" {
            partitions.push(Partition {
                name: child.name.clone(),
                path: child.path(),
                size_bytes: child.size,
                fstype: clean(child.fstype.clone()),
                label: clean(child.label.clone()),
                mountpoints: child.subtree_mountpoints(),
            });
        } else {
            // LUKS/LVM direto no disco, sem tabela de partição.
            stacked.extend(child.subtree_mountpoints());
        }
    }
    Device {
        path: node.path(),
        size_bytes: node.size,
        transport,
        removable: node.rm,
        hotplug: node.hotplug,
        read_only: node.ro,
        // "ATA" é o rótulo genérico da camada SCSI, não o fabricante.
        vendor: clean(node.vendor).filter(|v| v != "ATA"),
        model: clean(node.model),
        serial: clean(node.serial),
        partition_table: clean(node.pttype),
        logical_sector: node.log_sec as u32,
        physical_sector: node.phy_sec as u32,
        mountpoints: dedup(stacked),
        partitions,
        usb: None,
        name: node.name,
    }
}

/// Converte a saída do lsblk em discos (tipos `disk` e `loop`).
pub fn parse(json: &str) -> Result<Vec<Device>, CoreError> {
    let root: Root = serde_json::from_str(json).map_err(|e| CoreError::Parse(e.to_string()))?;
    Ok(root
        .blockdevices
        .into_iter()
        .filter(|n| n.kind == "disk" || n.kind == "loop")
        .map(to_device)
        .collect())
}

fn flex_u64<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flex {
        Num(u64),
        Str(String),
        Null,
    }
    Ok(match Flex::deserialize(d)? {
        Flex::Num(n) => n,
        Flex::Str(s) => s.trim().parse().unwrap_or(0),
        Flex::Null => 0,
    })
}

fn flex_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flex {
        Bool(bool),
        Num(u8),
        Str(String),
        Null,
    }
    Ok(match Flex::deserialize(d)? {
        Flex::Bool(b) => b,
        Flex::Num(n) => n != 0,
        Flex::Str(s) => s.trim() == "1" || s.trim().eq_ignore_ascii_case("true"),
        Flex::Null => false,
    })
}
