//! Detecção de pendrive/cartão falsificado (capacidade adulterada).
//!
//! **Completo** ([`full_check`]): grava todos os setores com o carimbo de
//! [`crate::pattern`] e lê tudo de volta. Definitivo, mas destrói os dados e
//! demora o tempo de gravar o dispositivo inteiro.
//!
//! **Rápido** ([`quick_probe`]): grava ~128–256 blocos de 4 KiB numa grade de
//! potência de 2, lê de volta e faz busca binária até o limite real
//! (precisão de 1 MiB). Salva e restaura os blocos originais. Pega os falsos
//! comuns (descarte acima da capacidade real e "volta" com módulo potência de
//! 2, como os detectados pelo f3probe); para certeza total use o completo.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use serde::Serialize;

use crate::CoreError;
use crate::blockdev::{AlignedBuf, BlockDevice, Phase, Progress, SECTOR, align_down, cancelled};
use crate::pattern::{SectorCheck, check_sector, stamp};

const CHUNK: usize = 1 << 20;
pub const PROBE: usize = 4096;
const MIN_PROBES: u64 = 128;
const PRECISION: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum CheckMode {
    Quick,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Toda a capacidade anunciada guarda dados corretamente.
    Genuine,
    /// Capacidade real menor que a anunciada.
    Fake,
    /// Capacidade correta, mas com setores defeituosos.
    Damaged,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct FakeReport {
    pub mode: CheckMode,
    pub verdict: Verdict,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub advertised_bytes: u64,
    /// Capacidade que realmente guarda dados (estimativa no modo rápido).
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub real_bytes: u64,
    /// Ciclo após o qual os endereços "dão a volta", se detectado.
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub wrap_module_bytes: Option<u64>,
    /// Primeiro endereço que devolveu lixo/zeros.
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub first_bad_offset: Option<u64>,
    /// No completo: bytes íntegros / com lixo / sobrescritos por outro
    /// endereço. No rápido: o mesmo, mas só dos blocos amostrados.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub ok_bytes: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub bad_bytes: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub aliased_bytes: u64,
    pub probes: Option<u32>,
    pub restored_blocks: Option<u32>,
    pub write_mbps: Option<f64>,
    pub read_mbps: Option<f64>,
    pub seconds: f64,
    /// Para falsos: valor para `format --max-size` (capacidade real com 2%
    /// de margem, alinhada em 1 MiB).
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub suggested_max_bytes: Option<u64>,
}

/// Capacidade segura para reaproveitar um falso: 98% da real, em MiB inteiros.
pub fn suggested_max_bytes(verdict: Verdict, real_bytes: u64) -> Option<u64> {
    let mib = 1 << 20;
    let safe = real_bytes / 100 * 98 / mib * mib;
    (verdict == Verdict::Fake && safe >= 16 * mib).then_some(safe)
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn gcd_opt(acc: Option<u64>, value: u64) -> Option<u64> {
    if value == 0 {
        return acc;
    }
    Some(acc.map_or(value, |g| gcd(g, value)))
}

fn mbps(bytes: u64, secs: f64) -> f64 {
    bytes as f64 / secs.max(1e-9) / 1e6
}

// ---------------------------------------------------------------- completo

pub fn full_check(
    dev: &mut dyn BlockDevice,
    seed: u64,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<FakeReport, CoreError> {
    let size = align_down(dev.size(), SECTOR as u64);
    let started = Instant::now();
    let mut buf = AlignedBuf::new(CHUNK);

    let mut offset = 0;
    while offset < size {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        let len = (size - offset).min(CHUNK as u64) as usize;
        stamp(&mut buf[..len], offset, seed);
        dev.write_at(offset, &buf[..len])?;
        offset += len as u64;
        progress.report(Phase::Write, offset, size);
    }
    dev.flush()?;
    let write_secs = started.elapsed().as_secs_f64();

    let read_started = Instant::now();
    let (mut ok, mut bad, mut aliased) = (0u64, 0u64, 0u64);
    let mut first_bad = None;
    let mut module = None;
    let mut offset = 0;
    while offset < size {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        let len = (size - offset).min(CHUNK as u64) as usize;
        dev.read_at(offset, &mut buf[..len])?;
        for (i, sector) in buf[..len].as_chunks::<SECTOR>().0.iter().enumerate() {
            let at = offset + (i * SECTOR) as u64;
            match check_sector(sector, at, seed) {
                SectorCheck::Ok => ok += 1,
                SectorCheck::Alias(other) => {
                    aliased += 1;
                    module = gcd_opt(module, other.abs_diff(at));
                }
                SectorCheck::Bad => {
                    bad += 1;
                    first_bad.get_or_insert(at);
                }
            }
        }
        offset += len as u64;
        progress.report(Phase::Verify, offset, size);
    }
    let read_secs = read_started.elapsed().as_secs_f64();

    let sector = SECTOR as u64;
    // Lixo concentrado no fim (≥95% do trecho a partir do 1º setor ruim, e
    // trecho de pelo menos 1% do disco) é capacidade inexistente, não desgaste.
    let tail_fake = first_bad.is_some_and(|fb| {
        let tail = size - fb;
        tail >= (size / 100).max(PRECISION) && bad * sector * 100 >= tail * 95
    });
    let (verdict, real_bytes) = if aliased > 0 {
        let real = ok * sector;
        (Verdict::Fake, first_bad.map_or(real, |fb| real.min(fb)))
    } else if tail_fake {
        (Verdict::Fake, first_bad.unwrap_or(size))
    } else if bad > 0 {
        (Verdict::Damaged, size - bad * sector)
    } else {
        (Verdict::Genuine, size)
    };

    Ok(FakeReport {
        mode: CheckMode::Full,
        verdict,
        advertised_bytes: dev.size(),
        real_bytes,
        wrap_module_bytes: module,
        first_bad_offset: first_bad,
        ok_bytes: ok * sector,
        bad_bytes: bad * sector,
        aliased_bytes: aliased * sector,
        probes: None,
        restored_blocks: None,
        write_mbps: Some(mbps(size, write_secs)),
        read_mbps: Some(mbps(size, read_secs)),
        seconds: started.elapsed().as_secs_f64(),
        suggested_max_bytes: suggested_max_bytes(verdict, real_bytes),
    })
}

// ------------------------------------------------------------------ rápido

#[derive(Debug, Clone, Copy)]
pub struct QuickOptions {
    pub seed: u64,
    /// Salva os blocos originais e os restaura no fim (inclusive se cancelado).
    pub backup: bool,
}

/// Maior potência de 2 (≥ 4 KiB) que ainda gera pelo menos [`MIN_PROBES`]
/// pontos. Em grade de potência de 2, um endereço que "dá a volta" num
/// módulo potência de 2 cai exatamente sobre outro ponto já gravado.
pub fn grid_step(size: u64) -> u64 {
    let mut step = PROBE as u64;
    while size / (step * 2) >= MIN_PROBES {
        step *= 2;
    }
    step
}

fn grid_positions(size: u64) -> Vec<u64> {
    let step = grid_step(size);
    let probe = PROBE as u64;
    let mut positions: BTreeSet<u64> = (0..)
        .map(|i| i * step)
        .take_while(|p| p + probe <= size)
        .collect();
    positions.insert(align_down(size - probe, probe));
    positions.into_iter().collect()
}

struct Verification {
    bad: BTreeSet<u64>,
    module: Option<u64>,
}

struct Session<'a> {
    dev: &'a mut dyn BlockDevice,
    seed: u64,
    backup: bool,
    backups: Vec<(u64, AlignedBuf)>,
    written: BTreeSet<u64>,
    buf: AlignedBuf,
}

impl Session<'_> {
    fn probe(&mut self, pos: u64) -> std::io::Result<()> {
        if self.written.contains(&pos) {
            return Ok(());
        }
        if self.backup {
            let mut original = AlignedBuf::new(PROBE);
            self.dev.read_at(pos, &mut original)?;
            self.backups.push((pos, original));
        }
        stamp(&mut self.buf, pos, self.seed);
        self.dev.write_at(pos, &self.buf)?;
        self.written.insert(pos);
        Ok(())
    }

    /// Lê todos os blocos gravados. Endereços que mostram o mesmo carimbo
    /// compartilham a mesma célula física: só o menor deles é real.
    fn verify(&mut self) -> std::io::Result<Verification> {
        self.dev.flush()?;
        let probe = PROBE as u64;
        let mut bad = BTreeSet::new();
        // carimbo lido → endereços (de setor) onde ele apareceu
        let mut cells: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        let positions: Vec<u64> = self.written.iter().copied().collect();
        for pos in positions {
            self.dev.read_at(pos, &mut self.buf)?;
            for (i, sector) in self.buf.as_chunks::<SECTOR>().0.iter().enumerate() {
                let at = pos + (i * SECTOR) as u64;
                match check_sector(sector, at, self.seed) {
                    SectorCheck::Ok => cells.entry(at).or_default().push(at),
                    SectorCheck::Alias(stamped) => cells.entry(stamped).or_default().push(at),
                    SectorCheck::Bad => {
                        bad.insert(pos);
                    }
                }
            }
        }
        let mut module = None;
        for (stamped, mut readers) in cells {
            // O dono do carimbo também compartilha a célula, mesmo que sua
            // própria leitura tenha sido classificada em outro grupo.
            readers.push(stamped);
            readers.sort_unstable();
            readers.dedup();
            let lowest = readers[0];
            for &other in &readers[1..] {
                bad.insert(align_down(other, probe));
                module = gcd_opt(module, other - lowest);
            }
        }
        Ok(Verification { bad, module })
    }

    /// Restaura do maior para o menor endereço: em cada grupo que compartilha
    /// a mesma célula, o menor endereço (o único com o backup original) é
    /// gravado por último.
    fn restore(&mut self) -> std::io::Result<u32> {
        self.backups.sort_by_key(|(pos, _)| std::cmp::Reverse(*pos));
        for (pos, original) in &self.backups {
            self.dev.write_at(*pos, original)?;
        }
        self.dev.flush()?;
        Ok(self.backups.len() as u32)
    }
}

struct Probed {
    bad: BTreeSet<u64>,
    module: Option<u64>,
    boundary: Option<u64>,
}

fn probe_and_refine(
    s: &mut Session<'_>,
    positions: &[u64],
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<Probed, CoreError> {
    let total = positions.len() as u64;
    for (i, &pos) in positions.iter().enumerate() {
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        s.probe(pos)?;
        progress.report(Phase::Probe, i as u64 + 1, total);
    }
    let v = s.verify()?;
    let (mut bad, mut module) = (v.bad, v.module);

    let Some(&first_bad) = bad.first() else {
        return Ok(Probed {
            bad,
            module,
            boundary: None,
        });
    };

    // Abaixo do primeiro bloco ruim tudo leu certo; busca binária no meio.
    let mut hi = first_bad;
    let mut lo = s.written.range(..hi).next_back().copied();
    let rounds = lo.map_or(0, |l| ((hi - l) / PRECISION).max(1).ilog2() as u64 + 1);
    let mut round = 0;
    while let Some(l) = lo {
        if hi - l <= PRECISION {
            break;
        }
        let mid = align_down(l + (hi - l) / 2, PROBE as u64);
        if mid <= l {
            break;
        }
        if cancelled(cancel) {
            return Err(CoreError::Cancelled);
        }
        s.probe(mid)?;
        let v = s.verify()?;
        bad = v.bad;
        if let Some(m) = v.module {
            module = gcd_opt(module, m);
        }
        hi = *bad.first().expect("o bloco ruim anterior continua ruim");
        lo = s.written.range(..hi).next_back().copied();
        round += 1;
        progress.report(Phase::Refine, round.min(rounds), rounds);
    }
    Ok(Probed {
        bad,
        module,
        boundary: Some(if lo.is_some() { hi } else { 0 }),
    })
}

pub fn quick_probe(
    dev: &mut dyn BlockDevice,
    options: QuickOptions,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<FakeReport, CoreError> {
    let size = dev.size();
    if size < (PROBE as u64) * 4 {
        return Err(CoreError::Backend(
            "dispositivo pequeno demais para o teste".into(),
        ));
    }
    let started = Instant::now();
    let positions = grid_positions(size);
    let mut s = Session {
        dev,
        seed: options.seed,
        backup: options.backup,
        backups: Vec::new(),
        written: BTreeSet::new(),
        buf: AlignedBuf::new(PROBE),
    };

    let outcome = probe_and_refine(&mut s, &positions, progress, cancel);

    // A restauração acontece sempre, até em erro ou cancelamento.
    let restored = if options.backup {
        progress.report(Phase::Restore, 0, 1);
        let n = s.restore().map_err(|e| {
            CoreError::Backend(format!(
                "FALHA ao restaurar os blocos originais ({e}); os dados podem estar corrompidos"
            ))
        })?;
        progress.report(Phase::Restore, 1, 1);
        Some(n)
    } else {
        None
    };
    let probed = outcome?;

    let probe = PROBE as u64;
    let written = s.written.len() as u64;
    let bad_count = probed.bad.len() as u64;
    let (verdict, real_bytes) = match probed.boundary {
        None => (Verdict::Genuine, size),
        Some(boundary) => {
            // Falso de verdade: todos os pontos da grade a partir do limite
            // falham. Pontos ruins isolados no meio são defeito, não fraude.
            // Só pontos da grade contam: o último bloco (fora da grade) pode
            // "dar a volta" para uma posição nunca gravada e parecer bom.
            let step = grid_step(size);
            let tail_all_bad = positions
                .iter()
                .filter(|&&p| p >= boundary && p % step == 0)
                .all(|p| probed.bad.contains(p));
            if tail_all_bad {
                (Verdict::Fake, boundary)
            } else {
                (Verdict::Damaged, size)
            }
        }
    };

    Ok(FakeReport {
        mode: CheckMode::Quick,
        verdict,
        advertised_bytes: size,
        real_bytes,
        wrap_module_bytes: probed.module.filter(|_| verdict == Verdict::Fake),
        first_bad_offset: probed.bad.first().copied(),
        ok_bytes: (written - bad_count) * probe,
        bad_bytes: bad_count * probe,
        aliased_bytes: 0,
        probes: Some(written as u32),
        restored_blocks: restored,
        write_mbps: None,
        read_mbps: None,
        seconds: started.elapsed().as_secs_f64(),
        suggested_max_bytes: suggested_max_bytes(verdict, real_bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::Silent;
    use crate::fakedrive::FakeDrive;

    const MIB: u64 = 1 << 20;
    const GIB: u64 = 1 << 30;

    fn quick(dev: &mut FakeDrive) -> FakeReport {
        let opts = QuickOptions {
            seed: 1234,
            backup: true,
        };
        quick_probe(dev, opts, &mut Silent, &AtomicBool::new(false)).unwrap()
    }

    fn full(dev: &mut FakeDrive) -> FakeReport {
        full_check(dev, 99, &mut Silent, &AtomicBool::new(false)).unwrap()
    }

    #[test]
    fn grid_has_between_128_and_256_points() {
        for size in [64 * MIB, 8 * GIB, 64 * GIB, 2048 * GIB] {
            let n = grid_positions(size).len() as u64;
            assert!((128..=257).contains(&n), "{size}: {n} pontos");
        }
    }

    #[test]
    fn quick_genuine_64g() {
        let r = quick(&mut FakeDrive::genuine(64 * GIB));
        assert_eq!(r.verdict, Verdict::Genuine);
        assert_eq!(r.suggested_max_bytes, None);
        assert_eq!(r.real_bytes, 64 * GIB);
    }

    #[test]
    fn quick_detects_8g_wrapping_as_64g() {
        let r = quick(&mut FakeDrive::wrapping(64 * GIB, 8 * GIB));
        assert_eq!(r.verdict, Verdict::Fake);
        assert!(
            r.real_bytes.abs_diff(8 * GIB) <= MIB,
            "real={}",
            r.real_bytes
        );
        assert_eq!(r.wrap_module_bytes, Some(8 * GIB));
        let suggested = r.suggested_max_bytes.unwrap();
        assert!(suggested < r.real_bytes && suggested > r.real_bytes / 100 * 97);
        assert_eq!(suggested % MIB, 0);
    }

    #[test]
    fn quick_detects_tiny_module_under_grid_step() {
        // Módulo (512 MiB) bem menor que o passo da grade de 2 TB.
        let r = quick(&mut FakeDrive::wrapping(2048 * GIB, 512 * MIB));
        assert_eq!(r.verdict, Verdict::Fake);
        assert!(
            r.real_bytes.abs_diff(512 * MIB) <= MIB,
            "real={}",
            r.real_bytes
        );
    }

    #[test]
    fn quick_detects_discarding_fake() {
        // Capacidade real não é potência de 2: descarte é pego direto.
        let real = 7_800 * MIB;
        let r = quick(&mut FakeDrive::discarding(32 * GIB, real));
        assert_eq!(r.verdict, Verdict::Fake);
        assert!(r.real_bytes.abs_diff(real) <= MIB, "real={}", r.real_bytes);
        assert_eq!(r.wrap_module_bytes, None);
    }

    #[test]
    fn quick_isolated_bad_block_is_damage_not_fake() {
        let step = grid_step(16 * GIB);
        let mut d = FakeDrive::genuine(16 * GIB).with_rotten_sector(10 * step);
        let r = quick(&mut d);
        assert_eq!(r.verdict, Verdict::Damaged);
        assert_eq!(r.first_bad_offset, Some(10 * step));
    }

    #[test]
    fn quick_restores_user_data_even_when_wrapping() {
        let mut d = FakeDrive::wrapping(64 * GIB, 8 * GIB);
        let step = grid_step(64 * GIB);
        d.seed_user_data(&[0, 512, step, 3 * step, 8 * GIB - 4096]);
        let before = d.physical_snapshot();
        let r = quick(&mut d);
        assert!(r.restored_blocks.unwrap() > 0);
        // Toda célula que tinha dado volta ao valor original; as que eram
        // vazias voltam a zero (o backup leu zeros).
        let after: Vec<_> = d
            .physical_snapshot()
            .into_iter()
            .filter(|(_, data)| data.iter().any(|&b| b != 0))
            .collect();
        assert_eq!(after, before);
    }

    /// Simula Ctrl+C depois de N blocos gravados.
    struct CancelAfter<'a> {
        writes: u64,
        flag: &'a AtomicBool,
    }

    impl Progress for CancelAfter<'_> {
        fn report(&mut self, phase: Phase, done: u64, _: u64) {
            if phase == Phase::Probe && done >= self.writes {
                self.flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    #[test]
    fn quick_cancel_mid_write_still_restores() {
        let mut d = FakeDrive::genuine(GIB);
        let step = grid_step(GIB);
        d.seed_user_data(&[0, step, 5 * step, 50 * step]);
        let before = d.physical_snapshot();
        let flag = AtomicBool::new(false);
        let mut progress = CancelAfter {
            writes: 10,
            flag: &flag,
        };
        let opts = QuickOptions {
            seed: 1,
            backup: true,
        };
        let err = quick_probe(&mut d, opts, &mut progress, &flag).unwrap_err();
        assert!(matches!(err, CoreError::Cancelled));
        let after: Vec<_> = d
            .physical_snapshot()
            .into_iter()
            .filter(|(_, data)| data.iter().any(|&b| b != 0))
            .collect();
        assert_eq!(
            after, before,
            "blocos gravados antes do Ctrl+C foram restaurados"
        );
    }

    #[test]
    fn full_genuine() {
        let r = full(&mut FakeDrive::genuine(16 * MIB));
        assert_eq!(r.verdict, Verdict::Genuine);
        assert_eq!(r.ok_bytes, 16 * MIB);
    }

    #[test]
    fn full_detects_wrap_and_module() {
        let r = full(&mut FakeDrive::wrapping(64 * MIB, 16 * MIB));
        assert_eq!(r.verdict, Verdict::Fake);
        assert_eq!(r.real_bytes, 16 * MIB);
        assert_eq!(r.wrap_module_bytes, Some(16 * MIB));
        assert_eq!(r.aliased_bytes, 48 * MIB);
    }

    #[test]
    fn full_detects_non_power_of_two_wrap() {
        let real = 10 * MIB + 512 * 3;
        let r = full(&mut FakeDrive::wrapping(32 * MIB, real));
        assert_eq!(r.verdict, Verdict::Fake);
        assert_eq!(r.real_bytes, real);
    }

    #[test]
    fn full_detects_discard() {
        let r = full(&mut FakeDrive::discarding(40 * MIB, 13 * MIB));
        assert_eq!(r.verdict, Verdict::Fake);
        assert_eq!(r.real_bytes, 13 * MIB);
        assert_eq!(r.first_bad_offset, Some(13 * MIB));
    }

    #[test]
    fn full_scattered_bad_sectors_is_damage() {
        let mut d = FakeDrive::genuine(16 * MIB)
            .with_rotten_sector(MIB)
            .with_rotten_sector(16 * MIB - 512);
        let r = full(&mut d);
        assert_eq!(r.verdict, Verdict::Damaged);
        assert_eq!(r.bad_bytes, 1024);
        assert_eq!(r.real_bytes, 16 * MIB - 1024);
    }
}
