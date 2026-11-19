//! Enumerador: roda o script de Storage do PowerShell e interpreta.

use std::os::windows::process::CommandExt;
use std::process::Command;

use usbkeeper_core::{CoreError, Device, Enumerator, Transport};

use crate::storage::{SCRIPT, find, parse};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Default, Clone)]
pub struct WindowsEnumerator {
    /// Inclui discos virtuais (VHD), úteis em testes.
    pub include_virtual: bool,
}

fn run_script() -> Result<String, CoreError> {
    let out = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            SCRIPT,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| CoreError::Backend(format!("não consegui executar o PowerShell: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("erro desconhecido");
        return Err(CoreError::Backend(format!(
            "PowerShell (Get-Disk) falhou: {}",
            first.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

impl WindowsEnumerator {
    fn all(&self) -> Result<Vec<Device>, CoreError> {
        let mut devices = parse(&run_script()?)?;
        if !self.include_virtual {
            devices.retain(|d| d.transport != Transport::Virtual);
        }
        Ok(devices)
    }
}

impl Enumerator for WindowsEnumerator {
    fn list(&self) -> Result<Vec<Device>, CoreError> {
        self.all()
    }

    fn get(&self, query: &str) -> Result<Device, CoreError> {
        // Busca também entre os virtuais: quem nomeia o disco sabe o que quer.
        let all = parse(&run_script()?)?;
        find(&all, query)
            .cloned()
            .ok_or_else(|| CoreError::NotFound(query.to_string()))
    }
}
