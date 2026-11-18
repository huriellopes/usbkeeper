use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Usb,
    Sata,
    Nvme,
    Mmc,
    Scsi,
    Virtual,
    Other,
}

impl Transport {
    pub fn label(self) -> &'static str {
        match self {
            Transport::Usb => "USB",
            Transport::Sata => "SATA",
            Transport::Nvme => "NVMe",
            Transport::Mmc => "MMC/SD",
            Transport::Scsi => "SCSI",
            Transport::Virtual => "virtual",
            Transport::Other => "outro",
        }
    }
}

/// Velocidade USB negociada, em ordem crescente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbSpeed {
    Unknown,
    Low,
    Full,
    High,
    Super,
    SuperPlus,
    SuperPlus2x2,
}

impl UsbSpeed {
    /// Converte o valor de `/sys/bus/usb/devices/*/speed` (em Mb/s).
    pub fn from_mbps(raw: &str) -> Self {
        match raw.trim() {
            "1.5" => UsbSpeed::Low,
            "12" => UsbSpeed::Full,
            "480" => UsbSpeed::High,
            "5000" => UsbSpeed::Super,
            "10000" => UsbSpeed::SuperPlus,
            "20000" => UsbSpeed::SuperPlus2x2,
            _ => UsbSpeed::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            UsbSpeed::Unknown => "desconhecida",
            UsbSpeed::Low => "USB 1.0 (1,5 Mb/s)",
            UsbSpeed::Full => "USB 1.1 (12 Mb/s)",
            UsbSpeed::High => "USB 2.0 (480 Mb/s)",
            UsbSpeed::Super => "USB 3.2 Gen 1 (5 Gb/s)",
            UsbSpeed::SuperPlus => "USB 3.2 Gen 2 (10 Gb/s)",
            UsbSpeed::SuperPlus2x2 => "USB 3.2 Gen 2x2 (20 Gb/s)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbInfo {
    pub vendor_id: String,
    pub product_id: String,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    /// Versão USB declarada pelo dispositivo (bcdUSB), ex.: "3.20".
    pub usb_version: Option<String>,
    pub speed: UsbSpeed,
}

impl UsbInfo {
    /// O dispositivo declara USB 3.x mas negociou só até USB 2.0:
    /// porta, hub ou cabo está limitando a velocidade.
    pub fn is_bottlenecked(&self) -> bool {
        let major = self
            .usb_version
            .as_deref()
            .and_then(|v| v.trim().split('.').next())
            .and_then(|m| m.parse::<u32>().ok());
        matches!(major, Some(m) if m >= 3)
            && self.speed != UsbSpeed::Unknown
            && self.speed <= UsbSpeed::High
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Partition {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub fstype: Option<String>,
    pub label: Option<String>,
    /// Pontos de montagem da partição e de tudo empilhado sobre ela
    /// (LUKS, LVM), para que um root criptografado não passe despercebido.
    pub mountpoints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Device {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub transport: Transport,
    pub removable: bool,
    pub hotplug: bool,
    pub read_only: bool,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub partition_table: Option<String>,
    pub logical_sector: u32,
    pub physical_sector: u32,
    /// Montagens do disco inteiro (sem tabela de partição) e de camadas
    /// empilhadas diretamente sobre ele.
    pub mountpoints: Vec<String>,
    pub partitions: Vec<Partition>,
    pub usb: Option<UsbInfo>,
}

impl Device {
    pub fn all_mountpoints(&self) -> impl Iterator<Item = &str> {
        self.mountpoints
            .iter()
            .chain(self.partitions.iter().flat_map(|p| p.mountpoints.iter()))
            .map(String::as_str)
    }

    pub fn is_mounted(&self) -> bool {
        self.all_mountpoints().next().is_some()
    }

    /// Nome amigável: fabricante + modelo, com fallback para o USB.
    pub fn display_name(&self) -> String {
        let parts: Vec<&str> = [self.vendor.as_deref(), self.model.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        if !parts.is_empty() {
            return parts.join(" ");
        }
        self.usb
            .as_ref()
            .and_then(|u| u.product.clone())
            .unwrap_or_else(|| "(sem modelo)".to_string())
    }

    pub fn identity(&self) -> DeviceIdentity {
        DeviceIdentity {
            path: self.path.clone(),
            serial: self.serial.clone(),
            size_bytes: self.size_bytes,
        }
    }
}

/// O que precisa continuar igual entre a confirmação e a gravação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceIdentity {
    pub path: String,
    pub serial: Option<String>,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb(version: Option<&str>, speed: UsbSpeed) -> UsbInfo {
        UsbInfo {
            vendor_id: "0951".into(),
            product_id: "1666".into(),
            manufacturer: None,
            product: None,
            usb_version: version.map(String::from),
            speed,
        }
    }

    #[test]
    fn parses_sysfs_speed() {
        assert_eq!(UsbSpeed::from_mbps("480\n"), UsbSpeed::High);
        assert_eq!(UsbSpeed::from_mbps("5000"), UsbSpeed::Super);
        assert_eq!(UsbSpeed::from_mbps("lixo"), UsbSpeed::Unknown);
    }

    #[test]
    fn usb3_device_on_usb2_port_is_bottlenecked() {
        assert!(usb(Some(" 3.20"), UsbSpeed::High).is_bottlenecked());
        assert!(!usb(Some("3.20"), UsbSpeed::Super).is_bottlenecked());
        assert!(!usb(Some("2.00"), UsbSpeed::High).is_bottlenecked());
        assert!(!usb(None, UsbSpeed::High).is_bottlenecked());
        assert!(!usb(Some("3.00"), UsbSpeed::Unknown).is_bottlenecked());
    }
}
