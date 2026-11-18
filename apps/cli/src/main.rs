mod backend;
mod destructive;
mod view;

use anyhow::Result;
use clap::{Parser, Subcommand};
use usbkeeper_core::format::{Filesystem, PartitionTable};
use usbkeeper_core::safety::{self, Policy};

use crate::backend::backend;
use crate::destructive::FormatArgs;

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
                println!("{}", serde_json::to_string_pretty(&devices)?);
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
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                view::print_info(&dev, policy, &holders);
            }
        }
        Command::Format {
            device,
            filesystem,
            table,
            label,
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
                unmount,
                allow_non_usb,
                yes_i_know,
                dry_run,
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
