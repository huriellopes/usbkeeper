//! Assistente de formatação: recomenda sistema de arquivos, tabela e limite
//! a partir do uso pretendido e do que já se sabe do disco.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::format::{Filesystem, PartitionTable};
use crate::model::Device;

const TWO_TIB: u64 = 2 * 1024 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Usage {
    Mixed,
    WindowsOnly,
    LinuxOnly,
    Appliance,
    Console,
}

impl FromStr for Usage {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "misto" | "mixed" => Ok(Self::Mixed),
            "windows" => Ok(Self::WindowsOnly),
            "linux" => Ok(Self::LinuxOnly),
            "aparelho" | "tv" => Ok(Self::Appliance),
            "videogame" | "console" => Ok(Self::Console),
            o => Err(format!(
                "uso desconhecido: {o} (use misto, windows, linux, aparelho ou videogame)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum BigFiles {
    Yes,
    No,
    Unknown,
}

impl FromStr for BigFiles {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "sim" | "yes" => Ok(Self::Yes),
            "nao" | "não" | "no" => Ok(Self::No),
            "nao-sei" | "não-sei" | "unknown" => Ok(Self::Unknown),
            o => Err(format!(
                "resposta desconhecida: {o} (use sim, nao ou nao-sei)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskFacts {
    pub size_bytes: u64,
    pub removable: bool,
    /// Último fake-check deu falso: (capacidade real, limite sugerido).
    pub fake: Option<(u64, u64)>,
    /// Último veredito de saúde: "good" | "warning" | "failing" | "unknown".
    pub health: Option<String>,
    pub usb_bottleneck: bool,
}

impl DiskFacts {
    pub fn of(device: &Device, fake: Option<(u64, u64)>, health: Option<String>) -> Self {
        Self {
            size_bytes: device.size_bytes,
            removable: device.removable,
            fake,
            health,
            usb_bottleneck: device.usb.as_ref().is_some_and(|u| u.is_bottlenecked()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Advice {
    pub filesystem: Filesystem,
    pub table: PartitionTable,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub max_bytes: Option<u64>,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn advise(usage: Usage, big: BigFiles, facts: &DiskFacts) -> Advice {
    let mut reasons: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let filesystem = match usage {
        Usage::Appliance => {
            reasons.push(
                "FAT32 é o formato que TVs, sons de carro e aparelhos antigos leem com mais segurança."
                    .into(),
            );
            if big == BigFiles::Yes {
                warnings.push(
                    "Arquivos acima de 4 GB não cabem em FAT32; só alguns aparelhos novos leem exFAT. Teste no aparelho antes."
                        .into(),
                );
            }
            Filesystem::Fat32
        }
        Usage::Mixed => {
            reasons.push(
                "exFAT funciona em Windows, macOS e Linux e aceita arquivos acima de 4 GB.".into(),
            );
            Filesystem::Exfat
        }
        Usage::WindowsOnly if !facts.removable => {
            reasons.push(
                "NTFS é o formato nativo do Windows e tem journaling, que protege melhor um HD/SSD externo contra quedas de energia."
                    .into(),
            );
            Filesystem::Ntfs
        }
        Usage::WindowsOnly => {
            reasons.push(
                "Num pendrive ou cartão, exFAT é mais leve que NTFS e gasta menos a memória flash; o Windows lê e grava normalmente."
                    .into(),
            );
            Filesystem::Exfat
        }
        Usage::LinuxOnly => {
            reasons.push(
                "ext4 é o formato nativo do Linux: guarda permissões e tem journaling. Windows e macOS não o leem sem programas extras."
                    .into(),
            );
            Filesystem::Ext4
        }
        Usage::Console => {
            reasons
                .push("Os consoles atuais leem exFAT; alguns modelos antigos exigem FAT32.".into());
            Filesystem::Exfat
        }
    };

    let (table, max_bytes) = if let Some((real, suggested)) = facts.fake {
        reasons.push(format!(
            "O último teste mostrou que o dispositivo é falsificado: só {:.1} GB existem de verdade. A formatação usa só essa parte, com MBR (a GPT grava no fim do disco, que não existe).",
            real as f64 / 1e9
        ));
        warnings.push(
            "Mesmo reformatado, não use um dispositivo falsificado para a única cópia de nada importante."
                .into(),
        );
        (PartitionTable::Mbr, Some(suggested))
    } else if facts.size_bytes > TWO_TIB {
        reasons.push("Acima de 2 TB é preciso GPT: o MBR não endereça além disso.".into());
        if usage == Usage::Appliance {
            warnings.push(
                "Muitos aparelhos não leem discos acima de 2 TB ou com GPT; prefira um disco menor para a TV."
                    .into(),
            );
        }
        (PartitionTable::Gpt, None)
    } else {
        reasons.push("MBR é a tabela mais compatível para discos de até 2 TB.".into());
        (PartitionTable::Mbr, None)
    };

    match facts.health.as_deref() {
        Some("failing") => warnings
            .push("O SMART indica falha iminente: não guarde nada importante neste disco.".into()),
        Some("warning") => warnings
            .push("O SMART mostra sinais de desgaste: mantenha backup do que guardar aqui.".into()),
        _ => {}
    }
    if facts.usb_bottleneck {
        warnings
            .push("Está conectado como USB 2.0; numa porta USB 3 ele fica bem mais rápido.".into());
    }

    Advice {
        filesystem,
        table,
        max_bytes,
        reasons,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const GB: u64 = 1_000_000_000;

    fn facts(size: u64, removable: bool) -> DiskFacts {
        DiskFacts {
            size_bytes: size,
            removable,
            fake: None,
            health: None,
            usb_bottleneck: false,
        }
    }

    #[test]
    fn filesystem_by_usage() {
        let pen = facts(64 * GB, true);
        let hd = facts(1000 * GB, false);
        let fs = |u, f: &DiskFacts| advise(u, BigFiles::Unknown, f).filesystem;
        assert_eq!(fs(Usage::Appliance, &pen), Filesystem::Fat32);
        assert_eq!(fs(Usage::Mixed, &pen), Filesystem::Exfat);
        assert_eq!(fs(Usage::WindowsOnly, &hd), Filesystem::Ntfs);
        assert_eq!(fs(Usage::WindowsOnly, &pen), Filesystem::Exfat);
        assert_eq!(fs(Usage::LinuxOnly, &pen), Filesystem::Ext4);
        assert_eq!(fs(Usage::Console, &pen), Filesystem::Exfat);
    }

    #[test]
    fn appliance_with_big_files_warns() {
        let a = advise(Usage::Appliance, BigFiles::Yes, &facts(64 * GB, true));
        assert_eq!(a.filesystem, Filesystem::Fat32);
        assert!(a.warnings.iter().any(|w| w.contains("4 GB")));
    }

    #[test]
    fn table_by_size() {
        assert_eq!(
            advise(Usage::Mixed, BigFiles::No, &facts(64 * GB, true)).table,
            PartitionTable::Mbr
        );
        let big = advise(Usage::Mixed, BigFiles::No, &facts(3000 * GB, false));
        assert_eq!(big.table, PartitionTable::Gpt);
        let tv = advise(Usage::Appliance, BigFiles::No, &facts(3000 * GB, false));
        assert!(tv.warnings.iter().any(|w| w.contains("2 TB")));
    }

    #[test]
    fn fake_limits_and_forces_mbr() {
        let mut f = facts(4000 * GB, true);
        f.fake = Some((8 * GB, 7_800_000_000));
        let a = advise(Usage::Mixed, BigFiles::No, &f);
        assert_eq!(a.max_bytes, Some(7_800_000_000));
        assert_eq!(a.table, PartitionTable::Mbr);
        assert!(a.reasons.iter().any(|r| r.contains("falsificado")));
        assert!(a.warnings.iter().any(|w| w.contains("única cópia")));
    }

    #[test]
    fn health_and_usb_warnings() {
        let mut f = facts(64 * GB, true);
        f.health = Some("failing".into());
        f.usb_bottleneck = true;
        let a = advise(Usage::Mixed, BigFiles::No, &f);
        assert!(a.warnings.iter().any(|w| w.contains("falha iminente")));
        assert!(a.warnings.iter().any(|w| w.contains("USB 2.0")));
    }

    #[test]
    fn parses_cli_words() {
        assert_eq!("tv".parse::<Usage>(), Ok(Usage::Appliance));
        assert_eq!("MISTO".parse::<Usage>(), Ok(Usage::Mixed));
        assert_eq!("não".parse::<BigFiles>(), Ok(BigFiles::No));
        assert!("xpto".parse::<Usage>().is_err());
    }
}
