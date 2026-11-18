#![cfg(target_os = "linux")]
//! Backend Linux: `lsblk` para a topologia e sysfs para os dados USB.

pub mod format;
pub mod lsblk;
pub mod mounts;
pub mod rawdev;
pub mod sysfs;
pub mod wsl;

use std::path::PathBuf;
use std::process::Command;

use usbkeeper_core::{CoreError, Device, Enumerator, Transport};

pub use format::LinuxFormatter;
pub use mounts::{LinuxEjector, LinuxMounts, is_root};
pub use rawdev::RawDevice;

pub struct LinuxEnumerator {
    pub sys_root: PathBuf,
    /// Inclui loop/zram/ram (útil para testes com `losetup`).
    pub include_virtual: bool,
}

impl Default for LinuxEnumerator {
    fn default() -> Self {
        Self {
            sys_root: PathBuf::from("/sys"),
            include_virtual: false,
        }
    }
}

impl LinuxEnumerator {
    fn run_lsblk() -> Result<String, CoreError> {
        let out = Command::new("lsblk")
            .args(["-J", "-b", "-o", lsblk::COLUMNS])
            .output()
            .map_err(|e| CoreError::Backend(format!("não foi possível executar lsblk: {e}")))?;
        if !out.status.success() {
            return Err(CoreError::Backend(format!(
                "lsblk falhou: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        String::from_utf8(out.stdout).map_err(|e| CoreError::Parse(e.to_string()))
    }

    /// Junta lsblk + sysfs e aplica o filtro de dispositivos virtuais.
    pub fn enrich(&self, devices: Vec<Device>) -> Vec<Device> {
        devices
            .into_iter()
            .filter(|d| self.include_virtual || d.transport != Transport::Virtual)
            .map(|mut d| {
                d.usb = sysfs::usb_info(&self.sys_root, &d.name);
                // Algumas versões do lsblk não preenchem TRAN para leitores
                // de cartão; o sysfs é a fonte de verdade sobre ser USB.
                if d.usb.is_some() {
                    d.transport = Transport::Usb;
                }
                d
            })
            .collect()
    }
}

impl Enumerator for LinuxEnumerator {
    fn list(&self) -> Result<Vec<Device>, CoreError> {
        let json = Self::run_lsblk()?;
        Ok(self.enrich(lsblk::parse(&json)?))
    }
}
