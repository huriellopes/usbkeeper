//! Apagar com segurança por sobrescrita: zeros ou dados pseudoaleatórios,
//! 1 a 3 passadas e verificação opcional. Em memória flash, sobrescrever não
//! garante apagar as áreas de reserva do controlador (o usbkeeper avisa).

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use serde::Serialize;

use crate::CoreError;
use crate::blockdev::{
    AlignedBuf, BlockDevice, Phase, Progress, SECTOR, XorShift, align_down, cancelled,
};

const CHUNK: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum WipePattern {
    Zeros,
    Random,
}

#[derive(Debug, Clone, Copy)]
pub struct WipeOptions {
    pub pattern: WipePattern,
    /// 1 a 3.
    pub passes: u8,
    pub verify: bool,
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct WipeReport {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub bytes: u64,
    pub passes: u8,
    pub pattern: WipePattern,
    /// `None` sem verificação; `Some(true)` se só restou o padrão gravado.
    pub verified: Option<bool>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub mismatched_bytes: u64,
    pub seconds: f64,
}

/// Conteúdo de um bloco: zeros, ou XorShift com semente que depende da
/// passada e do endereço (dá para regenerar na verificação sem guardar nada).
fn fill(buf: &mut [u8], pattern: WipePattern, seed: u64, pass: u8, offset: u64) {
    match pattern {
        WipePattern::Zeros => buf.fill(0),
        WipePattern::Random => {
            let mut rng = XorShift::new(
                seed ^ (u64::from(pass) << 56) ^ offset.wrapping_mul(0x9E37_79B9_7F4A_7C15),
            );
            for chunk in buf.chunks_mut(8) {
                let bytes = rng.next_u64().to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
        }
    }
}

pub fn wipe(
    dev: &mut dyn BlockDevice,
    opts: WipeOptions,
    progress: &mut dyn Progress,
    cancel: &AtomicBool,
) -> Result<WipeReport, CoreError> {
    if !(1..=3).contains(&opts.passes) {
        return Err(CoreError::Backend("passadas devem ser de 1 a 3".into()));
    }
    let size = align_down(dev.size(), SECTOR as u64);
    let started = Instant::now();
    let mut buf = AlignedBuf::new(CHUNK);

    for pass in 1..=opts.passes {
        let mut offset = 0;
        while offset < size {
            if cancelled(cancel) {
                return Err(CoreError::Cancelled);
            }
            let len = (size - offset).min(CHUNK as u64) as usize;
            fill(&mut buf[..len], opts.pattern, opts.seed, pass, offset);
            dev.write_at(offset, &buf[..len])?;
            offset += len as u64;
            progress.report(Phase::Wipe, offset, size);
        }
        dev.flush()?;
    }

    let (verified, mismatched_bytes) = if opts.verify {
        let mut expected = AlignedBuf::new(CHUNK);
        let mut mismatched = 0u64;
        let mut offset = 0;
        while offset < size {
            if cancelled(cancel) {
                return Err(CoreError::Cancelled);
            }
            let len = (size - offset).min(CHUNK as u64) as usize;
            dev.read_at(offset, &mut buf[..len])?;
            fill(
                &mut expected[..len],
                opts.pattern,
                opts.seed,
                opts.passes,
                offset,
            );
            mismatched += buf[..len]
                .iter()
                .zip(&expected[..len])
                .filter(|(a, b)| a != b)
                .count() as u64;
            offset += len as u64;
            progress.report(Phase::Verify, offset, size);
        }
        (Some(mismatched == 0), mismatched)
    } else {
        (None, 0)
    };

    Ok(WipeReport {
        bytes: size,
        passes: opts.passes,
        pattern: opts.pattern,
        verified,
        mismatched_bytes,
        seconds: started.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::Silent;
    use crate::fakedrive::FakeDrive;

    fn opts(pattern: WipePattern, passes: u8, verify: bool) -> WipeOptions {
        WipeOptions {
            pattern,
            passes,
            verify,
            seed: 7,
        }
    }

    #[test]
    fn zeros_leave_only_zeros_and_verify() {
        let mut d = FakeDrive::genuine(8 << 20);
        d.seed_user_data(&[0, 4096, 5 << 20]);
        let r = wipe(
            &mut d,
            opts(WipePattern::Zeros, 1, true),
            &mut Silent,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(r.verified, Some(true));
        assert_eq!(r.bytes, 8 << 20);
        assert!(
            d.physical_snapshot()
                .iter()
                .all(|(_, c)| c.iter().all(|&b| b == 0))
        );
    }

    struct Rec<'a>(&'a mut Vec<(Phase, u64, u64)>);
    impl Progress for Rec<'_> {
        fn report(&mut self, p: Phase, d: u64, t: u64) {
            self.0.push((p, d, t));
        }
    }

    #[test]
    fn random_passes_are_counted_and_verified_by_regeneration() {
        let mut d = FakeDrive::genuine(4 << 20);
        let mut events = Vec::new();
        let r = wipe(
            &mut d,
            opts(WipePattern::Random, 2, true),
            &mut Rec(&mut events),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(r.verified, Some(true));
        assert_eq!(r.passes, 2);
        let finished = events
            .iter()
            .filter(|(p, d, t)| *p == Phase::Wipe && d == t)
            .count();
        assert_eq!(finished, 2, "uma conclusão por passada");
        assert!(
            d.physical_snapshot()
                .iter()
                .any(|(_, c)| c.iter().any(|&b| b != 0)),
            "aleatório não deixa só zeros"
        );
    }

    #[test]
    fn verify_catches_sector_that_does_not_store() {
        let mut d = FakeDrive::genuine(4 << 20).with_rotten_sector(1 << 20);
        let r = wipe(
            &mut d,
            opts(WipePattern::Zeros, 1, true),
            &mut Silent,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(r.verified, Some(false));
        assert_eq!(r.mismatched_bytes, 512);
    }

    #[test]
    fn without_verify_there_is_no_verdict() {
        let mut d = FakeDrive::genuine(2 << 20);
        let r = wipe(
            &mut d,
            opts(WipePattern::Zeros, 1, false),
            &mut Silent,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(r.verified, None);
    }

    #[test]
    fn cancel_stops() {
        let err = wipe(
            &mut FakeDrive::genuine(4 << 20),
            opts(WipePattern::Zeros, 1, false),
            &mut Silent,
            &AtomicBool::new(true),
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Cancelled));
    }

    #[test]
    fn passes_must_be_one_to_three() {
        for bad in [0, 4] {
            assert!(
                wipe(
                    &mut FakeDrive::genuine(1 << 20),
                    opts(WipePattern::Zeros, bad, false),
                    &mut Silent,
                    &AtomicBool::new(false)
                )
                .is_err()
            );
        }
    }
}
