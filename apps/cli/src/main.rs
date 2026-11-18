use anyhow::Result;
use clap::{Parser, Subcommand};
use usbkeeper_core::safety::{self, Policy};
use usbkeeper_core::units::human_bytes;
use usbkeeper_core::{Device, Enumerator, Transport};

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
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("erro: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "linux")]
fn enumerator(include_virtual: bool) -> impl Enumerator {
    usbkeeper_linux::LinuxEnumerator {
        include_virtual,
        ..Default::default()
    }
}

#[cfg(not(target_os = "linux"))]
fn enumerator(_include_virtual: bool) -> impl Enumerator {
    struct Unsupported;
    impl Enumerator for Unsupported {
        fn list(&self) -> Result<Vec<Device>, usbkeeper_core::CoreError> {
            Err(usbkeeper_core::CoreError::Backend(
                "plataforma ainda não suportada (Windows e macOS chegam nas próximas etapas)"
                    .into(),
            ))
        }
    }
    Unsupported
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::List { all, json } => {
            let devices = enumerator(all).list()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&devices)?);
            } else {
                print_table(&devices);
                wsl_hint(&devices);
            }
        }
        Command::Info {
            device,
            json,
            allow_non_usb,
        } => {
            let dev = enumerator(true).get(&device)?;
            let policy = Policy { allow_non_usb };
            if json {
                let refusals = safety::check_target(&dev, policy);
                let out = serde_json::json!({ "device": dev, "refusals": refusals });
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                print_info(&dev, policy);
            }
        }
    }
    Ok(())
}

fn status(dev: &Device) -> &'static str {
    let refusals = safety::check_target(dev, Policy::default());
    match refusals.first() {
        None => "pode ser alvo",
        Some(r) if r.is_absolute() => "SISTEMA",
        Some(safety::Refusal::NotUsb { .. }) => "não-USB",
        Some(safety::Refusal::Mounted { .. }) => "montado",
        Some(safety::Refusal::ReadOnly) => "somente leitura",
        Some(safety::Refusal::Empty) => "sem mídia",
        Some(_) => "bloqueado",
    }
}

fn print_table(devices: &[Device]) {
    if devices.is_empty() {
        println!("Nenhum disco encontrado.");
        return;
    }
    let rows: Vec<[String; 6]> = devices
        .iter()
        .map(|d| {
            let usb = d
                .usb
                .as_ref()
                .map(|u| {
                    let mut s = u.speed.label().to_string();
                    if u.is_bottlenecked() {
                        s.push_str(" ⚠");
                    }
                    s
                })
                .unwrap_or_else(|| "-".into());
            [
                d.path.clone(),
                human_bytes(d.size_bytes),
                d.transport.label().to_string(),
                d.display_name(),
                usb,
                status(d).to_string(),
            ]
        })
        .collect();
    let header = [
        "DISPOSITIVO",
        "TAMANHO",
        "BARRAMENTO",
        "MODELO",
        "USB",
        "STATUS",
    ];
    let widths: Vec<usize> = (0..header.len())
        .map(|i| {
            rows.iter()
                .map(|r| r[i].chars().count())
                .chain([header[i].len()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: &[String]| {
        cells
            .iter()
            .zip(&widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    println!("{}", line(&header.map(String::from)));
    for r in &rows {
        println!("{}", line(r));
    }
    if devices
        .iter()
        .any(|d| d.usb.as_ref().is_some_and(|u| u.is_bottlenecked()))
    {
        println!(
            "\n⚠ dispositivo USB 3 operando em velocidade USB 2: troque de porta, hub ou cabo."
        );
    }
}

fn print_info(dev: &Device, policy: Policy) {
    let opt = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
    println!("{}  ({})", dev.path, dev.display_name());
    let yes_no = |b: bool| if b { "sim" } else { "não" }.to_string();
    for (label, value) in [
        (
            "Tamanho",
            format!("{} ({} bytes)", human_bytes(dev.size_bytes), dev.size_bytes),
        ),
        ("Barramento", dev.transport.label().to_string()),
        ("Nº de série", opt(&dev.serial)),
        ("Removível", yes_no(dev.removable)),
        ("Somente leitura", yes_no(dev.read_only)),
        ("Tabela", opt(&dev.partition_table)),
        (
            "Setores",
            format!(
                "lógico {} B, físico {} B",
                dev.logical_sector, dev.physical_sector
            ),
        ),
    ] {
        println!("  {:<17}{value}", format!("{label}:"));
    }

    if let Some(u) = &dev.usb {
        println!("  USB:");
        println!("    ID:           {}:{}", u.vendor_id, u.product_id);
        println!("    Fabricante:   {}", opt(&u.manufacturer));
        println!("    Produto:      {}", opt(&u.product));
        println!("    Declara:      USB {}", opt(&u.usb_version));
        println!("    Negociado:    {}", u.speed.label());
        if u.is_bottlenecked() {
            println!("    ⚠ Dispositivo USB 3 rodando em USB 2: troque de porta, hub ou cabo.");
        }
    }

    if !dev.mountpoints.is_empty() {
        println!("  Montado em:     {}", dev.mountpoints.join(", "));
    }
    if !dev.partitions.is_empty() {
        println!("  Partições:");
        for p in &dev.partitions {
            println!(
                "    {:<16} {:>9}  {:<12} {:<14} {}",
                p.path,
                human_bytes(p.size_bytes),
                opt(&p.fstype),
                opt(&p.label),
                p.mountpoints.join(", ")
            );
        }
    }

    let refusals = safety::check_target(dev, policy);
    if refusals.is_empty() {
        println!("\n✔ Pode ser alvo de operações destrutivas.");
        println!(
            "  Confirmação exigirá digitar: {}",
            safety::confirmation_token(dev)
        );
    } else {
        println!("\n✘ Não pode ser alvo agora:");
        for r in &refusals {
            println!("  - {}", r.message());
        }
    }
}

#[cfg(target_os = "linux")]
fn wsl_hint(devices: &[Device]) {
    if usbkeeper_linux::wsl::running_in_wsl()
        && !devices.iter().any(|d| d.transport == Transport::Usb)
    {
        println!("\n{}", usbkeeper_linux::wsl::USBIPD_HINT);
    }
}

#[cfg(not(target_os = "linux"))]
fn wsl_hint(_devices: &[Device]) {}
