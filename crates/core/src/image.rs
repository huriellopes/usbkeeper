//! Cópia disco ↔ fluxo (arquivo de imagem, ISO) com SHA-256 calculado
//! durante a cópia: base do backup/restauração e da gravação de ISO.

use std::io::{Read, Write};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::CoreError;
use crate::blockdev::{AlignedBuf, BlockDevice, Phase, Progress, SECTOR, cancelled};

const CHUNK: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CopyReport {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub bytes: u64,
    /// SHA-256 dos bytes copiados, em hexadecimal.
    pub sha256: String,
    pub seconds: f64,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn round_up(n: usize) -> usize {
    n.div_ceil(SECTOR) * SECTOR
}

/// Lê `len` bytes do disco e escreve em `out`, calculando o hash.
pub fn device_to_writer(
    dev: &mut dyn BlockDevice,
    len: u64,
    out: &mut dyn Write,
    phase: Phase,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<CopyReport, CoreError> {
    let started = Instant::now();
    let mut buf = AlignedBuf::new(CHUNK);
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    while offset < len {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        let want = (len - offset).min(CHUNK as u64) as usize;
        // O disco só lê em múltiplos do setor; o excesso é descartado.
        dev.read_at(offset, &mut buf[..round_up(want)])?;
        hasher.update(&buf[..want]);
        out.write_all(&buf[..want])?;
        offset += want as u64;
        progress.report(phase, offset, len);
    }
    out.flush()?;
    Ok(CopyReport {
        bytes: len,
        sha256: hex(&hasher.finalize()),
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// SHA-256 dos primeiros `len` bytes do disco (verificação).
pub fn hash_device(
    dev: &mut dyn BlockDevice,
    len: u64,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<String, CoreError> {
    Ok(device_to_writer(
        dev,
        len,
        &mut std::io::sink(),
        Phase::Verify,
        progress,
        cancel,
    )?
    .sha256)
}

/// Lê `src` até o fim e grava no disco a partir do início; recusa antes de
/// passar do fim do disco. `total_hint` só serve para o progresso.
pub fn reader_to_device(
    src: &mut dyn Read,
    dev: &mut dyn BlockDevice,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
    total_hint: u64,
) -> Result<CopyReport, CoreError> {
    let started = Instant::now();
    let capacity = dev.size();
    let mut buf = AlignedBuf::new(CHUNK);
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    loop {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        // Enche o bloco (um Read pode devolver menos que o pedido).
        let mut filled = 0;
        while filled < CHUNK {
            let n = src.read(&mut buf[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        if filled == 0 {
            break;
        }
        let padded = round_up(filled);
        if offset + padded as u64 > capacity {
            return Err(CoreError::Backend(format!(
                "a origem é maior que o disco ({} bytes); nada além do fim foi gravado",
                capacity
            )));
        }
        hasher.update(&buf[..filled]);
        buf[filled..padded].fill(0);
        dev.write_at(offset, &buf[..padded])?;
        offset += filled as u64;
        progress.report(Phase::Image, offset, total_hint.max(offset));
        if filled < CHUNK {
            break;
        }
    }
    dev.flush()?;
    Ok(CopyReport {
        bytes: offset,
        sha256: hex(&hasher.finalize()),
        seconds: started.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::Silent;
    use crate::fakedrive::FakeDrive;
    use std::io::Cursor;

    fn never() -> AtomicBool {
        AtomicBool::new(false)
    }

    #[test]
    fn device_roundtrip_preserves_bytes_and_hash() {
        let mut src = FakeDrive::genuine(4 << 20);
        src.seed_user_data(&[0, 4096, 3 << 20]);
        let mut out = Vec::new();
        let a = device_to_writer(
            &mut src,
            4 << 20,
            &mut out,
            Phase::Image,
            &mut Silent,
            &never(),
        )
        .unwrap();
        assert_eq!(a.bytes, 4 << 20);
        assert_eq!(out.len(), 4 << 20);

        let mut dst = FakeDrive::genuine(8 << 20);
        let b = reader_to_device(
            &mut Cursor::new(out),
            &mut dst,
            &mut Silent,
            &never(),
            4 << 20,
        )
        .unwrap();
        assert_eq!(a.sha256, b.sha256);
        assert_eq!(a.sha256.len(), 64);
        assert_eq!(
            hash_device(&mut dst, 4 << 20, &mut Silent, &never()).unwrap(),
            a.sha256
        );
        // Conteúdo lido de volta igual ao da origem (os mapas internos do
        // FakeDrive diferem: o destino guarda também os setores zerados).
        let (mut a_bytes, mut b_bytes) = (Vec::new(), Vec::new());
        device_to_writer(
            &mut src,
            4 << 20,
            &mut a_bytes,
            Phase::Verify,
            &mut Silent,
            &never(),
        )
        .unwrap();
        device_to_writer(
            &mut dst,
            4 << 20,
            &mut b_bytes,
            Phase::Verify,
            &mut Silent,
            &never(),
        )
        .unwrap();
        assert_eq!(a_bytes, b_bytes);
    }

    #[test]
    fn odd_length_is_padded_on_disk_but_hashed_exactly() {
        let len = (3 << 20) + 1000;
        let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8 + 1).collect();
        let mut dst = FakeDrive::genuine(8 << 20);
        let r = reader_to_device(
            &mut Cursor::new(data.clone()),
            &mut dst,
            &mut Silent,
            &never(),
            len as u64,
        )
        .unwrap();
        assert_eq!(r.bytes, len as u64);
        assert_eq!(
            hash_device(&mut dst, len as u64, &mut Silent, &never()).unwrap(),
            r.sha256
        );
        let mut back = Vec::new();
        device_to_writer(
            &mut dst,
            len as u64,
            &mut back,
            Phase::Verify,
            &mut Silent,
            &never(),
        )
        .unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn source_larger_than_device_is_refused() {
        let mut dst = FakeDrive::genuine(1 << 20);
        let err = reader_to_device(
            &mut Cursor::new(vec![1u8; (2 << 20) + 5]),
            &mut dst,
            &mut Silent,
            &never(),
            0,
        )
        .unwrap_err();
        assert!(err.to_string().contains("maior que o disco"), "{err}");
    }

    #[test]
    fn known_hash() {
        // sha256 de 1 MiB de zeros
        let mut d = FakeDrive::genuine(1 << 20);
        assert_eq!(
            hash_device(&mut d, 1 << 20, &mut Silent, &never()).unwrap(),
            "30e14955ebf1352266dc2ff8067e68104607e750abb9d3b36582b8af909fcb58"
        );
    }

    #[test]
    fn cancel_stops() {
        let mut d = FakeDrive::genuine(2 << 20);
        let err = device_to_writer(
            &mut d,
            2 << 20,
            &mut Vec::new(),
            Phase::Image,
            &mut Silent,
            &AtomicBool::new(true),
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Cancelled));
    }
}
