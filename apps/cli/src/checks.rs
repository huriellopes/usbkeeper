//! `fake-check` e `bench`: testes que leem/gravam o dispositivo bruto.

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use usbkeeper_core::bench::{self, BenchOptions, BenchResult};
use usbkeeper_core::blockdev::{Phase, Progress, Silent, fresh_seed};
use usbkeeper_core::fakecheck::{self, FakeReport, QuickOptions, Verdict};
use usbkeeper_core::units::human_bytes;
use usbkeeper_core::{CoreError, Device};

use crate::backend::Backend;
use crate::destructive::{Guard, confirm_and_take, precheck};

/// Barra de progresso em uma linha (stderr), atualizada no máximo 5x/s.
struct Bar {
    phase: Option<Phase>,
    phase_started: Instant,
    last_draw: Instant,
}

impl Bar {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            phase: None,
            phase_started: now,
            last_draw: now - Duration::from_secs(1),
        }
    }

    fn finish(&mut self) {
        if self.phase.take().is_some() {
            eprintln!();
        }
    }
}

fn duration(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    match secs {
        s if s >= 3600 => format!("{}h{:02}min", s / 3600, (s % 3600) / 60),
        s if s >= 60 => format!("{}min{:02}s", s / 60, s % 60),
        s => format!("{s}s"),
    }
}

impl Progress for Bar {
    fn report(&mut self, phase: Phase, done: u64, total: u64) {
        let now = Instant::now();
        if self.phase != Some(phase) {
            self.finish();
            self.phase = Some(phase);
            self.phase_started = now;
        } else if now - self.last_draw < Duration::from_millis(200) && done < total {
            return;
        }
        self.last_draw = now;
        let pct = if total == 0 {
            100.0
        } else {
            done as f64 * 100.0 / total as f64
        };
        let mut extra = String::new();
        if phase.counts_bytes() {
            let elapsed = (now - self.phase_started).as_secs_f64();
            if elapsed > 0.5 && done > 0 {
                let rate = done as f64 / elapsed;
                extra = format!(
                    "  {}/s  restam ~{}",
                    human_bytes(rate as u64),
                    duration((total - done) as f64 / rate)
                );
            }
        }
        eprint!("\r\x1b[2K  {:<30} {:>5.1}%{extra}", phase.label(), pct);
        let _ = std::io::stderr().flush();
    }
}

/// Ctrl+C liga a flag; os algoritmos param no próximo bloco e o teste
/// rápido ainda restaura os blocos originais.
fn cancel_flag() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let handler_flag = flag.clone();
    let _ = ctrlc::set_handler(move || {
        if handler_flag.swap(true, Ordering::SeqCst) {
            eprintln!("\nSegundo Ctrl+C: saindo sem esperar.");
            std::process::exit(130);
        }
        eprintln!("\nCancelando no próximo bloco... (Ctrl+C de novo força a saída)");
    });
    flag
}

fn open_checked(
    b: &Backend,
    device: &Device,
    write: bool,
) -> Result<Box<dyn usbkeeper_core::BlockDevice>> {
    let raw = (b.open_raw)(&device.path, write)?;
    if raw.size() != device.size_bytes {
        bail!(
            "tamanho lido de {} ({} bytes) difere do informado pelo sistema ({} bytes); abortando",
            device.path,
            raw.size(),
            device.size_bytes
        );
    }
    Ok(raw)
}

pub struct FakeCheckArgs {
    pub device: String,
    pub full: bool,
    pub no_backup: bool,
    pub guard: Guard,
    pub json: bool,
}

pub fn fake_check(b: &Backend, args: FakeCheckArgs) -> Result<()> {
    let device = b.enumerator.get(&args.device)?;
    precheck(&device, &args.guard)?;

    println!(
        "Alvo: {}  {}  ({} anunciados, série {})",
        device.path,
        device.display_name(),
        human_bytes(device.size_bytes),
        device.serial.as_deref().unwrap_or("-")
    );
    if args.full {
        println!(
            "Teste COMPLETO: grava e lê o disco inteiro. TODOS OS DADOS SERÃO APAGADOS.\n\
             Pode levar horas (a {} a 10 MB/s: ~{}).",
            human_bytes(device.size_bytes),
            duration(device.size_bytes as f64 * 2.0 / 10e6)
        );
    } else if args.no_backup {
        println!(
            "Teste rápido SEM backup: os blocos de teste corrompem o sistema de arquivos atual."
        );
    } else {
        println!(
            "Teste rápido: grava ~200 blocos de 4 KiB espalhados e restaura os originais no fim.\n\
             Os dados devem sobreviver, mas tenha backup do que for importante."
        );
    }

    let current = confirm_and_take(b, &device, &args.guard)?;
    let mut raw = open_checked(b, &current, true)?;
    let cancel = cancel_flag();
    let mut bar = Bar::new();
    let mut silent = Silent;
    let progress: &mut dyn Progress = if args.json { &mut silent } else { &mut bar };

    let seed = fresh_seed();
    let result = if args.full {
        fakecheck::full_check(raw.as_mut(), seed, progress, &cancel)
    } else {
        let opts = QuickOptions {
            seed,
            backup: !args.no_backup,
        };
        fakecheck::quick_probe(raw.as_mut(), opts, progress, &cancel)
    };
    bar.finish();

    let report = match result {
        Ok(r) => r,
        Err(CoreError::Cancelled) if args.full => {
            bail!("cancelado. O disco ficou com dados de teste: formate-o antes de usar.")
        }
        Err(CoreError::Cancelled) if !args.no_backup => {
            bail!("cancelado. Os blocos originais foram restaurados.")
        }
        Err(e) => return Err(e.into()),
    };

    if args.json {
        let out = serde_json::json!({ "device": current, "report": report });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_report(&current, &report);
    }
    Ok(())
}

fn print_report(device: &Device, r: &FakeReport) {
    println!();
    match r.verdict {
        Verdict::Genuine => {
            println!("✔ GENUÍNO: a capacidade anunciada guarda dados corretamente.")
        }
        Verdict::Fake => println!("✘ FALSIFICADO: a capacidade real é menor que a anunciada."),
        Verdict::Damaged => {
            println!("⚠ COM DEFEITO: capacidade correta, mas há setores que não guardam dados.")
        }
    }
    let pct = r.real_bytes as f64 * 100.0 / r.advertised_bytes.max(1) as f64;
    println!("  Anunciado:     {}", human_bytes(r.advertised_bytes));
    println!(
        "  Real:          {} ({pct:.1}% do anunciado){}",
        human_bytes(r.real_bytes),
        if r.mode == fakecheck::CheckMode::Quick && r.verdict == Verdict::Fake {
            ", precisão de 1 MB"
        } else {
            ""
        }
    );
    if let Some(m) = r.wrap_module_bytes {
        println!(
            "  Dá a volta a cada {}: o que é gravado além disso sobrescreve o início.",
            human_bytes(m)
        );
    }
    if let Some(off) = r.first_bad_offset {
        println!("  1º setor ruim: posição {} ({off})", human_bytes(off));
    }
    if r.mode == fakecheck::CheckMode::Full {
        println!(
            "  Setores: {} íntegros, {} com lixo, {} sobrescritos por outro endereço",
            human_bytes(r.ok_bytes),
            human_bytes(r.bad_bytes),
            human_bytes(r.aliased_bytes)
        );
    }
    if let (Some(w), Some(rd)) = (r.write_mbps, r.read_mbps) {
        println!("  Velocidade:    escrita {w:.1} MB/s, leitura {rd:.1} MB/s");
    }
    if let Some(p) = r.probes {
        print!("  Blocos testados: {p}");
        match r.restored_blocks {
            Some(n) => println!(", {n} restaurados"),
            None => println!(" (sem backup)"),
        }
    }
    println!("  Duração:       {}", duration(r.seconds));

    match (r.verdict, r.mode) {
        (Verdict::Fake, _) => println!(
            "\nNão guarde nada importante nele: arquivos além de {} se perdem sem aviso.\n\
             Guarde este resultado (--json) como prova para pedir reembolso.",
            human_bytes(r.real_bytes)
        ),
        (Verdict::Damaged, _) => {
            println!("\nO dispositivo está falhando; copie o que precisar e substitua-o.")
        }
        (Verdict::Genuine, fakecheck::CheckMode::Quick) => println!(
            "\nO teste rápido pega os tipos comuns de falsificação. Para certeza absoluta: --full (apaga tudo)."
        ),
        _ => {}
    }
    if r.mode == fakecheck::CheckMode::Full {
        println!(
            "O teste completo deixou o disco sem sistema de arquivos. Formate com:\n  usbkeeper format {} --fs exfat",
            device.name
        );
    }
}

pub struct BenchArgs {
    pub device: String,
    pub write: bool,
    pub sample_mb: u64,
    pub guard: Guard,
    pub json: bool,
}

pub fn bench(b: &Backend, args: BenchArgs) -> Result<()> {
    let device = b.enumerator.get(&args.device)?;
    let current = if args.write {
        precheck(&device, &args.guard)?;
        println!(
            "Benchmark com ESCRITA em {} ({}): sobrescreve ~{} MB do início e do meio do disco.",
            device.path,
            device.display_name(),
            args.sample_mb
        );
        confirm_and_take(b, &device, &args.guard)?
    } else {
        device
    };

    let mut raw = open_checked(b, &current, args.write)?;
    let cancel = cancel_flag();
    let mut bar = Bar::new();
    let mut silent = Silent;
    let progress: &mut dyn Progress = if args.json { &mut silent } else { &mut bar };
    let options = BenchOptions {
        write: args.write,
        sample_bytes: args.sample_mb << 20,
        seed: fresh_seed(),
        ..Default::default()
    };
    let result = bench::run(raw.as_mut(), options, progress, &cancel);
    bar.finish();
    let result = match result {
        Err(CoreError::Cancelled) => bail!("cancelado"),
        other => other?,
    };

    if args.json {
        let out = serde_json::json!({ "device": current, "bench": result });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_bench(&current, &result);
    }
    Ok(())
}

fn print_bench(device: &Device, r: &BenchResult) {
    println!("\n{}  {}", device.path, device.display_name());
    println!("  Leitura sequencial:   {:>8.1} MB/s", r.seq_read_mbps);
    println!(
        "  Leitura aleatória 4K: {:>8.0} IOPS ({:.1} MB/s)",
        r.rand_read_iops,
        r.rand_read_iops * 4096.0 / 1e6
    );
    if let (Some(w), Some(rw)) = (r.seq_write_mbps, r.rand_write_iops) {
        println!("  Escrita sequencial:   {w:>8.1} MB/s");
        println!(
            "  Escrita aleatória 4K: {rw:>8.0} IOPS ({:.2} MB/s)",
            rw * 4096.0 / 1e6
        );
        if rw < 50.0 {
            println!(
                "  ⚠ Escrita 4K muito lenta: ruim para rodar programas ou sistema a partir dele."
            );
        }
    }
    if let Some(usb) = &device.usb {
        if let Some(limit) = bench::practical_limit_mbps(usb.speed) {
            println!(
                "  Link: {} (teto prático ~{limit:.0} MB/s){}",
                usb.speed.label(),
                if r.seq_read_mbps >= limit * 0.85 {
                    ". A leitura está no limite do link."
                } else {
                    ""
                }
            );
        }
        if usb.is_bottlenecked() {
            println!("  ⚠ Dispositivo USB 3 conectado como USB 2: troque de porta, hub ou cabo.");
        }
    }
}
