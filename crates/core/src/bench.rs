//! Benchmark: leitura sequencial e aleatória 4K (não destrutivo) e,
//! opcionalmente, escrita sequencial e aleatória 4K (destrutivo).

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::CoreError;
use crate::blockdev::{AlignedBuf, BlockDevice, Phase, Progress, XorShift, align_down, cancelled};
use crate::model::UsbSpeed;

const CHUNK: usize = 1 << 20;
const SMALL: usize = 4096;

#[derive(Debug, Clone, Copy)]
pub struct BenchOptions {
    pub write: bool,
    /// Bytes lidos (e gravados, com `write`) no teste sequencial.
    pub sample_bytes: u64,
    pub random_ops: u64,
    pub random_time_limit: Duration,
    pub seed: u64,
}

impl Default for BenchOptions {
    fn default() -> Self {
        Self {
            write: false,
            sample_bytes: 256 << 20,
            random_ops: 2000,
            random_time_limit: Duration::from_secs(5),
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchResult {
    pub seq_read_mbps: f64,
    pub rand_read_iops: f64,
    pub seq_write_mbps: Option<f64>,
    pub rand_write_iops: Option<f64>,
    pub sample_bytes: u64,
}

fn rate(amount: f64, elapsed: Duration) -> f64 {
    amount / elapsed.as_secs_f64().max(1e-9)
}

/// Lista de (offset, tamanho) em blocos de 1 MiB cobrindo `sample` bytes,
/// divididos entre início, meio e fim do disco (o desempenho varia por região).
fn sequential_plan(size: u64, sample: u64) -> Vec<(u64, usize)> {
    let chunk = CHUNK as u64;
    let sample = sample.min(align_down(size, chunk)).max(chunk.min(size));
    let per_region = align_down(sample / 3, chunk).max(chunk);
    let starts = if size >= per_region * 3 {
        vec![
            0,
            align_down(size / 2, chunk),
            align_down(size - per_region, chunk),
        ]
    } else {
        vec![0]
    };
    let mut plan = Vec::new();
    for start in starts {
        let end = (start + per_region).min(size);
        let mut off = start;
        while off < end {
            let len = (end - off).min(chunk);
            let len = align_down(len, 512);
            if len == 0 {
                break;
            }
            plan.push((off, len as usize));
            off += len;
        }
    }
    plan
}

fn sequential(
    dev: &mut dyn BlockDevice,
    plan: &[(u64, usize)],
    write: bool,
    rng: &mut XorShift,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<f64, CoreError> {
    let mut buf = AlignedBuf::new(CHUNK);
    if write {
        // Dados aleatórios: controladores que comprimem/deduplicam não trapaceiam.
        for c in buf.as_chunks_mut::<8>().0 {
            c.copy_from_slice(&rng.next_u64().to_le_bytes());
        }
    }
    let phase = if write {
        Phase::SeqWrite
    } else {
        Phase::SeqRead
    };
    let total: u64 = plan.iter().map(|(_, l)| *l as u64).sum();
    let mut done = 0;
    let started = Instant::now();
    for &(off, len) in plan {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        if write {
            dev.write_at(off, &buf[..len])?;
        } else {
            dev.read_at(off, &mut buf[..len])?;
        }
        done += len as u64;
        progress.report(phase, done, total);
    }
    if write {
        dev.flush()?;
    }
    Ok(rate(total as f64, started.elapsed()) / 1e6)
}

fn random(
    dev: &mut dyn BlockDevice,
    span: u64,
    write: bool,
    options: &BenchOptions,
    rng: &mut XorShift,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<f64, CoreError> {
    let mut buf = AlignedBuf::new(SMALL);
    let slots = (span / SMALL as u64).max(1);
    let phase = if write {
        Phase::RandWrite
    } else {
        Phase::RandRead
    };
    let started = Instant::now();
    let mut ops = 0;
    while ops < options.random_ops && started.elapsed() < options.random_time_limit {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        let off = rng.below(slots) * SMALL as u64;
        if write {
            buf[..8].copy_from_slice(&rng.next_u64().to_le_bytes());
            dev.write_at(off, &buf)?;
        } else {
            dev.read_at(off, &mut buf)?;
        }
        ops += 1;
        progress.report(phase, ops, options.random_ops);
    }
    if write {
        dev.flush()?;
    }
    Ok(rate(ops as f64, started.elapsed()))
}

pub fn run(
    dev: &mut dyn BlockDevice,
    options: BenchOptions,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<BenchResult, CoreError> {
    let size = dev.size();
    if size < SMALL as u64 {
        return Err(CoreError::Backend("dispositivo pequeno demais".into()));
    }
    let mut rng = XorShift::new(options.seed);
    let plan = sequential_plan(size, options.sample_bytes);
    let sample: u64 = plan.iter().map(|(_, l)| *l as u64).sum();

    let seq_read_mbps = sequential(dev, &plan, false, &mut rng, progress, cancel)?;
    let rand_read_iops = random(dev, size, false, &options, &mut rng, progress, cancel)?;

    let (seq_write_mbps, rand_write_iops) = if options.write {
        let seq = sequential(dev, &plan, true, &mut rng, progress, cancel)?;
        // Escrita aleatória só na região inicial já sacrificada.
        let span = plan.first().map_or(size, |_| sample.min(size));
        let rnd = random(dev, span, true, &options, &mut rng, progress, cancel)?;
        (Some(seq), Some(rnd))
    } else {
        (None, None)
    };

    Ok(BenchResult {
        seq_read_mbps,
        rand_read_iops,
        seq_write_mbps,
        rand_write_iops,
        sample_bytes: sample,
    })
}

/// Teto prático (MB/s) de leitura sequencial para a velocidade USB negociada.
pub fn practical_limit_mbps(speed: UsbSpeed) -> Option<f64> {
    match speed {
        UsbSpeed::Low | UsbSpeed::Full => Some(1.0),
        UsbSpeed::High => Some(42.0),
        UsbSpeed::Super => Some(450.0),
        UsbSpeed::SuperPlus => Some(1000.0),
        UsbSpeed::SuperPlus2x2 => Some(2000.0),
        UsbSpeed::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::Silent;
    use crate::fakedrive::FakeDrive;

    #[test]
    fn plan_covers_three_regions_within_bounds() {
        let size = 16u64 << 30;
        let plan = sequential_plan(size, 256 << 20);
        let total: u64 = plan.iter().map(|(_, l)| *l as u64).sum();
        assert!((255 << 20..=256 << 20).contains(&total), "{total}");
        assert!(plan.iter().all(|(o, l)| o + *l as u64 <= size));
        assert_eq!(plan[0].0, 0);
        assert!(plan.iter().any(|(o, _)| *o >= size / 2));
    }

    #[test]
    fn plan_for_tiny_device() {
        let plan = sequential_plan(3 << 20, 256 << 20);
        let total: u64 = plan.iter().map(|(_, l)| *l as u64).sum();
        assert!(total <= 3 << 20 && total > 0);
    }

    #[test]
    fn read_only_bench_does_not_write() {
        let mut d = FakeDrive::genuine(64 << 20);
        d.seed_user_data(&[0, 4096]);
        let before = d.physical_snapshot();
        let opts = BenchOptions {
            random_ops: 100,
            ..Default::default()
        };
        let r = run(&mut d, opts, &mut Silent, &AtomicBool::new(false)).unwrap();
        assert!(r.seq_read_mbps > 0.0 && r.rand_read_iops > 0.0);
        assert!(r.seq_write_mbps.is_none());
        assert_eq!(d.physical_snapshot(), before);
    }

    #[test]
    fn write_bench_reports_write_numbers() {
        let mut d = FakeDrive::genuine(64 << 20);
        let opts = BenchOptions {
            write: true,
            random_ops: 100,
            ..Default::default()
        };
        let r = run(&mut d, opts, &mut Silent, &AtomicBool::new(false)).unwrap();
        assert!(r.seq_write_mbps.unwrap() > 0.0);
        assert!(r.rand_write_iops.unwrap() > 0.0);
    }
}
