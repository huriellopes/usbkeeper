//! `repair`/`reparar`: verifica (`--check`, não altera nada) ou repara os
//! sistemas de arquivos das partições.

use anyhow::{Result, bail};
use usbkeeper_core::repair::{RepairReport, RepairStatus};
use usbkeeper_history::kind;

use crate::backend::Backend;
use crate::destructive::{Guard, confirm_and_take, precheck, take};
use crate::store;

pub fn repair(b: &Backend, device: &str, check: bool, guard: Guard, json: bool) -> Result<()> {
    if cfg!(windows) {
        anyhow::bail!(crate::backend::WINDOWS_NEXT);
    }
    let dev = b.enumerator.get(device)?;
    precheck(&dev, &guard)?;
    if dev.partitions.is_empty() {
        bail!("{} não tem partições para verificar", dev.path);
    }
    if !json {
        eprintln!(
            "{} {}  {}",
            if check {
                "Verificando (não altera nada)"
            } else {
                "Reparando"
            },
            dev.path,
            dev.display_name()
        );
        if !check {
            eprintln!(
                "O reparo pode mover arquivos danificados para pastas como FSCK0000.REC ou lost+found. Tenha backup."
            );
        }
    }
    let current = if check {
        take(b, &dev, &guard)?
    } else {
        confirm_and_take(b, &dev, &guard)?
    };
    let report = b
        .repairer
        .repair(&current, check, &mut |label, done, total| {
            if crate::machine::get().progress_json {
                eprintln!(
                    "{}",
                    crate::machine::step_line_phase("repair", label, done, total)
                );
            } else if !json {
                eprintln!("[{done}/{total}] {label}");
            }
        })?;
    store::record(
        &current,
        kind::REPAIR,
        Some(report.verdict()),
        &summary(&report),
        serde_json::json!(report),
    );
    if json {
        return crate::print_json(&serde_json::json!({ "device": current.path, "report": report }));
    }
    print_report(&report);
    Ok(())
}

pub fn summary(r: &RepairReport) -> String {
    let count = |s: RepairStatus| r.partitions.iter().filter(|p| p.status == s).count();
    format!(
        "{}: {} sem problemas, {} corrigidas, {} com erros",
        if r.check_only {
            "verificação"
        } else {
            "reparo"
        },
        count(RepairStatus::Clean),
        count(RepairStatus::Fixed),
        count(RepairStatus::ErrorsRemain) + count(RepairStatus::Failed)
    )
}

fn print_report(r: &RepairReport) {
    println!();
    for p in &r.partitions {
        let label = match p.status {
            RepairStatus::Clean => "✔ sem problemas",
            RepairStatus::Fixed => "✔ corrigido",
            RepairStatus::ErrorsRemain if r.check_only => "⚠ tem problemas: rode o reparo",
            RepairStatus::ErrorsRemain => "✘ ainda tem erros",
            RepairStatus::Unsupported => "– não verificado",
            RepairStatus::Failed => "✘ a ferramenta falhou",
        };
        println!(
            "{}  ({})  {label}",
            p.path,
            p.fstype.as_deref().unwrap_or("?")
        );
        if let Some(n) = &p.note {
            println!("  {n}");
        }
        if matches!(
            p.status,
            RepairStatus::ErrorsRemain | RepairStatus::Failed | RepairStatus::Fixed
        ) {
            for l in p.log.iter().rev().take(6).rev() {
                println!("    {l}");
            }
        }
    }
    if let Some(w) = &r.table_warning {
        println!("\n⚠ {w}");
    }
    if r.verdict() == "errors" {
        println!(
            "\nSe os erros continuarem, veja a saúde do disco: usbkeeper health <disco> --scan"
        );
    }
    println!("Entenda: usbkeeper guia reparar");
}
