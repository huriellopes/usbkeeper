//! "Desmontar" no Windows: travar e desmontar os volumes do disco.

use usbkeeper_core::{CoreError, Device, Holder, Mounts};

use crate::paths::disk_number;
use crate::rawdev::lock_volumes;

/// `mount_all` (atribuir letra) chega na M4b2.
pub struct WindowsMounts {
    pub mount_unsupported: &'static str,
}

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

    fn mount_all(&self, _device: &Device) -> Result<Vec<String>, CoreError> {
        Err(CoreError::Backend(self.mount_unsupported.into()))
    }
}
