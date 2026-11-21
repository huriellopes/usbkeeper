//! Remover com segurança: o mesmo pedido do ícone "Remover hardware com
//! segurança" (`CM_Request_Device_EjectW`), no ancestral removível do disco.

/// Id da instância PnP a partir do `Path` do `Get-Disk`:
/// `\\?\usbstor#disk&ven_x&prod_y#serial&0#{guid}` → `usbstor\disk&ven_x&prod_y\serial&0`.
pub fn instance_id(disk_path: &str) -> Option<String> {
    let rest = disk_path.trim().strip_prefix(r"\\?\")?;
    let without_guid = match rest.rfind("#{") {
        Some(i) => &rest[..i],
        None => rest,
    };
    let id = without_guid.replace('#', r"\");
    (id.matches('\\').count() >= 2).then_some(id)
}

#[cfg(windows)]
mod run {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    use usbkeeper_core::{CoreError, Device, Ejector};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_DEVCAP_REMOVABLE, CM_DRP_CAPABILITIES, CM_Get_DevNode_Registry_PropertyW, CM_Get_Parent,
        CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CM_Request_Device_EjectW, CR_SUCCESS,
        PNP_VETO_TYPE, PNP_VetoTypeUnknown,
    };
    use windows::core::PCWSTR;

    use super::instance_id;
    use crate::paths::disk_number;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const TRIES: u32 = 3;

    pub struct WindowsEjector;

    fn disk_path(n: u32) -> Result<String, CoreError> {
        let out = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!("(Get-Disk -Number {n}).Path"),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn removable(devinst: u32) -> bool {
        let mut caps = 0u32;
        let mut len = size_of::<u32>() as u32;
        // SAFETY: buffer local de 4 bytes e tamanho correspondente.
        let r = unsafe {
            CM_Get_DevNode_Registry_PropertyW(
                devinst,
                CM_DRP_CAPABILITIES,
                None,
                Some(&mut caps as *mut u32 as *mut _),
                &mut len,
                0,
            )
        };
        r == CR_SUCCESS && caps & CM_DEVCAP_REMOVABLE.0 != 0
    }

    impl Ejector for WindowsEjector {
        fn power_off(&self, device: &Device) -> Result<(), CoreError> {
            let n = disk_number(&device.path).ok_or_else(|| {
                CoreError::Backend(format!("caminho inesperado: {}", device.path))
            })?;
            let id = instance_id(&disk_path(n)?)
                .ok_or_else(|| CoreError::Backend("não achei o dispositivo PnP do disco".into()))?;
            let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
            let mut devinst = 0u32;
            // SAFETY: `wide` termina em zero e vive durante a chamada.
            let r = unsafe {
                CM_Locate_DevNodeW(
                    &mut devinst,
                    PCWSTR(wide.as_ptr()),
                    CM_LOCATE_DEVNODE_NORMAL,
                )
            };
            if r != CR_SUCCESS {
                return Err(CoreError::Backend(format!(
                    "dispositivo PnP não encontrado ({id})"
                )));
            }
            // Sobe até o ancestral removível (o pendrive/adaptador USB).
            let mut target = devinst;
            for _ in 0..4 {
                if removable(target) {
                    break;
                }
                let mut parent = 0u32;
                // SAFETY: saída numa variável local.
                if unsafe { CM_Get_Parent(&mut parent, target, 0) } != CR_SUCCESS {
                    break;
                }
                target = parent;
            }
            if !removable(target) {
                target = devinst;
            }
            let mut last = String::new();
            for attempt in 0..TRIES {
                let mut veto = PNP_VETO_TYPE::default();
                let mut name = [0u16; 260];
                // SAFETY: saídas locais com o tamanho do buffer.
                let r = unsafe {
                    CM_Request_Device_EjectW(target, Some(&mut veto), Some(&mut name), 0)
                };
                if r == CR_SUCCESS && veto == PNP_VetoTypeUnknown {
                    return Ok(());
                }
                let who = String::from_utf16_lossy(
                    &name[..name.iter().position(|&c| c == 0).unwrap_or(0)],
                );
                last = if who.is_empty() {
                    format!(
                        "o Windows recusou remover (código {}, veto {})",
                        r.0, veto.0
                    )
                } else {
                    format!("o Windows recusou remover: em uso por {who}")
                };
                if attempt + 1 < TRIES {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
            Err(CoreError::Backend(format!(
                "{last}. Feche as janelas e programas que usam o dispositivo e tente de novo"
            )))
        }
    }
}

#[cfg(windows)]
pub use run::WindowsEjector;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_ids() {
        assert_eq!(
            instance_id(r"\\?\usbstor#disk&ven_kingston&prod_datatraveler_3.0&rev_pmap#60a44c4138b3e361b9750a3f&0#{53f56307-b6bf-11d0-94f2-00a0c91efb8b}").as_deref(),
            Some(r"usbstor\disk&ven_kingston&prod_datatraveler_3.0&rev_pmap\60a44c4138b3e361b9750a3f&0")
        );
        assert_eq!(
            instance_id(r"\\?\scsi#disk&ven_msft&prod_virtual_disk#2&1f4adffe&0&000001#{53f56307-b6bf-11d0-94f2-00a0c91efb8b}").as_deref(),
            Some(r"scsi\disk&ven_msft&prod_virtual_disk\2&1f4adffe&0&000001")
        );
        assert_eq!(instance_id(r"\\.\PhysicalDrive1"), None);
        assert_eq!(instance_id(""), None);
    }
}
