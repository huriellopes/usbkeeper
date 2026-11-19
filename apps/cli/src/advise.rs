//! `advise`/`recomendar`: qual formatação usar para o uso pretendido.

use anyhow::Result;
use usbkeeper_core::advice::{Advice, BigFiles, DiskFacts, Usage, advise as rules};
use usbkeeper_core::format::PartitionTable;

use crate::backend::Backend;
use crate::store;

pub fn advise(b: &Backend, device: &str, usage: Usage, big: BigFiles, json: bool) -> Result<()> {
    let dev = b.enumerator.get(device)?;
    let (fake, health) = store::last_facts(&dev);
    let advice = rules(usage, big, &DiskFacts::of(&dev, fake, health));
    if json {
        return crate::print_json(&serde_json::json!({ "device": dev.path, "advice": advice }));
    }
    print_advice(&dev.name, &advice);
    Ok(())
}

/// Comando `format` equivalente à recomendação.
pub fn command_line(device: &str, a: &Advice) -> String {
    let mut cmd = format!("usbkeeper format {device} --fs {}", a.filesystem);
    if let Some(max) = a.max_bytes {
        cmd.push_str(&format!(" --max-size {}MiB", max >> 20));
    } else if a.table == PartitionTable::Gpt {
        cmd.push_str(" --table gpt");
    }
    cmd
}

fn print_advice(device: &str, a: &Advice) {
    println!(
        "Recomendado: {} em tabela {}{}",
        a.filesystem,
        a.table,
        a.max_bytes
            .map(|m| format!(", usando só os primeiros {} MiB", m >> 20))
            .unwrap_or_default()
    );
    println!("\nPor quê:");
    for r in &a.reasons {
        println!("  • {r}");
    }
    if !a.warnings.is_empty() {
        println!("\nAtenção:");
        for w in &a.warnings {
            println!("  ⚠ {w}");
        }
    }
    println!(
        "\nPara formatar assim:\n  {}\n(entenda: usbkeeper guia sistemas-de-arquivos)",
        command_line(device, a)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use usbkeeper_core::format::Filesystem;

    fn advice(table: PartitionTable, max: Option<u64>) -> Advice {
        Advice {
            filesystem: Filesystem::Exfat,
            table,
            max_bytes: max,
            reasons: vec![],
            warnings: vec![],
        }
    }

    #[test]
    fn command_line_matches_advice() {
        assert_eq!(
            command_line("sdb", &advice(PartitionTable::Mbr, None)),
            "usbkeeper format sdb --fs exfat"
        );
        assert_eq!(
            command_line("sdb", &advice(PartitionTable::Gpt, None)),
            "usbkeeper format sdb --fs exfat --table gpt"
        );
        assert_eq!(
            command_line("sdb", &advice(PartitionTable::Mbr, Some(8 << 30))),
            "usbkeeper format sdb --fs exfat --max-size 8192MiB"
        );
    }
}
