//! Pedido de formatação e regras portáteis (rótulos, nomes de partição).

use std::fmt;
use std::str::FromStr;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Filesystem {
    Fat32,
    Exfat,
    Ntfs,
    Ext4,
}

impl Filesystem {
    pub const ALL: [Filesystem; 4] = [Self::Fat32, Self::Exfat, Self::Ntfs, Self::Ext4];

    pub fn name(self) -> &'static str {
        match self {
            Self::Fat32 => "fat32",
            Self::Exfat => "exfat",
            Self::Ntfs => "ntfs",
            Self::Ext4 => "ext4",
        }
    }

    /// Tamanho máximo do rótulo (em bytes) aceito pelo sistema de arquivos.
    pub fn max_label_len(self) -> usize {
        match self {
            Self::Fat32 => 11,
            Self::Exfat => 15,
            Self::Ntfs => 32,
            Self::Ext4 => 16,
        }
    }

    /// Uma linha para ajudar a escolher.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Fat32 => "compatível com tudo (TV, som, carro, BIOS); arquivos até 4 GB",
            Self::Exfat => "Windows/macOS/Linux, sem limite de 4 GB; melhor para pendrive",
            Self::Ntfs => "Windows; leitura/escrita no Linux, só leitura no macOS",
            Self::Ext4 => "somente Linux; permissões e journaling",
        }
    }
}

impl fmt::Display for Filesystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Filesystem {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "fat32" | "vfat" | "fat" => Ok(Self::Fat32),
            "exfat" => Ok(Self::Exfat),
            "ntfs" => Ok(Self::Ntfs),
            "ext4" => Ok(Self::Ext4),
            other => Err(format!(
                "sistema de arquivos desconhecido: {other} (use fat32, exfat, ntfs ou ext4)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PartitionTable {
    Gpt,
    Mbr,
}

impl PartitionTable {
    /// MBR é o mais compatível com aparelhos (TV, som, BIOS antiga) e só é
    /// limitado acima de 2 TiB; GPT fica para discos maiores.
    pub fn default_for(size_bytes: u64) -> Self {
        const TWO_TIB: u64 = 2 * 1024 * 1024 * 1024 * 1024;
        if size_bytes > TWO_TIB {
            Self::Gpt
        } else {
            Self::Mbr
        }
    }
}

impl FromStr for PartitionTable {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "gpt" => Ok(Self::Gpt),
            "mbr" | "dos" | "msdos" => Ok(Self::Mbr),
            other => Err(format!("tabela desconhecida: {other} (use gpt ou mbr)")),
        }
    }
}

impl fmt::Display for PartitionTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Gpt => "gpt",
            Self::Mbr => "mbr",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormatRequest {
    pub filesystem: Filesystem,
    pub table: PartitionTable,
    /// Já normalizado por [`normalize_label`].
    pub label: Option<String>,
    /// Limita a partição a esta posição final (pendrive falsificado: usar só
    /// a capacidade real). `None` = disco inteiro.
    pub max_bytes: Option<u64>,
}

/// Início da partição única (alinhamento padrão de 1 MiB).
pub const PARTITION_START: u64 = 1 << 20;
const MIN_PARTITION: u64 = 8 << 20;

/// Escolhe a tabela. Com limite de tamanho só MBR é seguro: a GPT grava uma
/// cópia do cabeçalho no último setor anunciado, que num falso não existe
/// (a gravação some ou "dá a volta" e corrompe o início do disco).
pub fn resolve_table(
    explicit: Option<PartitionTable>,
    disk_bytes: u64,
    max_bytes: Option<u64>,
) -> Result<PartitionTable, String> {
    match (explicit, max_bytes) {
        (Some(PartitionTable::Gpt), Some(_)) => Err(
            "--max-size exige tabela MBR: a GPT grava uma cópia no fim do disco, \
             que num pendrive falsificado não existe"
                .into(),
        ),
        (_, Some(max)) if max > 2 * 1024 * 1024 * 1024 * 1024 => {
            Err("--max-size acima de 2 TiB não cabe em MBR".into())
        }
        (_, Some(_)) => Ok(PartitionTable::Mbr),
        (Some(t), None) => Ok(t),
        (None, None) => Ok(PartitionTable::default_for(disk_bytes)),
    }
}

/// Tamanho da partição, em setores de 512 B, para que ela termine antes de
/// `max_bytes` (alinhado em 1 MiB, começando em [`PARTITION_START`]).
pub fn partition_sectors(disk_bytes: u64, max_bytes: u64) -> Result<u64, String> {
    if max_bytes >= disk_bytes {
        return Err(format!(
            "--max-size ({max_bytes} bytes) precisa ser menor que o disco ({disk_bytes} bytes)"
        ));
    }
    let mib = 1 << 20;
    let usable = max_bytes.saturating_sub(PARTITION_START) / mib * mib;
    if usable < MIN_PARTITION {
        return Err("--max-size pequeno demais (mínimo ~9 MiB)".into());
    }
    Ok(usable / 512)
}

/// Valida e normaliza o rótulo. FAT32 guarda em maiúsculas e não aceita
/// alguns caracteres; os demais só têm limite de tamanho.
pub fn normalize_label(fs: Filesystem, label: &str) -> Result<Option<String>, String> {
    let label = label.trim();
    if label.is_empty() {
        return Ok(None);
    }
    let label = match fs {
        Filesystem::Fat32 => {
            const FORBIDDEN: &str = "\"*+,./:;<=>?[\\]|";
            if let Some(c) = label.chars().find(|c| FORBIDDEN.contains(*c)) {
                return Err(format!("o rótulo FAT32 não pode conter '{c}'"));
            }
            if !label.is_ascii() {
                return Err("o rótulo FAT32 deve usar só caracteres ASCII (sem acentos)".into());
            }
            label.to_ascii_uppercase()
        }
        _ => label.to_string(),
    };
    if label.len() > fs.max_label_len() {
        return Err(format!(
            "rótulo grande demais para {fs}: {} bytes (máximo {})",
            label.len(),
            fs.max_label_len()
        ));
    }
    Ok(Some(label))
}

/// Caminho da N-ésima partição: `/dev/sdb` → `/dev/sdb1`, mas
/// `/dev/nvme0n1`, `/dev/mmcblk0` e `/dev/loop3` → `...p1`.
pub fn partition_path(disk_path: &str, number: u32) -> String {
    if disk_path.ends_with(|c: char| c.is_ascii_digit()) {
        format!("{disk_path}p{number}")
    } else {
        format!("{disk_path}{number}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filesystem_aliases() {
        assert_eq!("VFAT".parse::<Filesystem>(), Ok(Filesystem::Fat32));
        assert_eq!("exFAT".parse::<Filesystem>(), Ok(Filesystem::Exfat));
        assert!("btrfs".parse::<Filesystem>().is_err());
        assert_eq!("dos".parse::<PartitionTable>(), Ok(PartitionTable::Mbr));
    }

    #[test]
    fn fat_labels_are_uppercased_and_checked() {
        assert_eq!(
            normalize_label(Filesystem::Fat32, " meu pen "),
            Ok(Some("MEU PEN".into()))
        );
        assert!(normalize_label(Filesystem::Fat32, "a/b").is_err());
        assert!(normalize_label(Filesystem::Fat32, "câmera").is_err());
        assert!(normalize_label(Filesystem::Fat32, "DOZE_LETRAS_").is_err());
        assert_eq!(normalize_label(Filesystem::Fat32, "  "), Ok(None));
    }

    #[test]
    fn other_labels_keep_case_and_limit_bytes() {
        assert_eq!(
            normalize_label(Filesystem::Exfat, "Backup Fotos"),
            Ok(Some("Backup Fotos".into()))
        );
        // "ç" e "ã" ocupam 2 bytes: 15 caracteres viram 17 bytes.
        assert!(normalize_label(Filesystem::Exfat, "ççaaaaaaaaaaaaa").is_err());
        assert!(normalize_label(Filesystem::Ext4, "dezesseis-letras").is_ok());
        assert!(normalize_label(Filesystem::Ext4, "dezessete-letras!").is_err());
    }

    #[test]
    fn partition_paths() {
        assert_eq!(partition_path("/dev/sdb", 1), "/dev/sdb1");
        assert_eq!(partition_path("/dev/nvme0n1", 1), "/dev/nvme0n1p1");
        assert_eq!(partition_path("/dev/mmcblk0", 2), "/dev/mmcblk0p2");
        assert_eq!(partition_path("/dev/loop7", 1), "/dev/loop7p1");
    }

    #[test]
    fn max_size_forces_mbr_and_refuses_gpt() {
        let gib = 1u64 << 30;
        assert_eq!(
            resolve_table(None, 64 * gib, Some(8 * gib)),
            Ok(PartitionTable::Mbr)
        );
        assert!(resolve_table(Some(PartitionTable::Gpt), 64 * gib, Some(8 * gib)).is_err());
        assert_eq!(
            resolve_table(Some(PartitionTable::Gpt), 64 * gib, None),
            Ok(PartitionTable::Gpt)
        );
        assert!(resolve_table(None, 8 << 40, Some(3 << 40)).is_err());
    }

    #[test]
    fn partition_ends_before_max_size() {
        let sectors = partition_sectors(128 << 20, 40 << 20).unwrap();
        assert_eq!(sectors, 39 * 2048);
        assert!(PARTITION_START + sectors * 512 <= 40 << 20);
        // não alinhado: arredonda para baixo
        let s = partition_sectors(128 << 20, (40 << 20) + 12345).unwrap();
        assert_eq!(s, 39 * 2048);
        assert!(partition_sectors(128 << 20, 128 << 20).is_err());
        assert!(partition_sectors(128 << 20, 4 << 20).is_err());
    }

    #[test]
    fn default_table_by_size() {
        assert_eq!(
            PartitionTable::default_for(64_000_000_000),
            PartitionTable::Mbr
        );
        assert_eq!(
            PartitionTable::default_for(4_000_000_000_000),
            PartitionTable::Gpt
        );
    }
}
