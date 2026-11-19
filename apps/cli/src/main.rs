mod backend;
mod checks;
mod destructive;
mod health;
mod machine;
mod report;
mod store;
mod view;

use anyhow::{Context, Result};
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
    /// Não grava este comando no histórico
    #[arg(long, global = true)]
    no_history: bool,
    /// Progresso como linhas JSON no stderr (interface gráfica)
    #[arg(long, global = true, hide = true)]
    progress_json: bool,
    /// Cancela ao ler a linha "cancel" no stdin (interface gráfica)
    #[arg(long, global = true, hide = true)]
    cancel_stdin: bool,
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
        /// Resultado em JSON
        #[arg(long)]
        json: bool,
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
    /// Saúde: SMART (HD/SSD) e, com --scan, leitura de toda a superfície
    Health {
        device: String,
        /// Lê o disco inteiro procurando blocos ilegíveis (não grava nada)
        #[arg(long)]
        scan: bool,
        /// Saída em JSON (sem barra de progresso)
        #[arg(long)]
        json: bool,
    },
    /// Histórico de testes: todos os dispositivos, ou um (nome, chave ou série)
    History {
        #[arg(allow_hyphen_values = true)]
        device: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Gera laudo HTML do dispositivo a partir do histórico
    Report {
        #[arg(allow_hyphen_values = true)]
        device: String,
        /// Arquivo de saída (padrão: usbkeeper-<chave>.html)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,
    },
    /// Guia: pendrive, HD externo, MBR × GPT, sistemas de arquivos, falsificados… (ex.: usbkeeper guia mbr)
    Guia {
        /// Tópico (id ou apelido). Sem tópico, lista todos.
        topic: Option<String>,
        /// Imprime o HTML do tópico
        #[arg(long)]
        html: bool,
    },
    /// Remove com segurança: desmonta e desliga (apelido: remover)
    #[command(alias = "remover")]
    Eject {
        device: String,
        /// Só desmonta, sem desligar
        #[arg(long)]
        no_power_off: bool,
        #[arg(long)]
        allow_non_usb: bool,
        /// Resultado em JSON
        #[arg(long)]
        json: bool,
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
    let cli = Cli::parse();
    machine::set(machine::Mode {
        progress_json: cli.progress_json,
        cancel_stdin: cli.cancel_stdin,
    });
    if let Err(e) = run(cli) {
        if machine::get().progress_json {
            eprintln!("{}", machine::error_value(&e));
        } else {
            eprintln!("erro: {e:#}");
        }
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    if cli.no_history {
        store::disable();
    }
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
            json,
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
                json,
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
        Command::Health { device, scan, json } => {
            health::health(&backend(true), health::HealthArgs { device, scan, json })?
        }
        Command::History { device, json } => report::history(&backend(true), device, json)?,
        Command::Report { device, output } => report::report(&backend(true), device, output)?,
        Command::Guia { topic, html } => match topic {
            None => {
                println!("Tópicos do guia (usbkeeper guia <tópico>):\n");
                for t in usbkeeper_guide::topics() {
                    println!("  {:<22} {}\n  {:<22} {}\n", t.id, t.title, "", t.summary);
                }
            }
            Some(q) => {
                let t = usbkeeper_guide::find(&q).with_context(|| {
                    format!("tópico desconhecido: {q:?}; rode `usbkeeper guia` para ver a lista")
                })?;
                if html {
                    println!("{}", usbkeeper_guide::to_html(t.markdown));
                } else {
                    print!("{}", usbkeeper_guide::to_terminal(t.markdown));
                }
            }
        },
        Command::Eject {
            device,
            no_power_off,
            allow_non_usb,
            json,
        } => destructive::eject(&backend(true), &device, allow_non_usb, !no_power_off, json)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_and_report_accept_keys_starting_with_hyphen() {
        // Chaves gravadas antes desta correção começavam com "-:-:".
        let cli = Cli::try_parse_from(["usbkeeper", "history", "-:-:WD-1"]).unwrap();
        assert!(matches!(cli.command, Command::History { device: Some(d), .. } if d == "-:-:WD-1"));
        let cli = Cli::try_parse_from(["usbkeeper", "report", "-:-:x", "-o", "a.html"]).unwrap();
        assert!(matches!(cli.command, Command::Report { device, .. } if device == "-:-:x"));
    }

    #[test]
    fn machine_flags_are_global() {
        let cli = Cli::try_parse_from([
            "usbkeeper",
            "fake-check",
            "sdb",
            "--json",
            "--progress-json",
            "--cancel-stdin",
        ])
        .unwrap();
        assert!(cli.progress_json && cli.cancel_stdin);
    }

    #[test]
    fn guia_and_remover_parse() {
        let cli = Cli::try_parse_from(["usbkeeper", "guia", "mbr"]).unwrap();
        assert!(matches!(cli.command, Command::Guia { topic: Some(t), html: false } if t == "mbr"));
        let cli = Cli::try_parse_from(["usbkeeper", "remover", "sdb"]).unwrap();
        assert!(matches!(cli.command, Command::Eject { device, .. } if device == "sdb"));
    }

    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
