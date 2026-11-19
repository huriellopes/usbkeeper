//! Inspeção do começo de uma imagem ISO: é ISO 9660? tem setor de boot
//! para pendrive (híbrida)? é uma ISO do Windows (que não dá boot gravada
//! byte a byte)?

use serde::Serialize;

/// Bytes do início necessários para a inspeção (até o fim do descritor de volume).
pub const HEAD_LEN: usize = 34_816;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct IsoInfo {
    pub is_iso: bool,
    /// Assinatura de setor de boot MBR: dá boot gravada direto num pendrive.
    pub hybrid: bool,
    pub volume_id: Option<String>,
    pub windows: bool,
}

const WINDOWS_PREFIXES: [&str; 5] = ["CCCOMA_", "CPBA_", "CENA_", "CCSA_", "GRMC"];

pub fn inspect(head: &[u8]) -> IsoInfo {
    let hybrid = head.len() >= 512 && head[510] == 0x55 && head[511] == 0xAA;
    let is_iso = head.len() >= 32_840 && &head[32_769..32_774] == b"CD001";
    let volume_id = is_iso
        .then(|| {
            String::from_utf8_lossy(&head[32_808..32_840])
                .trim()
                .to_string()
        })
        .filter(|v| !v.is_empty());
    let windows = volume_id.as_deref().is_some_and(|v| {
        let up = v.to_ascii_uppercase();
        up == "ESD-ISO" || WINDOWS_PREFIXES.iter().any(|p| up.starts_with(p))
    });
    IsoInfo {
        is_iso,
        hybrid,
        volume_id,
        windows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head() -> Vec<u8> {
        vec![0u8; HEAD_LEN]
    }

    fn with_volume(mut h: Vec<u8>, volume: &str) -> Vec<u8> {
        h[32769..32774].copy_from_slice(b"CD001");
        let mut v = [b' '; 32];
        v[..volume.len()].copy_from_slice(volume.as_bytes());
        h[32808..32840].copy_from_slice(&v);
        h
    }

    #[test]
    fn garbage_is_nothing() {
        let i = inspect(&head());
        assert!(!i.is_iso && !i.hybrid && !i.windows && i.volume_id.is_none());
        assert!(!inspect(&[1, 2, 3]).is_iso);
    }

    #[test]
    fn hybrid_linux_iso() {
        let mut h = with_volume(head(), "Ubuntu 24.04 LTS amd64");
        h[510] = 0x55;
        h[511] = 0xAA;
        let i = inspect(&h);
        assert!(i.is_iso && i.hybrid && !i.windows);
        assert_eq!(i.volume_id.as_deref(), Some("Ubuntu 24.04 LTS amd64"));
    }

    #[test]
    fn windows_iso_is_detected() {
        let i = inspect(&with_volume(head(), "CCCOMA_X64FRE_PT-BR_DV9"));
        assert!(i.is_iso && i.windows && !i.hybrid);
        assert!(inspect(&with_volume(head(), "ESD-ISO")).windows);
    }
}
