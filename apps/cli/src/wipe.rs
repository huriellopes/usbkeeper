//! `wipe`/`apagar`: sobrescreve o disco inteiro (ou descarta com TRIM) para
//! vender, doar ou descartar. Mesmas travas do format, mais aviso explícito.

use anyhow::{Result, bail};
use usbkeeper_core::CoreError;
use usbkeeper_core::blockdev::fresh_seed;
use usbkeeper_core::units::human_bytes;
use usbkeeper_core::wipe::{WipeOptions, WipePattern, WipeReport};
use usbkeeper_history::kind;

use crate::backend::Backend;
use crate::checks::{cancel_flag, duration, open_checked};
use crate::destructive::{Guard, confirm_and_take, precheck};
use crate::store;

pub struct WipeArgs {
    pub device: String,
    pub random: bool,
    pub passes: u8,
    pub verify: bool,
    pub discard: bool,
    pub guard: Guard,
    pub json: bool,
}

const FLASH_NOTE: &str = "Em pendrive, cartão e SSD, sobrescrever não garante apagar as áreas de reserva do \
controlador. Para garantia total, use criptografia desde o início (veja: usbkeeper guia apagar).";

pub fn wipe(b: &Backend, a: WipeArgs) -> Result<()> {
    let device = b.enumerator.get(&a.device)?;
    precheck(&device, &a.guard)?;
    eprintln!(
        "APAGAR TUDO em {}  {} ({}). Isto é irreversível.",
        device.path,
        device.display_name(),
        human_bytes(device.size_bytes)
    );
    if a.discard {
        eprintln!("Método: descarte (TRIM) do disco inteiro.");
    } else {
        eprintln!(
            "Método: sobrescrever com {} em {} passada(s){}. Estimativa a 30 MB/s: ~{}.",
            if a.random {
                "dados aleatórios"
            } else {
                "zeros"
            },
            a.passes,
            if a.verify { " e verificar" } else { "" },
            duration(device.size_bytes as f64 * f64::from(a.passes + u8::from(a.verify)) / 30e6)
        );
    }
    eprintln!("{FLASH_NOTE}");
    let current = confirm_and_take(b, &device, &a.guard)?;

    if a.discard {
        discard_device(&current.path)?;
        store::record(
            &current,
            kind::WIPE,
            Some("discarded"),
            "descartado (TRIM)",
            serde_json::json!({"method": "discard"}),
        );
        if a.json {
            return crate::print_json(
                &serde_json::json!({ "device": current.path, "method": "discard" }),
            );
        }
        println!(
            "✔ {} descartado. Formate antes de usar: usbkeeper format {} --fs exfat",
            current.path, current.name
        );
        return Ok(());
    }

    let mut raw = open_checked(b, &current, true)?;
    let cancel = cancel_flag();
    let mut reporter = crate::machine::Reporter::new(a.json);
    let opts = WipeOptions {
        pattern: if a.random {
            WipePattern::Random
        } else {
            WipePattern::Zeros
        },
        passes: a.passes,
        verify: a.verify,
        seed: fresh_seed(),
    };
    let result = usbkeeper_core::wipe::wipe(raw.as_mut(), opts, &mut reporter, &cancel);
    reporter.finish();
    let report = match result {
        Err(CoreError::Cancelled) => {
            bail!("cancelado: o disco ficou parcialmente apagado; formate antes de usar")
        }
        other => other?,
    };
    let verdict = if report.verified == Some(false) {
        "verify_failed"
    } else {
        "wiped"
    };
    store::record(
        &current,
        kind::WIPE,
        Some(verdict),
        &summary(&report),
        serde_json::json!(report),
    );
    if a.json {
        return crate::print_json(&serde_json::json!({ "device": current.path, "report": report }));
    }
    match report.verified {
        Some(true) => println!(
            "✔ {} apagado e verificado: só restou o padrão gravado.",
            current.path
        ),
        Some(false) => println!(
            "✘ A verificação encontrou {} bytes diferentes do esperado: partes do disco não aceitaram a gravação (defeito?).",
            report.mismatched_bytes
        ),
        None => println!("✔ {} apagado ({} passada(s)).", current.path, report.passes),
    }
    println!(
        "Formate antes de usar: usbkeeper format {} --fs exfat",
        current.name
    );
    Ok(())
}

pub fn summary(r: &WipeReport) -> String {
    format!(
        "apagado com {} em {} passada(s){}",
        match r.pattern {
            WipePattern::Zeros => "zeros",
            WipePattern::Random => "dados aleatórios",
        },
        r.passes,
        match r.verified {
            Some(true) => ", verificado",
            Some(false) => ", VERIFICAÇÃO FALHOU",
            None => "",
        }
    )
}

#[cfg(target_os = "linux")]
fn discard_device(path: &str) -> Result<()> {
    Ok(usbkeeper_linux::discard(path)?)
}

#[cfg(not(target_os = "linux"))]
fn discard_device(_path: &str) -> Result<()> {
    bail!("descarte ainda não suportado nesta plataforma")
}
