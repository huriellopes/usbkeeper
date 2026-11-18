mod backend;
mod checks;
mod destructive;
mod view;

use anyhow::Result;
use clap::{Parser, Subcommand};
use usbkeeper_core::format::{Filesystem, PartitionTable};
use usbkeeper_core::safety::{self, Policy};

use crate::backend::backend;
use crate::checks::{BenchArgs, FakeCheckArgs};
use crate::destructive::{FormatArgs, Guard};

#[derive(Parser)]
#[command(
    name = "usbkeeper",
    version,
    about = "Gerencia pendrives e HDs externos"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lista os discos e se podem ser alvo de operações destrutivas
    List {
        /// Inclui dispositivos virtuais (loop, zram)
        #[arg(long)]
        all: bool,
        /// Saída em JSON
        #[arg(long)]
        json: bool,
    },
    /// Detalhes de um disco (ex.: sdb ou /dev/sdb)
    Info {
        device: String,
        #[arg(long)]
        json: bool,
        /// Considera discos não-USB como alvos possíveis na avaliação
        #[arg(long)]
        allow_non_usb: bool,
    },
    /// Apaga o disco e cria uma partição única formatada
    Format {
        device: String,
        /// fat32 | exfat | ntfs | ext4
        #[arg(long = "fs", short = 'f')]
        filesystem: Filesystem,
        /// gpt | mbr (padrão: mbr até 2 TiB, gpt acima)
        #[arg(long)]
        table: Option<PartitionTable>,
        /// Rótulo do volume
        #[arg(long, short = 'l')]
        label: Option<String>,
        /// Usa só o início do disco (ex.: 7800MiB): para pendrive falsificado,
        /// com o valor sugerido pelo fake-check. Força tabela MBR.
        #[arg(long, value_name = "TAMANHO", value_parser = usbkeeper_core::units::parse_size)]
        max_size: Option<u64>,
        /// Desmonta as partições antes, se estiverem montadas
        #[arg(long)]
        unmount: bool,
        /// Permite alvo que não seja USB (o disco do sistema continua proibido)
        #[arg(long)]
        allow_non_usb: bool,
        /// Confirma sem perguntar; precisa ser o número de série (ou o caminho)
        #[arg(long, value_name = "SERIE")]
        yes_i_know: Option<String>,
        /// Mostra o que seria feito, sem alterar nada
        #[arg(long)]
        dry_run: bool,
    },
    /// Verifica se o pendrive/cartão é falsificado (capacidade adulterada)
    #[command(name = "fake-check")]
    FakeCheck {
        device: String,
        /// Grava e lê o disco inteiro (definitivo, APAGA TUDO, demorado)
        #[arg(long)]
        full: bool,
        /// Teste rápido sem salvar/restaurar os blocos originais
        #[arg(long, conflicts_with = "full")]
        no_backup: bool,
        #[command(flatten)]
        guard: GuardArgs,
        /// Saída em JSON (sem barra de progresso)
        #[arg(long)]
        json: bool,
    },
    /// Mede velocidade: leitura sequencial e 4K (e escrita, com --write)
    Bench {
        device: String,
        /// Também mede escrita (destrutivo: sobrescreve parte do disco)
        #[arg(long)]
        write: bool,
        /// MB lidos/gravados no teste sequencial
        #[arg(long, default_value_t = 256)]
        sample_mb: u64,
        #[command(flatten)]
        guard: GuardArgs,
        /// Saída em JSON (sem barra de progresso)
        #[arg(long)]
        json: bool,
    },
    /// Desmonta e desliga o dispositivo para remoção segura
    Eject {
        device: String,
        /// Só desmonta, sem desligar
        #[arg(long)]
        no_power_off: bool,
        #[arg(long)]
        allow_non_usb: bool,
    },
}

#[derive(clap::Args)]
struct GuardArgs {
    /// Desmonta as partições antes, se estiverem montadas
    #[arg(long)]
    unmount: bool,
    /// Permite alvo que não seja USB (o disco do sistema continua proibido)
    #[arg(long)]
    allow_non_usb: bool,
    /// Confirma sem perguntar; precisa ser o número de série (ou o caminho)
    #[arg(long, value_name = "SERIE")]
    yes_i_know: Option<String>,
}

impl From<GuardArgs> for Guard {
    fn from(g: GuardArgs) -> Self {
        Guard {
            unmount: g.unmount,
            allow_non_usb: g.allow_non_usb,
            yes_i_know: g.yes_i_know,
        }
    }
}

/// Escreve JSON no stdout; se o leitor fechar o pipe (ex.: `| head`), sai
/// em silêncio em vez de entrar em pânico.
pub fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    use std::io::Write;
    let text = serde_json::to_string_pretty(value)?;
    match writeln!(std::io::stdout().lock(), "{text}") {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => Ok(other?),
    }
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("erro: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::List { all, json } => {
            let devices = backend(all).enumerator.list()?;
            if json {
                print_json(&devices)?;
            } else {
                view::print_table(&devices);
                view::wsl_hint(&devices);
            }
        }
        Command::Info {
            device,
            json,
            allow_non_usb,
        } => {
            let b = backend(true);
            let dev = b.enumerator.get(&device)?;
            let policy = Policy { allow_non_usb };
            let holders = b.mounts.holders(&dev);
            if json {
                let refusals = safety::check_target(&dev, policy);
                let out = serde_json::json!({
                    "device": dev,
                    "refusals": refusals,
                    "holders": holders,
                });
                print_json(&out)?;
            } else {
                view::print_info(&dev, policy, &holders);
            }
        }
        Command::Format {
            device,
            filesystem,
            table,
            label,
            max_size,
            unmount,
            allow_non_usb,
            yes_i_know,
            dry_run,
        } => destructive::format(
            &backend(true),
            FormatArgs {
                device,
                filesystem,
                table,
                label,
                max_bytes: max_size,
                guard: Guard {
                    unmount,
                    allow_non_usb,
                    yes_i_know,
                },
                dry_run,
            },
        )?,
        Command::FakeCheck {
            device,
            full,
            no_backup,
            guard,
            json,
        } => checks::fake_check(
            &backend(true),
            FakeCheckArgs {
                device,
                full,
                no_backup,
                guard: guard.into(),
                json,
            },
        )?,
        Command::Bench {
            device,
            write,
            sample_mb,
            guard,
            json,
        } => checks::bench(
            &backend(true),
            BenchArgs {
                device,
                write,
                sample_mb,
                guard: guard.into(),
                json,
            },
        )?,
        Command::Eject {
            device,
            no_power_off,
            allow_non_usb,
        } => destructive::eject(&backend(true), &device, allow_non_usb, !no_power_off)?,
    }
    Ok(())
}
