//! Modo "dirigido por programa" (usado pela interface gráfica via pkexec):
//! progresso como linhas JSON no stderr e cancelamento pela linha `cancel`
//! no stdin (o usuário não pode mandar sinal a um processo root).

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use usbkeeper_core::blockdev::{Phase, Progress};

use crate::checks::Bar;

#[derive(Debug, Clone, Copy, Default)]
pub struct Mode {
    pub progress_json: bool,
    pub cancel_stdin: bool,
}

static MODE: OnceLock<Mode> = OnceLock::new();

pub fn set(mode: Mode) {
    let _ = MODE.set(mode);
}

pub fn get() -> Mode {
    MODE.get().copied().unwrap_or_default()
}

#[derive(Serialize)]
struct Line<'a> {
    phase: &'a str,
    label: &'a str,
    done: u64,
    total: u64,
    bytes: bool,
}

pub fn progress_line(phase: Phase, done: u64, total: u64) -> String {
    serde_json::to_string(&Line {
        phase: phase.id(),
        label: phase.label(),
        done,
        total,
        bytes: phase.counts_bytes(),
    })
    .expect("serializável")
}

/// Etapa do `format` (não é uma `Phase` do core).
pub fn step_line(label: &str, done: u64, total: u64) -> String {
    serde_json::to_string(&Line {
        phase: "format",
        label,
        done,
        total,
        bytes: false,
    })
    .expect("serializável")
}

pub fn error_line(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

const THROTTLE: Duration = Duration::from_millis(100);

pub struct JsonProgress<W: Write> {
    out: W,
    last: Option<(Phase, Instant)>,
}

impl<W: Write> JsonProgress<W> {
    pub fn new(out: W) -> Self {
        Self { out, last: None }
    }

    #[cfg(test)]
    pub fn into_inner(self) -> W {
        self.out
    }
}

impl<W: Write> Progress for JsonProgress<W> {
    fn report(&mut self, phase: Phase, done: u64, total: u64) {
        let now = Instant::now();
        let emit = match self.last {
            Some((p, at)) => p != phase || done >= total || now - at >= THROTTLE,
            None => true,
        };
        if emit {
            self.last = Some((phase, now));
            let _ = writeln!(self.out, "{}", progress_line(phase, done, total));
            let _ = self.out.flush();
        }
    }
}

/// Escolhe a forma de progresso conforme as flags.
pub enum Reporter {
    Bar(Bar),
    Json(JsonProgress<std::io::Stderr>),
    Silent,
}

impl Reporter {
    pub fn new(json_output: bool) -> Self {
        if get().progress_json {
            Reporter::Json(JsonProgress::new(std::io::stderr()))
        } else if json_output {
            Reporter::Silent
        } else {
            Reporter::Bar(Bar::new())
        }
    }

    pub fn finish(&mut self) {
        if let Reporter::Bar(bar) = self {
            bar.finish();
        }
    }
}

impl Progress for Reporter {
    fn report(&mut self, phase: Phase, done: u64, total: u64) {
        match self {
            Reporter::Bar(b) => b.report(phase, done, total),
            Reporter::Json(j) => j.report(phase, done, total),
            Reporter::Silent => {}
        }
    }
}

/// Lê linhas de `reader`; `cancel` liga a flag. Fim da entrada é ignorado.
pub fn watch_reader<R: BufRead + Send + 'static>(
    reader: R,
    flag: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        for line in reader.lines() {
            match line {
                Ok(l) if l.trim() == "cancel" => {
                    flag.store(true, Ordering::SeqCst);
                    return;
                }
                Ok(_) => {}
                Err(_) => return,
            }
        }
    })
}

pub fn watch_stdin(flag: Arc<AtomicBool>) {
    let _ = watch_reader(std::io::BufReader::new(std::io::stdin()), flag);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use usbkeeper_core::blockdev::Phase;

    #[test]
    fn progress_line_is_json() {
        let line = progress_line(Phase::Probe, 12, 129);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["phase"], "probe");
        assert_eq!(v["label"], "gravando blocos de teste");
        assert_eq!(v["done"], 12);
        assert_eq!(v["total"], 129);
        assert_eq!(v["bytes"], false);
    }

    #[test]
    fn json_progress_throttles_but_keeps_phase_changes_and_completion() {
        let mut p = JsonProgress::new(Vec::new());
        p.report(Phase::Write, 1, 100);
        p.report(Phase::Write, 2, 100); // engolido (< 100 ms)
        p.report(Phase::Write, 100, 100); // fim da etapa sempre sai
        p.report(Phase::Verify, 1, 100); // troca de etapa sempre sai
        let out = String::from_utf8(p.into_inner()).unwrap();
        let done: Vec<u64> = out
            .lines()
            .map(|l| {
                serde_json::from_str::<serde_json::Value>(l).unwrap()["done"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        assert_eq!(done, [1, 100, 1]);
    }

    #[test]
    fn cancel_line_sets_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        watch_reader(Cursor::new("oi\ncancel\n"), flag.clone())
            .join()
            .unwrap();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn eof_without_cancel_does_not_cancel() {
        let flag = Arc::new(AtomicBool::new(false));
        watch_reader(Cursor::new(""), flag.clone()).join().unwrap();
        assert!(!flag.load(Ordering::SeqCst));
    }

    #[test]
    fn error_line_is_json() {
        let v: serde_json::Value = serde_json::from_str(&error_line("deu \"ruim\"")).unwrap();
        assert_eq!(v["error"], "deu \"ruim\"");
    }
}
