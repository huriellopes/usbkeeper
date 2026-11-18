//! Carimbo de setor: cada setor de 512 B gravado no teste carrega o próprio
//! endereço e a semente da execução, seguidos de dados pseudoaleatórios
//! derivados dos dois. Ao ler de volta dá para saber se o setor está íntegro,
//! corrompido ou se contém o carimbo de *outro* endereço (aliasing).

use crate::blockdev::{SECTOR, XorShift};

const MAGIC: [u8; 8] = *b"USBKEEP1";
const HEADER: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorCheck {
    /// Carimbo deste endereço, conteúdo íntegro.
    Ok,
    /// Carimbo íntegro de outro endereço: os dois apontam para a mesma
    /// célula física (pendrive que "dá a volta").
    Alias(u64),
    /// Zeros, lixo ou carimbo corrompido.
    Bad,
}

fn fill_sector(sector: &mut [u8], offset: u64, seed: u64) {
    sector[..8].copy_from_slice(&MAGIC);
    sector[8..16].copy_from_slice(&offset.to_le_bytes());
    sector[16..24].copy_from_slice(&seed.to_le_bytes());
    let mut rng = XorShift::new(seed ^ offset.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    for chunk in sector[HEADER..].as_chunks_mut::<8>().0 {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
}

/// Carimba todos os setores de `buf`, que começa no endereço `offset`.
pub fn stamp(buf: &mut [u8], offset: u64, seed: u64) {
    for (i, sector) in buf.as_chunks_mut::<SECTOR>().0.iter_mut().enumerate() {
        fill_sector(sector, offset + (i * SECTOR) as u64, seed);
    }
}

/// Confere um setor lido do endereço `offset`.
pub fn check_sector(sector: &[u8], offset: u64, seed: u64) -> SectorCheck {
    if sector.len() != SECTOR || sector[..8] != MAGIC || sector[16..24] != seed.to_le_bytes() {
        return SectorCheck::Bad;
    }
    let stamped = u64::from_le_bytes(sector[8..16].try_into().expect("8 bytes"));
    let mut expected = [0u8; SECTOR];
    fill_sector(&mut expected, stamped, seed);
    if sector != expected {
        SectorCheck::Bad
    } else if stamped == offset {
        SectorCheck::Ok
    } else {
        SectorCheck::Alias(stamped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ok_alias_and_bad() {
        let mut buf = vec![0u8; SECTOR * 2];
        stamp(&mut buf, 4096, 7);
        assert_eq!(check_sector(&buf[..SECTOR], 4096, 7), SectorCheck::Ok);
        assert_eq!(check_sector(&buf[SECTOR..], 4608, 7), SectorCheck::Ok);
        // Mesmo conteúdo lido em outro endereço → aliasing.
        assert_eq!(check_sector(&buf[..SECTOR], 0, 7), SectorCheck::Alias(4096));
        // Semente de outra rodada não vale.
        assert_eq!(check_sector(&buf[..SECTOR], 4096, 8), SectorCheck::Bad);
        // Um bit trocado → corrompido.
        buf[300] ^= 1;
        assert_eq!(check_sector(&buf[..SECTOR], 4096, 7), SectorCheck::Bad);
        assert_eq!(check_sector(&[0u8; SECTOR], 0, 7), SectorCheck::Bad);
    }
}
