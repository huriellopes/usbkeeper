//! SafetyGuard: decide se um disco pode ser alvo de operação destrutiva.
//!
//! Regras (não negociáveis):
//! - disco do sistema nunca é alvo, sem exceção nem flag;
//! - só discos USB são aceitos, a menos que o usuário peça `allow_non_usb`;
//! - disco montado precisa ser desmontado antes;
//! - a confirmação é digitando o número de série (ou o caminho, se não houver);
//! - imediatamente antes de gravar, a identidade é conferida de novo.

use serde::Serialize;

use crate::model::{Device, DeviceIdentity, Transport};

/// Pontos de montagem que caracterizam o disco do sistema.
/// Comparação exata: um pendrive montado em `/home/fulano/usb` não é sistema.
pub const SYSTEM_MOUNTPOINTS: &[&str] = &[
    "/",
    "/boot",
    "/boot/efi",
    "/efi",
    "/usr",
    "/var",
    "/home",
    "/opt",
    "/srv",
    "[SWAP]",
];

#[derive(Debug, Clone, Copy, Default)]
pub struct Policy {
    pub allow_non_usb: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Refusal {
    SystemDisk { mountpoint: String },
    NotUsb { transport: Transport },
    Mounted { mountpoints: Vec<String> },
    ReadOnly,
    Empty,
}

impl Refusal {
    /// Recusas que nenhuma flag ou ação do usuário libera.
    pub fn is_absolute(&self) -> bool {
        matches!(self, Refusal::SystemDisk { .. })
    }

    pub fn message(&self) -> String {
        match self {
            Refusal::SystemDisk { mountpoint } => {
                format!("é o disco do sistema (montado em {mountpoint}); nunca será alvo")
            }
            Refusal::NotUsb { transport } => format!(
                "não é USB (barramento {}); use --allow-non-usb se tiver certeza",
                transport.label()
            ),
            Refusal::Mounted { mountpoints } => {
                format!("está montado em {}; desmonte antes", mountpoints.join(", "))
            }
            Refusal::ReadOnly => "está somente leitura (trava física ou do sistema)".into(),
            Refusal::Empty => "tem tamanho 0 (sem mídia inserida?)".into(),
        }
    }
}

pub fn system_mountpoint(device: &Device) -> Option<&str> {
    device
        .all_mountpoints()
        .find(|m| SYSTEM_MOUNTPOINTS.contains(m))
}

/// Lista todas as razões que impedem gravar no disco. Vazio = liberado.
pub fn check_target(device: &Device, policy: Policy) -> Vec<Refusal> {
    let mut refusals = Vec::new();

    if let Some(mountpoint) = system_mountpoint(device) {
        refusals.push(Refusal::SystemDisk {
            mountpoint: mountpoint.to_string(),
        });
    }
    if device.transport != Transport::Usb && !policy.allow_non_usb {
        refusals.push(Refusal::NotUsb {
            transport: device.transport,
        });
    }
    let mounted: Vec<String> = device.all_mountpoints().map(String::from).collect();
    if !mounted.is_empty() && refusals.iter().all(|r| !r.is_absolute()) {
        refusals.push(Refusal::Mounted {
            mountpoints: mounted,
        });
    }
    if device.read_only {
        refusals.push(Refusal::ReadOnly);
    }
    if device.size_bytes == 0 {
        refusals.push(Refusal::Empty);
    }
    refusals
}

/// O texto que o usuário precisa digitar para confirmar: o número de série,
/// ou o caminho do dispositivo quando ele não tem série (comum em falsificados).
pub fn confirmation_token(device: &Device) -> &str {
    device
        .serial
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&device.path)
}

pub fn confirm(device: &Device, typed: &str) -> bool {
    typed.trim() == confirmation_token(device)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityMismatch {
    pub expected: DeviceIdentity,
    pub found: DeviceIdentity,
}

/// Confere, logo antes de gravar, que o caminho ainda aponta para o mesmo
/// disco confirmado (o pendrive pode ter sido trocado e o nome reaproveitado).
pub fn verify_same(expected: &DeviceIdentity, current: &Device) -> Result<(), IdentityMismatch> {
    let found = current.identity();
    if &found == expected {
        Ok(())
    } else {
        Err(IdentityMismatch {
            expected: expected.clone(),
            found,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Partition;

    fn part(name: &str, mounts: &[&str]) -> Partition {
        Partition {
            name: name.into(),
            path: format!("/dev/{name}"),
            size_bytes: 1_000_000,
            fstype: Some("vfat".into()),
            label: None,
            mountpoints: mounts.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn disk(transport: Transport, partitions: Vec<Partition>) -> Device {
        Device {
            name: "sdb".into(),
            path: "/dev/sdb".into(),
            size_bytes: 16_000_000_000,
            transport,
            removable: true,
            hotplug: true,
            read_only: false,
            vendor: Some("Kingston".into()),
            model: Some("DataTraveler 3.0".into()),
            serial: Some("SERIAL123".into()),
            partition_table: Some("dos".into()),
            logical_sector: 512,
            physical_sector: 512,
            mountpoints: vec![],
            partitions,
            usb: None,
        }
    }

    #[test]
    fn unmounted_usb_is_allowed() {
        let d = disk(Transport::Usb, vec![part("sdb1", &[])]);
        assert!(check_target(&d, Policy::default()).is_empty());
    }

    #[test]
    fn system_disk_is_refused_even_with_allow_non_usb() {
        let d = disk(Transport::Nvme, vec![part("nvme0n1p2", &["/"])]);
        let r = check_target(
            &d,
            Policy {
                allow_non_usb: true,
            },
        );
        assert!(r.iter().any(Refusal::is_absolute));
    }

    #[test]
    fn usb_disk_holding_root_is_system() {
        // Sistema rodando de um SSD externo USB continua sendo o sistema.
        let d = disk(Transport::Usb, vec![part("sdb2", &["/boot/efi"])]);
        let r = check_target(&d, Policy::default());
        assert_eq!(
            r,
            vec![Refusal::SystemDisk {
                mountpoint: "/boot/efi".into()
            }]
        );
    }

    #[test]
    fn swap_on_whole_disk_is_system() {
        let mut d = disk(Transport::Usb, vec![]);
        d.mountpoints = vec!["[SWAP]".into()];
        assert!(check_target(&d, Policy::default())[0].is_absolute());
    }

    #[test]
    fn similar_paths_are_not_system() {
        let d = disk(Transport::Usb, vec![part("sdb1", &["/home/ana/pendrive"])]);
        let r = check_target(&d, Policy::default());
        assert_eq!(
            r,
            vec![Refusal::Mounted {
                mountpoints: vec!["/home/ana/pendrive".into()]
            }]
        );
    }

    #[test]
    fn non_usb_requires_flag() {
        let d = disk(Transport::Sata, vec![part("sda1", &[])]);
        assert_eq!(
            check_target(&d, Policy::default()),
            vec![Refusal::NotUsb {
                transport: Transport::Sata
            }]
        );
        assert!(
            check_target(
                &d,
                Policy {
                    allow_non_usb: true
                }
            )
            .is_empty()
        );
    }

    #[test]
    fn read_only_and_empty_are_refused() {
        let mut d = disk(Transport::Usb, vec![]);
        d.read_only = true;
        d.size_bytes = 0;
        let r = check_target(&d, Policy::default());
        assert!(r.contains(&Refusal::ReadOnly));
        assert!(r.contains(&Refusal::Empty));
    }

    #[test]
    fn confirmation_uses_serial_or_path() {
        let mut d = disk(Transport::Usb, vec![]);
        assert!(confirm(&d, " SERIAL123\n"));
        assert!(!confirm(&d, "/dev/sdb"));
        d.serial = Some("   ".into());
        assert_eq!(confirmation_token(&d), "/dev/sdb");
        assert!(confirm(&d, "/dev/sdb"));
    }

    #[test]
    fn swapped_device_fails_identity_check() {
        let d = disk(Transport::Usb, vec![]);
        let expected = d.identity();
        assert!(verify_same(&expected, &d).is_ok());

        let mut other = d.clone();
        other.serial = Some("OUTRO".into());
        assert!(verify_same(&expected, &other).is_err());

        let mut resized = d.clone();
        resized.size_bytes -= 512;
        assert!(verify_same(&expected, &resized).is_err());
    }
}
