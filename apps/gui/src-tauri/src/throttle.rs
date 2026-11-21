//! Limita os eventos de progresso enviados à interface: a CLI manda a cada
//! 100 ms e redesenhar tudo isso só gasta CPU. Troca de fase passa na hora.

use std::time::{Duration, Instant};

pub const EVERY: Duration = Duration::from_millis(250);

pub struct Throttle {
    every: Duration,
    last: Option<(String, Instant)>,
}

impl Throttle {
    pub fn new(every: Duration) -> Self {
        Self { every, last: None }
    }

    /// `true` se este progresso deve ser emitido agora.
    pub fn pass(&mut self, phase: &str, now: Instant) -> bool {
        let emit = match &self.last {
            None => true,
            Some((p, at)) => p != phase || now.duration_since(*at) >= self.every,
        };
        if emit {
            self.last = Some((phase.to_string(), now));
        }
        emit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_rate_within_phase() {
        let t0 = Instant::now();
        let mut t = Throttle::new(EVERY);
        assert!(t.pass("write", t0));
        assert!(!t.pass("write", t0 + Duration::from_millis(100)));
        assert!(!t.pass("write", t0 + Duration::from_millis(249)));
        assert!(t.pass("write", t0 + Duration::from_millis(250)));
        assert!(!t.pass("write", t0 + Duration::from_millis(300)));
    }

    #[test]
    fn phase_change_passes_immediately() {
        let t0 = Instant::now();
        let mut t = Throttle::new(EVERY);
        assert!(t.pass("write", t0));
        assert!(t.pass("verify", t0 + Duration::from_millis(10)));
        assert!(!t.pass("verify", t0 + Duration::from_millis(20)));
    }
}
