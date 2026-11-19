//! `health`: SMART (quando existe) + varredura de superfície somente leitura.

use anyhow::{Result, bail};
use serde_json::json;
use usbkeeper_core::blockdev::Progress;
use usbkeeper_core::scan::{self, ScanReport};
use usbkeeper_core::units::human_bytes;
use usbkeeper_core::{CoreError, Device};
use usbkeeper_history::kind;
use usbkeeper_smart::{HealthReport, HealthVerdict};

use crate::backend::Backend;
use crate::checks::{cancel_flag, duration, open_checked};
use crate::store;

pub struct HealthArgs {
    pub device: String,
    pub scan: bool,
    pub json: bool,
}

pub fn health(b: &Backend, args: HealthArgs) -> Result<()> {
    let device = b.enumerator.get(&args.device)?;
    if !args.json {
        eprintln!("Lendo SMART de {}...", device.path);
    }
    let smart = usbkeeper_smart::read(&device.path);
    // Últimos autotestes registrados no próprio disco (quando há SMART).
    let selftests: Vec<usbkeeper_smart::selftest::SelfTestEntry> = smart
        .device_type
        .as_deref()
        .filter(|_| smart.has_smart())
        .and_then(|dtype| {
            usbkeeper_smart::selftest::poll(
                &device.path,
                dtype,
                usbkeeper_smart::selftest::SelfTestKind::Short,
            )
            .ok()
        })
        .map(|s| s.log.into_iter().take(3).collect())
        .unwrap_or_default();

    let scan = if args.scan {
        let mut raw = open_checked(b, &device, false)?;
        let cancel = cancel_flag();
        let mut reporter = crate::machine::Reporter::new(args.json);
        let progress: &mut dyn Progress = &mut reporter;
        let result = scan::read_scan(raw.as_mut(), progress, &cancel);
        reporter.finish();
        match result {
            Err(CoreError::Cancelled) => bail!("varredura cancelada (nada foi gravado)"),
            other => Some(other?),
        }
    } else {
        None
    };

    // Sem permissão o resultado não diz nada sobre o disco: não registra.
    if !smart.access_denied {
        store::record(
            &device,
            kind::HEALTH,
            Some(&store::verdict_name(&smart.verdict)),
            &smart_summary(&smart),
            json!(smart),
        );
    }
    if let Some(s) = &scan {
        store::record(
            &device,
            kind::SCAN,
            Some(if s.is_clean() { "clean" } else { "unreadable" }),
            &scan_summary(s),
            json!(s),
        );
    }

    if args.json {
        crate::print_json(
            &json!({ "device": device, "smart": smart, "scan": scan, "selftests": selftests }),
        )?;
    } else {
        print_smart(&device, &smart);
        if !selftests.is_empty() {
            println!("  Últimos autotestes do disco:");
            for t in &selftests {
                println!(
                    "    {} {:<18} {}{}",
                    match t.passed {
                        Some(true) => "✔",
                        Some(false) => "✘",
                        None => "–",
                    },
                    t.kind,
                    t.status,
                    t.hours
                        .map(|h| format!(" (com {h} h de uso)"))
                        .unwrap_or_default()
                );
            }
        }
        if let Some(s) = &scan {
            print_scan(s);
        } else if !smart.has_smart() && !smart.access_denied {
            let fake_hint = if device.transport == usbkeeper_core::Transport::Usb {
                format!(
                    "\n(e, se for pendrive/cartão, confira se não é falso: usbkeeper fake-check {})",
                    device.name
                )
            } else {
                String::new()
            };
            println!(
                "\nSem SMART, a saúde se avalia lendo o disco: usbkeeper health {} --scan{fake_hint}",
                device.name
            );
        }
    }
    Ok(())
}

pub fn smart_summary(r: &HealthReport) -> String {
    match r.verdict {
        HealthVerdict::Unknown => format!("sem SMART: {}", r.notes.join("; ")),
        HealthVerdict::Good => format!(
            "SMART ok{}{}",
            r.power_on_hours
                .map(|h| format!(", {h} h ligado"))
                .unwrap_or_default(),
            r.temperature_c
                .map(|t| format!(", {t} °C"))
                .unwrap_or_default()
        ),
        _ => r.notes.join("; "),
    }
}

pub fn scan_summary(s: &ScanReport) -> String {
    if s.is_clean() {
        format!(
            "varredura sem erros ({}, {:.1} MB/s)",
            human_bytes(s.scanned_bytes),
            s.read_mbps
        )
    } else {
        format!(
            "{} blocos de 4 KiB ilegíveis (primeiro em {})",
            s.unreadable_blocks,
            human_bytes(s.unreadable_offsets.first().copied().unwrap_or(0))
        )
    }
}

fn print_smart(device: &Device, r: &HealthReport) {
    println!("\n{}  {}", device.path, device.display_name());
    let headline = match r.verdict {
        HealthVerdict::Good => "✔ SMART: saudável",
        HealthVerdict::Warning => "⚠ SMART: sinais de desgaste ou defeito",
        HealthVerdict::Failing => "✘ SMART: FALHA IMINENTE: copie seus dados agora",
        HealthVerdict::Unknown => "– SMART indisponível",
    };
    println!("{headline}");
    for note in &r.notes {
        println!("  - {note}");
    }
    if matches!(r.verdict, HealthVerdict::Warning | HealthVerdict::Failing) {
        println!("  Entenda cada item: usbkeeper guia saude");
    }
    if !r.has_smart() {
        return;
    }
    let opt = |v: Option<u64>| v.map(|x| x.to_string()).unwrap_or_else(|| "-".into());
    if let Some(t) = &r.device_type {
        println!(
            "  Acesso:              -d {t}{}",
            r.protocol
                .as_ref()
                .map(|p| format!(" ({p})"))
                .unwrap_or_default()
        );
    }
    if let Some(t) = r.temperature_c {
        println!("  Temperatura:         {t} °C");
    }
    println!("  Horas ligado:        {}", opt(r.power_on_hours));
    println!("  Ciclos de energia:   {}", opt(r.power_cycles));
    if r.protocol.as_deref() == Some("NVMe") {
        println!("  Vida útil usada:     {}%", opt(r.percentage_used));
        println!("  Erros de mídia:      {}", opt(r.media_errors));
    } else {
        println!("  Setores realocados:  {}", opt(r.reallocated_sectors));
        println!("  Setores pendentes:   {}", opt(r.pending_sectors));
        println!("  Incorrigíveis:       {}", opt(r.uncorrectable_sectors));
        println!("  Erros CRC (cabo):    {}", opt(r.crc_errors));
    }
}

fn print_scan(s: &ScanReport) {
    println!("\nVarredura de superfície (somente leitura):");
    if s.is_clean() {
        println!("  ✔ nenhum bloco ilegível");
    } else {
        println!("  ✘ {} blocos de 4 KiB ilegíveis", s.unreadable_blocks);
        for off in s.unreadable_offsets.iter().take(10) {
            println!("    em {} ({off})", human_bytes(*off));
        }
        if s.unreadable_offsets.len() > 10 {
            println!("    ... (lista completa no --json)");
        }
    }
    println!(
        "  Lidos {} a {:.1} MB/s em {}",
        human_bytes(s.scanned_bytes),
        s.read_mbps,
        duration(s.seconds)
    );
    if s.slow_chunks > 0 {
        println!(
            "  ⚠ {} trechos de 1 MiB lentos (>500 ms; pior: {} ms): regiões degradadas",
            s.slow_chunks, s.slowest_chunk_ms
        );
    }
}
