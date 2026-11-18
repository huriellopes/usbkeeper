//! Fluxos que alteram discos: `format` e `eject`.
//!
//! Ordem do `format`: avaliar → planejar (ferramentas presentes?) → mostrar
//! → confirmar pela série → desmontar → reler e reconferir identidade e
//! regras → executar passo a passo.

use std::io::{BufRead, IsTerminal, Write};

use anyhow::{Context, Result, bail};
use usbkeeper_core::format::{
    Filesystem, FormatRequest, PartitionTable, normalize_label, resolve_table,
};
use usbkeeper_core::safety::{self, Policy, Refusal};
use usbkeeper_core::units::human_bytes;
use usbkeeper_core::{CoreError, Device};

use crate::backend::Backend;

/// Opções de segurança comuns a toda operação que grava no disco.
pub struct Guard {
    pub unmount: bool,
    pub allow_non_usb: bool,
    pub yes_i_know: Option<String>,
}

impl Guard {
    fn policy(&self) -> Policy {
        Policy {
            allow_non_usb: self.allow_non_usb,
        }
    }
}

pub struct FormatArgs {
    pub device: String,
    pub filesystem: Filesystem,
    pub table: Option<PartitionTable>,
    pub label: Option<String>,
    pub max_bytes: Option<u64>,
    pub guard: Guard,
    pub dry_run: bool,
    pub json: bool,
}

/// Recusa cedo (antes de mostrar plano ou pedir confirmação). Disco montado
/// só passa se `--unmount` foi pedido.
pub fn precheck(device: &Device, guard: &Guard) -> Result<()> {
    let refusals: Vec<Refusal> = safety::check_target(device, guard.policy())
        .into_iter()
        .filter(|r| !(guard.unmount && matches!(r, Refusal::Mounted { .. })))
        .collect();
    if refusals.is_empty() {
        return Ok(());
    }
    let mut msg = refuse(device, &refusals).to_string();
    if refusals
        .iter()
        .any(|r| matches!(r, Refusal::Mounted { .. }))
    {
        msg.push_str("\n(use --unmount para desmontar automaticamente)");
    }
    bail!(msg)
}

/// Permissão → confirmação pela série → desmontagem → releitura do disco,
/// conferindo identidade e regras de novo. Devolve o estado atual do disco.
pub fn confirm_and_take(b: &Backend, device: &Device, guard: &Guard) -> Result<Device> {
    ensure_writable(device)?;
    confirm(device, guard.yes_i_know.as_deref())?;
    let identity = device.identity();

    if device.is_mounted() {
        eprintln!("Desmontando...");
        b.mounts.unmount_all(device).map_err(busy_error)?;
    }

    // O pendrive pode ter sido trocado enquanto o usuário digitava.
    let current = b.enumerator.get(&device.path)?;
    if let Err(m) = safety::verify_same(&identity, &current) {
        bail!(
            "o dispositivo em {} mudou desde a confirmação (esperado série {:?} e {} bytes, encontrado {:?} e {} bytes); nada foi alterado",
            device.path,
            m.expected.serial,
            m.expected.size_bytes,
            m.found.serial,
            m.found.size_bytes
        );
    }
    let refusals = safety::check_target(&current, guard.policy());
    if !refusals.is_empty() {
        return Err(refuse(&current, &refusals));
    }
    Ok(current)
}

fn refuse(device: &Device, refusals: &[Refusal]) -> anyhow::Error {
    let reasons: Vec<String> = refusals
        .iter()
        .map(|r| format!("  - {}", r.message()))
        .collect();
    anyhow::anyhow!(
        "{} não pode ser usado nesta operação:\n{}",
        device.path,
        reasons.join("\n")
    )
}

/// Falha cedo se não houver permissão de escrita no dispositivo, em vez de
/// descobrir isso no meio do plano.
#[cfg(unix)]
fn ensure_writable(device: &Device) -> Result<()> {
    match std::fs::OpenOptions::new().write(true).open(&device.path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => bail!(
            "sem permissão para gravar em {}. Rode com sudo:\n  sudo {} {}",
            device.path,
            std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "usbkeeper".into()),
            std::env::args().skip(1).collect::<Vec<_>>().join(" ")
        ),
        Err(e) => Err(e).with_context(|| format!("abrindo {}", device.path)),
    }
}

#[cfg(not(unix))]
fn ensure_writable(_device: &Device) -> Result<()> {
    Ok(())
}

fn confirm(device: &Device, given: Option<&str>) -> Result<()> {
    let token = safety::confirmation_token(device);
    let typed = match given {
        Some(t) => t.to_string(),
        None => {
            if !std::io::stdin().is_terminal() {
                bail!("sem terminal para confirmar; use --yes-i-know {token}");
            }
            eprint!("\nPara confirmar, digite {token}: ");
            std::io::stderr().flush()?;
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line)?;
            line
        }
    };
    if safety::confirm(device, &typed) {
        Ok(())
    } else {
        bail!("confirmação não confere; nada foi alterado")
    }
}

fn print_plan(device: &Device, request: &FormatRequest) {
    println!("Alvo:     {}  {}", device.path, device.display_name());
    println!(
        "Tamanho:  {}   Série: {}",
        human_bytes(device.size_bytes),
        device.serial.as_deref().unwrap_or("-")
    );
    if !device.partitions.is_empty() {
        println!("SERÁ APAGADO:");
        for p in &device.partitions {
            println!(
                "  {} {} {} {}",
                p.path,
                human_bytes(p.size_bytes),
                p.fstype.as_deref().unwrap_or("-"),
                p.label.as_deref().unwrap_or("")
            );
        }
    }
    println!(
        "Novo:     {} em tabela {}{} — {}",
        request.filesystem,
        request.table,
        request
            .label
            .as_deref()
            .map(|l| format!(", rótulo \"{l}\""))
            .unwrap_or_default(),
        request.filesystem.hint()
    );
    if let Some(max) = request.max_bytes {
        println!(
            "Limite:   partição termina antes de {} (o resto do disco fica sem uso)",
            human_bytes(max)
        );
    }
}

pub fn format(b: &Backend, args: FormatArgs) -> Result<()> {
    let device = b.enumerator.get(&args.device)?;
    let label = normalize_label(args.filesystem, args.label.as_deref().unwrap_or(""))
        .map_err(anyhow::Error::msg)?;
    let table =
        resolve_table(args.table, device.size_bytes, args.max_bytes).map_err(anyhow::Error::msg)?;
    let request = FormatRequest {
        filesystem: args.filesystem,
        table,
        label,
        max_bytes: args.max_bytes,
    };

    precheck(&device, &args.guard)?;

    let steps = b.formatter.plan(&device, &request)?;
    if !args.json {
        print_plan(&device, &request);
    }

    if args.dry_run {
        println!("\nComandos que seriam executados:");
        for s in &steps {
            println!("  {}", s.command_line());
        }
        println!("\nSimulação (--dry-run): nada foi alterado.");
        return Ok(());
    }

    let current = confirm_and_take(b, &device, &args.guard)?;

    let steps = b.formatter.plan(&current, &request)?;
    let total = steps.len();
    for (i, step) in steps.iter().enumerate() {
        if crate::machine::get().progress_json {
            eprintln!(
                "{}",
                crate::machine::step_line(&step.description, i as u64 + 1, total as u64)
            );
        } else if !args.json {
            println!("[{}/{total}] {}", i + 1, step.description);
        }
        b.formatter
            .execute(std::slice::from_ref(step))
            .with_context(|| format!("falhou no passo {}/{total}; o disco pode estar sem partições, rode o format de novo", i + 1))?;
    }
    if args.json {
        crate::print_json(&serde_json::json!({
            "device": device.path,
            "filesystem": request.filesystem,
            "table": request.table,
            "label": request.label,
            "max_bytes": request.max_bytes,
        }))?;
    } else {
        println!(
            "\n✔ {} formatado em {}{}.",
            device.path,
            request.filesystem,
            request
                .label
                .as_deref()
                .map(|l| format!(" (\"{l}\")"))
                .unwrap_or_default()
        );
    }
    Ok(())
}

fn busy_error(e: CoreError) -> anyhow::Error {
    if let CoreError::Busy {
        mountpoint,
        holders,
    } = &e
        && !holders.is_empty()
    {
        // Os processos vão na própria mensagem: no modo --json o stdout é
        // só do resultado, e a interface gráfica mostra o texto do erro.
        let list = holders
            .iter()
            .map(|h| format!("  PID {} {} ({})", h.pid, h.command, h.path))
            .collect::<Vec<_>>()
            .join("\n");
        return anyhow::anyhow!(
            "{mountpoint} está em uso por:\n{list}\nfeche esses programas e tente de novo"
        );
    }
    e.into()
}

pub fn eject(
    b: &Backend,
    device: &str,
    allow_non_usb: bool,
    power_off: bool,
    json: bool,
) -> Result<()> {
    let device = b.enumerator.get(device)?;
    let refusals = safety::check_target(&device, Policy { allow_non_usb });
    if let Some(r) = refusals
        .iter()
        .find(|r| r.is_absolute() || matches!(r, Refusal::NotUsb { .. }))
    {
        bail!("não vou ejetar {}: {}", device.path, r.message());
    }

    if device.is_mounted() {
        eprintln!(
            "Desmontando {}...",
            device.all_mountpoints().collect::<Vec<_>>().join(", ")
        );
        b.mounts.unmount_all(&device).map_err(busy_error)?;
    }
    if power_off {
        b.ejector.power_off(&device)?;
    }
    if json {
        crate::print_json(&serde_json::json!({
            "device": device.path,
            "powered_off": power_off,
        }))?;
    } else if power_off {
        println!("✔ {} desligado. Pode remover com segurança.", device.path);
    } else {
        println!("✔ {} desmontado.", device.path);
    }
    Ok(())
}
