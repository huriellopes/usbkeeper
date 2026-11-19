//! `selftest`/`autoteste`: o próprio disco se testa no firmware (não
//! altera dados). Com --esperar, acompanha até o fim; cancelar aborta o
//! teste no disco.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use usbkeeper_core::blockdev::{Phase, Progress};
use usbkeeper_history::kind;
use usbkeeper_smart::selftest::{self, SelfTestEntry, SelfTestKind, SelfTestState};

use crate::backend::Backend;
use crate::checks::cancel_flag;
use crate::store;

const POLL: Duration = Duration::from_secs(10);
/// Depois de o disco dizer que terminou, quanto esperar a entrada nova no log.
const LOG_GRACE_POLLS: u32 = 6;

fn name(kind: SelfTestKind) -> &'static str {
    match kind {
        SelfTestKind::Short => "curto",
        SelfTestKind::Long => "longo",
    }
}

pub fn selftest(
    b: &Backend,
    device: &str,
    kind: SelfTestKind,
    wait: bool,
    json: bool,
) -> Result<()> {
    let dev = b.enumerator.get(device)?;
    let smart = usbkeeper_smart::read(&dev.path);
    if !smart.has_smart() {
        bail!(
            "{} não tem SMART legível, e o autoteste depende dele: {}",
            dev.path,
            smart.notes.join("; ")
        );
    }
    let dtype = smart.device_type.clone().unwrap_or_else(|| "auto".into());
    let before = selftest::poll(&dev.path, &dtype, kind).map_err(|e| anyhow!(e))?;
    if before.running {
        bail!("já há um autoteste rodando neste disco; espere terminar ou aborte-o");
    }
    let previous = before.log.first().cloned();
    selftest::start(&dev.path, &dtype, kind).map_err(|e| anyhow!(e))?;
    let minutes = before.polling_minutes;

    if !wait {
        if json {
            return crate::print_json(&serde_json::json!({
                "device": dev.path, "started": true, "kind": name(kind), "estimated_minutes": minutes,
            }));
        }
        println!(
            "Autoteste {} iniciado no próprio disco{}.\nNão altera dados. Veja o resultado depois com: usbkeeper health {}",
            name(kind),
            minutes
                .map(|m| format!(" (estimado: ~{m} min)"))
                .unwrap_or_default(),
            dev.name
        );
        return Ok(());
    }

    let cancel = cancel_flag();
    let mut reporter = crate::machine::Reporter::new(json);
    let mut grace = 0;
    let state = loop {
        for _ in 0..POLL.as_secs() {
            std::thread::sleep(Duration::from_secs(1));
            if usbkeeper_core::blockdev::cancelled(&cancel) {
                reporter.finish();
                let _ = selftest::abort(&dev.path, &dtype);
                bail!("cancelado: o autoteste foi abortado no disco");
            }
        }
        let state: SelfTestState =
            selftest::poll(&dev.path, &dtype, kind).map_err(|e| anyhow!(e))?;
        if state.running {
            let done = 100 - u64::from(state.remaining_percent.unwrap_or(100).min(100));
            reporter.report(Phase::SelfTest, done, 100);
            continue;
        }
        // Terminou: espera o log ganhar a entrada nova (a anterior é de outro teste).
        if state.log.first() != previous.as_ref() || grace >= LOG_GRACE_POLLS {
            break state;
        }
        grace += 1;
    };
    reporter.report(Phase::SelfTest, 100, 100);
    reporter.finish();

    let result: Option<&SelfTestEntry> =
        state.log.first().filter(|e| Some(*e) != previous.as_ref());
    let verdict = match result.and_then(|e| e.passed) {
        Some(true) => "passed",
        Some(false) => "failed",
        None => "unknown",
    };
    let summary = match result {
        Some(e) => format!("autoteste {}: {}", name(kind), e.status),
        None => format!(
            "autoteste {}: resultado não encontrado no log do disco",
            name(kind)
        ),
    };
    store::record(
        &dev,
        kind::SELFTEST,
        Some(verdict),
        &summary,
        serde_json::json!(state),
    );
    if json {
        return crate::print_json(&serde_json::json!({
            "device": dev.path, "kind": name(kind), "verdict": verdict, "result": result, "state": state,
        }));
    }
    match verdict {
        "passed" => println!(
            "✔ Autoteste {} aprovado: {}",
            name(kind),
            result.map(|e| e.status.as_str()).unwrap_or("")
        ),
        "failed" => println!(
            "✘ Autoteste {} REPROVADO: {}\n  Copie seus dados e substitua o disco. Entenda: usbkeeper guia saude",
            name(kind),
            result.map(|e| e.status.as_str()).unwrap_or("")
        ),
        _ => println!("– O autoteste terminou, mas o resultado não apareceu no log do disco."),
    }
    Ok(())
}
