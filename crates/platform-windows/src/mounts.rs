//! Montar e "desmontar" no Windows: atribuir letras; travar e desmontar os
//! volumes do disco.

use std::os::windows::process::CommandExt;
use std::process::Command;

use usbkeeper_core::{CoreError, Device, Holder, Mounts};

use crate::paths::disk_number;
use crate::rawdev::lock_volumes;

pub struct WindowsMounts;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

impl Mounts for WindowsMounts {
    /// Verificação antecipada: trava e desmonta (e solta) para recusar um
    /// volume em uso antes de pedir a série. A trava que vale para gravar é
    /// tomada de novo ao abrir o disco (`RawDevice`), e mantida até o fim.
    fn unmount_all(&self, device: &Device) -> Result<(), CoreError> {
        let n = disk_number(&device.path)
            .ok_or_else(|| CoreError::Backend(format!("caminho inesperado: {}", device.path)))?;
        drop(lock_volumes(n)?);
        Ok(())
    }

    /// O Windows não informa quais processos usam um volume.
    fn holders(&self, _device: &Device) -> Vec<Holder> {
        Vec::new()
    }

    /// Atribui uma letra às partições com sistema de arquivos e sem letra;
    /// devolve as letras de todas (para aparecerem no Explorer).
    fn mount_all(&self, device: &Device) -> Result<Vec<String>, CoreError> {
        let n = disk_number(&device.path)
            .ok_or_else(|| CoreError::Backend(format!("caminho inesperado: {}", device.path)))?;
        let script = format!(
            "$ErrorActionPreference='Stop'; $out = @(); \
             foreach ($p in Get-Partition -DiskNumber {n} | Where-Object {{ $_.Type -notin 'Reserved','System','Recovery','Unknown' }}) {{ \
               $v = $p | Get-Volume -ErrorAction SilentlyContinue; \
               if (-not $v -or -not $v.FileSystem) {{ continue }}; \
               if (-not $p.DriveLetter -or [int][char]$p.DriveLetter -eq 0) {{ \
                 $p | Add-PartitionAccessPath -AssignDriveLetter; \
                 $p = Get-Partition -DiskNumber {n} -PartitionNumber $p.PartitionNumber }}; \
               $out += ([string]$p.DriveLetter + ':\\') }}; \
             $out -join [Environment]::NewLine"
        );
        let out = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let first = err
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("erro desconhecido");
            return Err(CoreError::Backend(format!(
                "não consegui atribuir letra: {first}"
            )));
        }
        let letters: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| l.len() >= 2)
            .map(String::from)
            .collect();
        if letters.is_empty() {
            return Err(CoreError::Backend(
                "nenhuma partição com sistema de arquivos reconhecido para montar".into(),
            ));
        }
        Ok(letters)
    }
}
