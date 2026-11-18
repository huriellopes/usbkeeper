//! Varredura de superfície somente leitura: lê o disco inteiro e localiza
//! blocos ilegíveis e regiões lentas (sinal de setores degradados que o
//! controlador está relendo). Não grava nada.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::CoreError;
use crate::blockdev::{AlignedBuf, BlockDevice, Phase, Progress, align_down, cancelled};

const CHUNK: usize = 1 << 20;
/// Unidade de localização dos defeitos: 4 KiB funciona também em discos 4Kn.
pub const BLOCK: usize = 4096;
const MAX_LISTED: usize = 256;
/// Uma leitura de 1 MiB acima disso indica região degradada.
pub const SLOW_CHUNK: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub scanned_bytes: u64,
    /// Blocos de 4 KiB que falharam na leitura (no máximo os primeiros 256).
    pub unreadable_offsets: Vec<u64>,
    pub unreadable_blocks: u64,
    pub slow_chunks: u64,
    pub slowest_chunk_ms: u64,
    pub read_mbps: f64,
    pub seconds: f64,
}

impl ScanReport {
    pub fn is_clean(&self) -> bool {
        self.unreadable_blocks == 0
    }
}

pub fn read_scan(
    dev: &mut dyn BlockDevice,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<ScanReport, CoreError> {
    let size = align_down(dev.size(), BLOCK as u64);
    let started = Instant::now();
    let mut buf = AlignedBuf::new(CHUNK);
    let mut small = AlignedBuf::new(BLOCK);
    let mut report = ScanReport {
        scanned_bytes: 0,
        unreadable_offsets: Vec::new(),
        unreadable_blocks: 0,
        slow_chunks: 0,
        slowest_chunk_ms: 0,
        read_mbps: 0.0,
        seconds: 0.0,
    };

    let mut offset = 0;
    while offset < size {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        let len = (size - offset).min(CHUNK as u64) as usize;
        let t = Instant::now();
        let ok = dev.read_at(offset, &mut buf[..len]).is_ok();
        let elapsed = t.elapsed();
        if ok {
            if elapsed >= SLOW_CHUNK {
                report.slow_chunks += 1;
            }
            report.slowest_chunk_ms = report.slowest_chunk_ms.max(elapsed.as_millis() as u64);
        } else {
            // Relê bloco a bloco para localizar exatamente o que falha.
            for block in (offset..offset + len as u64).step_by(BLOCK) {
                if dev.read_at(block, &mut small).is_err() {
                    report.unreadable_blocks += 1;
                    if report.unreadable_offsets.len() < MAX_LISTED {
                        report.unreadable_offsets.push(block);
                    }
                }
            }
        }
        offset += len as u64;
        report.scanned_bytes = offset;
        progress.report(Phase::Scan, offset, size);
    }
    report.seconds = started.elapsed().as_secs_f64();
    report.read_mbps = size as f64 / report.seconds.max(1e-9) / 1e6;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::Silent;
    use crate::fakedrive::FakeDrive;

    #[test]
    fn clean_disk() {
        let mut d = FakeDrive::genuine(8 << 20);
        let r = read_scan(&mut d, &mut Silent, &AtomicBool::new(false)).unwrap();
        assert!(r.is_clean());
        assert_eq!(r.scanned_bytes, 8 << 20);
    }

    #[test]
    fn locates_unreadable_blocks() {
        let mut d = FakeDrive::genuine(8 << 20)
            .with_unreadable_sector((3 << 20) + 4096 + 512)
            .with_unreadable_sector((3 << 20) + 4096 + 1024) // mesmo bloco de 4K
            .with_unreadable_sector((8 << 20) - 512);
        let r = read_scan(&mut d, &mut Silent, &AtomicBool::new(false)).unwrap();
        assert_eq!(r.unreadable_blocks, 2);
        assert_eq!(r.unreadable_offsets, [(3 << 20) + 4096, (8 << 20) - 4096]);
    }

    #[test]
    fn scan_never_writes() {
        let mut d = FakeDrive::genuine(4 << 20);
        d.seed_user_data(&[0, 1 << 20]);
        let before = d.physical_snapshot();
        read_scan(&mut d, &mut Silent, &AtomicBool::new(false)).unwrap();
        assert_eq!(d.physical_snapshot(), before);
    }
}
