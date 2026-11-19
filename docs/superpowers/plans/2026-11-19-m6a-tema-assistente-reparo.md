# M6a — Tema, assistente de formatação e verificar/reparar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Seletor de tema, assistente que recomenda a formatação e verificação/reparo de sistemas de arquivos, na CLI e na janela.

**Architecture:** Regras do assistente e interpretação dos resultados de reparo ficam no core (puro, testado); os comandos de reparo no backend Linux, com execução sempre seguida de nova verificação. A CLI ganha `advise`/`recomendar` e `repair`/`reparar`; a GUI chama `advise` direto (sem root) e `repair` via pkexec como as outras operações.

**Tech Stack:** Rust 1.99, clap 4, Tauri 2, React 19 + TS, zustand, vitest; dosfstools, exfatprogs, ntfs-3g, e2fsprogs, util-linux (sfdisk).

Spec: `docs/superpowers/specs/2026-11-19-m6a-tema-assistente-reparo-design.md`.

## Global Constraints

- Versão alvo: **0.6.0**.
- Reparar exige as mesmas travas do formatar (disco do sistema nunca, só USB por padrão, confirmação pela série, reconferência). `--check` não pede série, mas recusa o disco do sistema e exige disco desmontado.
- Ferramentas rodam com `LC_ALL=C`; o log guarda as últimas 30 linhas por partição.
- Reparar termina **sempre** com uma nova verificação; o status final vem dela.
- Tema em `localStorage` chave `usbkeeper.theme` ∈ {`system`,`light`,`dark`}, acesso protegido com try/catch.
- Commits sem coautoria do Claude.

## File Structure

```
crates/core/src/advice.rs            (novo) Usage, BigFiles, DiskFacts, Advice, advise()
crates/core/src/repair.rs            (novo) Tool, RepairStatus, PartitionRepair, RepairReport, check_status(), repair_status()
crates/core/src/platform.rs          trait Repairer
crates/platform-linux/src/repair.rs  (novo) tool_for(), command(), LinuxRepairer
crates/history/src/lib.rs            kind::REPAIR
crates/guide/topics/reparar.md       (novo) + lib.rs (13 tópicos, apelidos fsck/repair)
apps/cli/src/main.rs, destructive.rs (take()), advise.rs (novo), repair.rs (novo), backend.rs
apps/gui/src-tauri/src/{devices.rs (advise), cli.rs (OpRequest::Repair)}
apps/gui/index.html, src/index.css, src/lib/theme.ts (+test), src/features/theme/ThemeSwitch.tsx
apps/gui/src/features/format/{FormatTab.tsx, AdviceCard.tsx (+test)}
apps/gui/src/features/repair/{RepairTab.tsx (+test)}, features/device/DevicePanel.tsx, lib/{api,format,guideLinks}.ts
scripts/e2e-loop.sh, README.md
```

---

### Task 1: Regras do assistente (`core::advice`)

**Files:** Create `crates/core/src/advice.rs`; Modify `crates/core/src/lib.rs` (`pub mod advice;`).

**Interfaces — Produces:**
- `pub enum Usage { Mixed, WindowsOnly, LinuxOnly, Appliance, Console }` (`Deserialize`, `Serialize`, snake_case, TS; `FromStr`: `misto|mixed`, `windows`, `linux`, `aparelho|tv`, `videogame|console`)
- `pub enum BigFiles { Yes, No, Unknown }` (`FromStr`: `sim|yes`, `nao|não|no`, `nao-sei|unknown`)
- `pub struct DiskFacts { size_bytes: u64, removable: bool, fake: Option<(u64, u64)> /* (real, sugerido) */, health: Option<String>, usb_bottleneck: bool }` + `DiskFacts::of(device: &Device, fake: Option<(u64, u64)>, health: Option<String>) -> DiskFacts`
- `pub struct Advice { filesystem: Filesystem, table: PartitionTable, max_bytes: Option<u64>, reasons: Vec<String>, warnings: Vec<String> }` (Serialize, TS)
- `pub fn advise(usage: Usage, big: BigFiles, facts: &DiskFacts) -> Advice`

- [ ] **Step 1: Testes que falham** (em `advice.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    const GB: u64 = 1_000_000_000;

    fn facts(size: u64, removable: bool) -> DiskFacts {
        DiskFacts { size_bytes: size, removable, fake: None, health: None, usb_bottleneck: false }
    }

    #[test]
    fn filesystem_by_usage() {
        let pen = facts(64 * GB, true);
        let hd = facts(1000 * GB, false);
        let fs = |u, f: &DiskFacts| advise(u, BigFiles::Unknown, f).filesystem;
        assert_eq!(fs(Usage::Appliance, &pen), Filesystem::Fat32);
        assert_eq!(fs(Usage::Mixed, &pen), Filesystem::Exfat);
        assert_eq!(fs(Usage::WindowsOnly, &hd), Filesystem::Ntfs);
        assert_eq!(fs(Usage::WindowsOnly, &pen), Filesystem::Exfat);
        assert_eq!(fs(Usage::LinuxOnly, &pen), Filesystem::Ext4);
        assert_eq!(fs(Usage::Console, &pen), Filesystem::Exfat);
    }

    #[test]
    fn appliance_with_big_files_warns() {
        let a = advise(Usage::Appliance, BigFiles::Yes, &facts(64 * GB, true));
        assert_eq!(a.filesystem, Filesystem::Fat32);
        assert!(a.warnings.iter().any(|w| w.contains("4 GB")));
    }

    #[test]
    fn table_by_size() {
        assert_eq!(advise(Usage::Mixed, BigFiles::No, &facts(64 * GB, true)).table, PartitionTable::Mbr);
        let big = advise(Usage::Mixed, BigFiles::No, &facts(3000 * GB, false));
        assert_eq!(big.table, PartitionTable::Gpt);
        let tv = advise(Usage::Appliance, BigFiles::No, &facts(3000 * GB, false));
        assert!(tv.warnings.iter().any(|w| w.contains("2 TB")));
    }

    #[test]
    fn fake_limits_and_forces_mbr() {
        let mut f = facts(4000 * GB, true);
        f.fake = Some((8 * GB, 7_800_000_000));
        let a = advise(Usage::Mixed, BigFiles::No, &f);
        assert_eq!(a.max_bytes, Some(7_800_000_000));
        assert_eq!(a.table, PartitionTable::Mbr);
        assert!(a.reasons.iter().any(|r| r.contains("falsificado")));
        assert!(a.warnings.iter().any(|w| w.contains("única cópia")));
    }

    #[test]
    fn health_and_usb_warnings() {
        let mut f = facts(64 * GB, true);
        f.health = Some("failing".into());
        f.usb_bottleneck = true;
        let a = advise(Usage::Mixed, BigFiles::No, &f);
        assert!(a.warnings.iter().any(|w| w.contains("falha iminente")));
        assert!(a.warnings.iter().any(|w| w.contains("USB 2.0")));
    }

    #[test]
    fn parses_cli_words() {
        assert_eq!("tv".parse::<Usage>(), Ok(Usage::Appliance));
        assert_eq!("MISTO".parse::<Usage>(), Ok(Usage::Mixed));
        assert_eq!("não".parse::<BigFiles>(), Ok(BigFiles::No));
        assert!("xpto".parse::<Usage>().is_err());
    }
}
```

- [ ] **Step 2: Ver falhar** — `cargo test -q -p usbkeeper-core advice` → não compila.

- [ ] **Step 3: Implementação**

```rust
//! Assistente de formatação: recomenda sistema de arquivos, tabela e limite
//! a partir do uso pretendido e do que já se sabe do disco.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::format::{Filesystem, PartitionTable};
use crate::model::Device;

const TWO_TIB: u64 = 2 * 1024 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Usage { Mixed, WindowsOnly, LinuxOnly, Appliance, Console }

impl FromStr for Usage {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "misto" | "mixed" => Ok(Self::Mixed),
            "windows" => Ok(Self::WindowsOnly),
            "linux" => Ok(Self::LinuxOnly),
            "aparelho" | "tv" => Ok(Self::Appliance),
            "videogame" | "console" => Ok(Self::Console),
            o => Err(format!("uso desconhecido: {o} (use misto, windows, linux, aparelho ou videogame)")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum BigFiles { Yes, No, Unknown }

impl FromStr for BigFiles {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "sim" | "yes" => Ok(Self::Yes),
            "nao" | "não" | "no" => Ok(Self::No),
            "nao-sei" | "não-sei" | "unknown" => Ok(Self::Unknown),
            o => Err(format!("resposta desconhecida: {o} (use sim, nao ou nao-sei)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskFacts {
    pub size_bytes: u64,
    pub removable: bool,
    /// Último fake-check deu falso: (capacidade real, limite sugerido).
    pub fake: Option<(u64, u64)>,
    /// Último veredito de saúde: "good" | "warning" | "failing" | "unknown".
    pub health: Option<String>,
    pub usb_bottleneck: bool,
}

impl DiskFacts {
    pub fn of(device: &Device, fake: Option<(u64, u64)>, health: Option<String>) -> Self {
        Self {
            size_bytes: device.size_bytes,
            removable: device.removable,
            fake,
            health,
            usb_bottleneck: device.usb.as_ref().is_some_and(|u| u.is_bottlenecked()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Advice {
    pub filesystem: Filesystem,
    pub table: PartitionTable,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub max_bytes: Option<u64>,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn advise(usage: Usage, big: BigFiles, facts: &DiskFacts) -> Advice {
    let mut reasons = Vec::new();
    let mut warnings = Vec::new();

    let filesystem = match usage {
        Usage::Appliance => {
            reasons.push("FAT32 é o formato que TVs, sons de carro e aparelhos antigos leem com mais segurança.".into());
            if big == BigFiles::Yes {
                warnings.push("Arquivos acima de 4 GB não cabem em FAT32; só alguns aparelhos novos leem exFAT. Teste no aparelho antes.".into());
            }
            Filesystem::Fat32
        }
        Usage::Mixed => {
            reasons.push("exFAT funciona em Windows, macOS e Linux e aceita arquivos acima de 4 GB.".into());
            Filesystem::Exfat
        }
        Usage::WindowsOnly if !facts.removable => {
            reasons.push("NTFS é o formato nativo do Windows e tem journaling, que protege melhor um HD/SSD externo contra quedas de energia.".into());
            Filesystem::Ntfs
        }
        Usage::WindowsOnly => {
            reasons.push("Num pendrive ou cartão, exFAT é mais leve que NTFS e gasta menos a memória flash; o Windows lê e grava normalmente.".into());
            Filesystem::Exfat
        }
        Usage::LinuxOnly => {
            reasons.push("ext4 é o formato nativo do Linux: guarda permissões e tem journaling. Windows e macOS não o leem sem programas extras.".into());
            Filesystem::Ext4
        }
        Usage::Console => {
            reasons.push("Os consoles atuais leem exFAT; alguns modelos antigos exigem FAT32.".into());
            Filesystem::Exfat
        }
    };

    let (table, max_bytes) = if let Some((real, suggested)) = facts.fake {
        reasons.push(format!(
            "O último teste mostrou que o dispositivo é falsificado: só {:.1} GB existem de verdade. A formatação usa só essa parte, com MBR (a GPT grava no fim do disco, que não existe).",
            real as f64 / 1e9
        ));
        warnings.push("Mesmo reformatado, não use um dispositivo falsificado para a única cópia de nada importante.".into());
        (PartitionTable::Mbr, Some(suggested))
    } else if facts.size_bytes > TWO_TIB {
        reasons.push("Acima de 2 TB é preciso GPT: o MBR não endereça além disso.".into());
        if usage == Usage::Appliance {
            warnings.push("Muitos aparelhos não leem discos acima de 2 TB ou com GPT; prefira um disco menor para a TV.".into());
        }
        (PartitionTable::Gpt, None)
    } else {
        reasons.push("MBR é a tabela mais compatível para discos de até 2 TB.".into());
        (PartitionTable::Mbr, None)
    };

    match facts.health.as_deref() {
        Some("failing") => warnings.push("O SMART indica falha iminente: não guarde nada importante neste disco.".into()),
        Some("warning") => warnings.push("O SMART mostra sinais de desgaste: mantenha backup do que guardar aqui.".into()),
        _ => {}
    }
    if facts.usb_bottleneck {
        warnings.push("Está conectado como USB 2.0; numa porta USB 3 ele fica bem mais rápido.".into());
    }

    Advice { filesystem, table, max_bytes, reasons, warnings }
}
```

- [ ] **Step 4: Rodar** — `cargo test -q -p usbkeeper-core advice` → 6 passed; clippy limpo.
- [ ] **Step 5: Commit** — `feat(core): assistente de formatação (regras por uso e fatos do disco)`

---

### Task 2: Verificar/reparar — core + backend Linux

**Files:** Create `crates/core/src/repair.rs`, `crates/platform-linux/src/repair.rs`; Modify `crates/core/src/{lib,platform}.rs`, `crates/platform-linux/src/lib.rs`.

**Interfaces — Produces:**
- core: `pub enum Tool { Vfat, Exfat, Ntfs, Ext }`; `pub enum RepairStatus { Clean, Fixed, ErrorsRemain, Unsupported, Failed }` (Serialize snake_case, TS); `pub struct PartitionRepair { path: String, fstype: Option<String>, status: RepairStatus, log: Vec<String>, note: Option<String> }`; `pub struct RepairReport { check_only: bool, partitions: Vec<PartitionRepair>, table_warning: Option<String> }` com `fn verdict(&self) -> &'static str` (`"errors"`/`"fixed"`/`"clean"`); `pub fn check_status(tool: Tool, code: i32) -> RepairStatus`; `pub fn repair_status(tool: Tool, repair_code: i32, recheck_code: i32) -> RepairStatus`.
- core `platform.rs`: `pub trait Repairer { fn repair(&self, device: &Device, check_only: bool, step: &mut dyn FnMut(&str, u64, u64)) -> Result<RepairReport, CoreError>; }`
- linux: `pub fn tool_for(fstype: &str) -> Option<Tool>`; `pub fn command(tool: Tool, part: &str, repair: bool) -> (&'static str, Vec<String>)`; `pub struct LinuxRepairer { pub locate: fn(&str) -> Option<PathBuf> }` (Default = `find_tool`) implementando `Repairer`.

- [ ] **Step 1: Testes que falham**

`crates/core/src/repair.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_codes_per_tool() {
        assert_eq!(check_status(Tool::Ext, 0), RepairStatus::Clean);
        assert_eq!(check_status(Tool::Ext, 4), RepairStatus::ErrorsRemain);
        assert_eq!(check_status(Tool::Ext, 1), RepairStatus::ErrorsRemain);
        assert_eq!(check_status(Tool::Ext, 8), RepairStatus::Failed);
        assert_eq!(check_status(Tool::Vfat, 1), RepairStatus::ErrorsRemain);
        assert_eq!(check_status(Tool::Vfat, 2), RepairStatus::Failed); // erro de uso
        assert_eq!(check_status(Tool::Ntfs, 1), RepairStatus::ErrorsRemain);
    }

    #[test]
    fn repair_result_comes_from_recheck() {
        assert_eq!(repair_status(Tool::Ext, 1, 0), RepairStatus::Fixed);
        assert_eq!(repair_status(Tool::Ext, 0, 0), RepairStatus::Clean);
        assert_eq!(repair_status(Tool::Ext, 1, 4), RepairStatus::ErrorsRemain);
        assert_eq!(repair_status(Tool::Vfat, 2, 0), RepairStatus::Failed);
        assert_eq!(repair_status(Tool::Exfat, 1, 8), RepairStatus::Failed);
    }

    #[test]
    fn report_verdict() {
        let p = |status| PartitionRepair { path: "/dev/sdb1".into(), fstype: None, status, log: vec![], note: None };
        let r = |v: Vec<PartitionRepair>| RepairReport { check_only: false, partitions: v, table_warning: None };
        assert_eq!(r(vec![p(RepairStatus::Clean)]).verdict(), "clean");
        assert_eq!(r(vec![p(RepairStatus::Clean), p(RepairStatus::Fixed)]).verdict(), "fixed");
        assert_eq!(r(vec![p(RepairStatus::Fixed), p(RepairStatus::ErrorsRemain)]).verdict(), "errors");
        assert_eq!(r(vec![p(RepairStatus::Unsupported)]).verdict(), "clean");
    }
}
```

`crates/platform-linux/src/repair.rs` (testes com ferramentas falsas em shell):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use usbkeeper_core::{Partition, Transport};

    #[test]
    fn tools_and_commands() {
        assert_eq!(tool_for("vfat"), Some(Tool::Vfat));
        assert_eq!(tool_for("ntfs3"), Some(Tool::Ntfs));
        assert_eq!(tool_for("ext4"), Some(Tool::Ext));
        assert_eq!(tool_for("btrfs"), None);
        assert_eq!(command(Tool::Vfat, "/dev/sdb1", false), ("fsck.vfat", vec!["-n".into(), "/dev/sdb1".into()]));
        assert_eq!(command(Tool::Vfat, "/dev/sdb1", true).1, ["-a", "-w", "/dev/sdb1"]);
        assert_eq!(command(Tool::Exfat, "/dev/sdb1", true).1, ["-y", "/dev/sdb1"]);
        assert_eq!(command(Tool::Ntfs, "/dev/sdb1", true).1, ["-d", "/dev/sdb1"]);
        assert_eq!(command(Tool::Ext, "/dev/sdb1", false).1, ["-f", "-n", "/dev/sdb1"]);
        assert_eq!(command(Tool::Ext, "/dev/sdb1", true).1, ["-f", "-p", "/dev/sdb1"]);
    }

    fn device(parts: &[(&str, Option<&str>)]) -> Device {
        Device {
            name: "sdb".into(), path: "/dev/sdb".into(), size_bytes: 1, transport: Transport::Usb,
            removable: true, hotplug: true, read_only: false, vendor: None, model: None, serial: None,
            partition_table: Some("dos".into()), logical_sector: 512, physical_sector: 512, mountpoints: vec![],
            partitions: parts.iter().map(|(p, fs)| Partition { name: p.to_string(), path: format!("/dev/{p}"),
                size_bytes: 1, fstype: fs.map(String::from), label: None, mountpoints: vec![] }).collect(),
            usb: None,
        }
    }

    thread_local!(static BIN: std::cell::RefCell<PathBuf> = std::cell::RefCell::new(PathBuf::new()));
    fn fake_locate(name: &str) -> Option<PathBuf> {
        BIN.with(|b| Some(b.borrow().join(name))).filter(|p| p.is_file())
    }

    /// fsck.vfat falso: com -n falha (1) até que um -a crie o arquivo "consertado".
    fn install_fakes(dir: &std::path::Path) {
        let vfat = format!(
            "#!/bin/sh\nstate={}/consertado\nfor a in \"$@\"; do [ \"$a\" = -a ] && {{ echo 'Dirty bit is set. Fs was not properly unmounted'; touch $state; exit 1; }}; done\n[ -e $state ] && exit 0\necho 'Dirty bit is set.'; exit 1\n",
            dir.display()
        );
        for (name, body) in [("fsck.vfat", vfat.as_str()), ("sfdisk", "#!/bin/sh\nexit 0\n")] {
            let p = dir.join(name);
            std::fs::write(&p, body).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn check_then_repair_with_recheck() {
        let dir = tempfile::tempdir().unwrap();
        install_fakes(dir.path());
        BIN.with(|b| *b.borrow_mut() = dir.path().to_path_buf());
        let r = LinuxRepairer { locate: fake_locate };
        let dev = device(&[("sdb1", Some("vfat")), ("sdb2", Some("btrfs"))]);
        let mut steps = Vec::new();

        let check = r.repair(&dev, true, &mut |l, d, t| steps.push((l.to_string(), d, t))).unwrap();
        assert_eq!(check.partitions[0].status, RepairStatus::ErrorsRemain);
        assert!(check.partitions[0].log.iter().any(|l| l.contains("Dirty bit")));
        assert_eq!(check.partitions[1].status, RepairStatus::Unsupported);
        assert_eq!(check.verdict(), "errors");

        let fixed = r.repair(&dev, false, &mut |_, _, _| {}).unwrap();
        assert_eq!(fixed.partitions[0].status, RepairStatus::Fixed);
        assert_eq!(fixed.verdict(), "fixed");
        assert_eq!(steps.len(), 2, "um passo por partição");
    }

    #[test]
    fn missing_tool_aborts_before_running() {
        let dir = tempfile::tempdir().unwrap();
        BIN.with(|b| *b.borrow_mut() = dir.path().to_path_buf()); // nada instalado
        let r = LinuxRepairer { locate: fake_locate };
        let err = r.repair(&device(&[("sdb1", Some("exfat"))]), true, &mut |_, _, _| {}).unwrap_err();
        assert!(matches!(err, CoreError::MissingTool { ref tool, .. } if tool == "fsck.exfat"));
    }
}
```

- [ ] **Step 2: Ver falhar** — `cargo test -q -p usbkeeper-core repair; cargo test -q -p usbkeeper-linux repair` → não compilam.

- [ ] **Step 3: Implementação**

`crates/core/src/repair.rs`:

```rust
//! Verificar/reparar sistemas de arquivos: tipos e interpretação dos
//! códigos de saída (que diferem entre as ferramentas).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool { Vfat, Exfat, Ntfs, Ext }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum RepairStatus { Clean, Fixed, ErrorsRemain, Unsupported, Failed }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PartitionRepair {
    pub path: String,
    pub fstype: Option<String>,
    pub status: RepairStatus,
    pub log: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RepairReport {
    pub check_only: bool,
    pub partitions: Vec<PartitionRepair>,
    pub table_warning: Option<String>,
}

impl RepairReport {
    pub fn verdict(&self) -> &'static str {
        let any = |s: &[RepairStatus]| self.partitions.iter().any(|p| s.contains(&p.status));
        if any(&[RepairStatus::ErrorsRemain, RepairStatus::Failed]) {
            "errors"
        } else if any(&[RepairStatus::Fixed]) {
            "fixed"
        } else {
            "clean"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome { Clean, Changed, Errors, Failed }

fn outcome(tool: Tool, code: i32) -> Outcome {
    match tool {
        _ if code == 0 => Outcome::Clean,
        Tool::Ntfs => Outcome::Errors,
        Tool::Vfat if code == 1 => Outcome::Changed,
        Tool::Vfat => Outcome::Failed, // 2 = erro de uso
        Tool::Ext | Tool::Exfat if !(0..8).contains(&code) => Outcome::Failed,
        Tool::Ext | Tool::Exfat if code & 4 != 0 => Outcome::Errors,
        Tool::Ext | Tool::Exfat => Outcome::Changed,
    }
}

pub fn check_status(tool: Tool, code: i32) -> RepairStatus {
    match outcome(tool, code) {
        Outcome::Clean => RepairStatus::Clean,
        Outcome::Changed | Outcome::Errors => RepairStatus::ErrorsRemain,
        Outcome::Failed => RepairStatus::Failed,
    }
}

/// O status final do reparo vem da nova verificação feita logo depois.
pub fn repair_status(tool: Tool, repair_code: i32, recheck_code: i32) -> RepairStatus {
    let repaired = outcome(tool, repair_code);
    if repaired == Outcome::Failed {
        return RepairStatus::Failed;
    }
    match outcome(tool, recheck_code) {
        Outcome::Clean if repaired == Outcome::Changed => RepairStatus::Fixed,
        Outcome::Clean => RepairStatus::Clean,
        Outcome::Failed => RepairStatus::Failed,
        _ => RepairStatus::ErrorsRemain,
    }
}
```

`crates/core/src/platform.rs` += `use crate::repair::RepairReport;` e

```rust
/// Verifica (check_only) ou repara os sistemas de arquivos das partições.
/// `step(rótulo, feito, total)` é chamado antes de cada partição.
pub trait Repairer {
    fn repair(&self, device: &Device, check_only: bool, step: &mut dyn FnMut(&str, u64, u64))
        -> Result<RepairReport, CoreError>;
}
```

(`lib.rs`: `pub mod repair;` e `Repairer` no `pub use platform::{…}`.)

`crates/platform-linux/src/repair.rs`:

```rust
//! Verificar/reparar no Linux: fsck.vfat, fsck.exfat, ntfsfix, e2fsck.
//! Sempre com LC_ALL=C; reparar termina com uma nova verificação.

use std::path::PathBuf;
use std::process::Command;

use usbkeeper_core::repair::{PartitionRepair, RepairReport, RepairStatus, Tool, check_status, repair_status};
use usbkeeper_core::{CoreError, Device, Repairer};

use crate::format::find_tool;

const LOG_LINES: usize = 30;

pub fn tool_for(fstype: &str) -> Option<Tool> {
    match fstype {
        "vfat" => Some(Tool::Vfat),
        "exfat" => Some(Tool::Exfat),
        "ntfs" | "ntfs3" => Some(Tool::Ntfs),
        "ext2" | "ext3" | "ext4" => Some(Tool::Ext),
        _ => None,
    }
}

pub fn command(tool: Tool, part: &str, repair: bool) -> (&'static str, Vec<String>) {
    let (prog, flags): (&str, &[&str]) = match (tool, repair) {
        (Tool::Vfat, false) => ("fsck.vfat", &["-n"]),
        (Tool::Vfat, true) => ("fsck.vfat", &["-a", "-w"]),
        (Tool::Exfat, false) => ("fsck.exfat", &["-n"]),
        (Tool::Exfat, true) => ("fsck.exfat", &["-y"]),
        (Tool::Ntfs, false) => ("ntfsfix", &["-n"]),
        (Tool::Ntfs, true) => ("ntfsfix", &["-d"]),
        (Tool::Ext, false) => ("e2fsck", &["-f", "-n"]),
        (Tool::Ext, true) => ("e2fsck", &["-f", "-p"]),
    };
    let mut args: Vec<String> = flags.iter().map(|f| f.to_string()).collect();
    args.push(part.to_string());
    (prog, args)
}

fn package(prog: &str) -> &'static str {
    match prog {
        "fsck.vfat" => "dosfstools",
        "fsck.exfat" => "exfatprogs",
        "ntfsfix" => "ntfs-3g",
        "e2fsck" => "e2fsprogs",
        _ => "util-linux",
    }
}

pub struct LinuxRepairer {
    pub locate: fn(&str) -> Option<PathBuf>,
}

impl Default for LinuxRepairer {
    fn default() -> Self {
        Self { locate: find_tool }
    }
}

struct Run { code: i32, lines: Vec<String> }

impl LinuxRepairer {
    fn path(&self, prog: &str) -> Result<PathBuf, CoreError> {
        (self.locate)(prog).ok_or_else(|| CoreError::MissingTool { tool: prog.into(), package: package(prog).into() })
    }

    fn run(&self, prog: &str, args: &[String]) -> Result<Run, CoreError> {
        let out = Command::new(self.path(prog)?).args(args).env("LC_ALL", "C").output()?;
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let lines: Vec<String> = text.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
        let tail = lines[lines.len().saturating_sub(LOG_LINES)..].to_vec();
        Ok(Run { code: out.status.code().unwrap_or(-1), lines: tail })
    }

    fn table_warning(&self, device: &Device) -> Option<String> {
        let run = self.run("sfdisk", &["--verify".into(), device.path.clone()]).ok()?;
        (run.code != 0).then(|| format!("a tabela de partições tem inconsistências: {}", run.lines.join(" / ")))
    }
}

impl Repairer for LinuxRepairer {
    fn repair(&self, device: &Device, check_only: bool, step: &mut dyn FnMut(&str, u64, u64))
        -> Result<RepairReport, CoreError> {
        // Ferramentas resolvidas antes de começar.
        for p in &device.partitions {
            if let Some(tool) = p.fstype.as_deref().and_then(tool_for) {
                self.path(command(tool, &p.path, false).0)?;
            }
        }
        let total = device.partitions.len() as u64;
        let mut partitions = Vec::new();
        for (i, p) in device.partitions.iter().enumerate() {
            let verb = if check_only { "verificando" } else { "reparando" };
            step(&format!("{verb} {}", p.path), i as u64 + 1, total);
            let Some(tool) = p.fstype.as_deref().and_then(tool_for) else {
                partitions.push(PartitionRepair {
                    path: p.path.clone(), fstype: p.fstype.clone(), status: RepairStatus::Unsupported, log: vec![],
                    note: Some(match &p.fstype {
                        Some(fs) => format!("o sistema de arquivos {fs} não é verificado pelo usbkeeper"),
                        None => "partição sem sistema de arquivos reconhecido".into(),
                    }),
                });
                continue;
            };
            let (prog, check_args) = command(tool, &p.path, false);
            let (status, log) = if check_only {
                let run = self.run(prog, &check_args)?;
                (check_status(tool, run.code), run.lines)
            } else {
                let (_, repair_args) = command(tool, &p.path, true);
                let repaired = self.run(prog, &repair_args)?;
                let recheck = self.run(prog, &check_args)?;
                let mut log = repaired.lines;
                log.push("--- nova verificação ---".into());
                log.extend(recheck.lines);
                let log = log[log.len().saturating_sub(LOG_LINES)..].to_vec();
                (repair_status(tool, repaired.code, recheck.code), log)
            };
            let note = (tool == Tool::Ntfs && status == RepairStatus::ErrorsRemain)
                .then(|| "o ntfsfix só corrige o básico: rode `chkdsk /f` no Windows para um reparo completo".to_string());
            partitions.push(PartitionRepair { path: p.path.clone(), fstype: p.fstype.clone(), status, log, note });
        }
        Ok(RepairReport { check_only, partitions, table_warning: self.table_warning(device) })
    }
}
```

`crates/platform-linux/src/lib.rs`: `pub mod repair;` e `pub use repair::LinuxRepairer;`.

- [ ] **Step 4: Rodar** — `cargo test -q -p usbkeeper-core -p usbkeeper-linux` → todos ok; clippy limpo.
- [ ] **Step 5: Commit** — `feat: verificar/reparar sistemas de arquivos (core + Linux), com nova verificação após o reparo`

---

### Task 3: CLI — `advise`/`recomendar` e `repair`/`reparar`

**Files:** Create `apps/cli/src/advise.rs`, `apps/cli/src/repair.rs`; Modify `apps/cli/src/{main,backend,destructive,store}.rs`, `crates/history/src/lib.rs` (`kind::REPAIR = "repair"`), `crates/core/src/blockdev.rs` não muda.

**Interfaces:**
- Consumes: `advise()`, `DiskFacts::of`, `Repairer`, `RepairReport::verdict`.
- Produces: `Command::Advise { device, uso: Usage, arquivos_grandes: BigFiles, json }` (`#[command(name = "advise", alias = "recomendar")]`, flags `--uso`, `--arquivos-grandes` default `nao-sei`); `Command::Repair { device, check: bool, guard: GuardArgs, json }` (`alias = "reparar"`); `Backend.repairer: Box<dyn Repairer>`; `destructive::take(b, device, guard) -> Result<Device>` (desmonta + reconfere, sem pedir série) e `confirm_and_take` passa a chamar `take`; `store::last_facts(device: &Device) -> (Option<(u64, u64)>, Option<String>)` (último fake falso e última saúde, do histórico).

- [ ] **Step 1: Testes que falham** (`main.rs`, `mod tests`)

```rust
    #[test]
    fn advise_and_repair_parse() {
        let cli = Cli::try_parse_from(["usbkeeper", "recomendar", "sdb", "--uso", "tv", "--arquivos-grandes", "sim"]).unwrap();
        assert!(matches!(cli.command, Command::Advise { uso: Usage::Appliance, arquivos_grandes: BigFiles::Yes, .. }));
        let cli = Cli::try_parse_from(["usbkeeper", "reparar", "sdb", "--check"]).unwrap();
        assert!(matches!(cli.command, Command::Repair { check: true, .. }));
    }
```

- [ ] **Step 2: Ver falhar** — `cargo test -q -p usbkeeper` → não compila.

- [ ] **Step 3: Implementação**

`store.rs` += :

```rust
/// Do histórico: (capacidade real, limite sugerido) do último fake-check se
/// ele deu falso, e o último veredito de saúde.
pub fn last_facts(device: &Device) -> (Option<(u64, u64)>, Option<String>) {
    let Ok(h) = open() else { return (None, None) };
    let Ok(events) = h.events(&device_info(device).key()) else { return (None, None) };
    let fake = events.iter().rev().find(|e| e.kind == usbkeeper_history::kind::FAKE_CHECK)
        .filter(|e| e.verdict.as_deref() == Some("fake"))
        .and_then(|e| Some((e.data.get("real_bytes")?.as_u64()?, e.data.get("suggested_max_bytes")?.as_u64()?)));
    let health = events.iter().rev().find(|e| e.kind == usbkeeper_history::kind::HEALTH).and_then(|e| e.verdict.clone());
    (fake, health)
}
```

(`open()` não deve criar o banco só para ler: `last_facts` retorna `(None, None)` se `db_path()` não existir — checar `db_path().is_some_and(|p| p.exists())` antes.)

`advise.rs`:

```rust
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
    println!("Recomendado: {} em tabela {}{}", a.filesystem, a.table,
        a.max_bytes.map(|m| format!(", usando só os primeiros {} MiB", m >> 20)).unwrap_or_default());
    println!("\nPor quê:");
    for r in &a.reasons { println!("  • {r}"); }
    if !a.warnings.is_empty() {
        println!("\nAtenção:");
        for w in &a.warnings { println!("  ⚠ {w}"); }
    }
    println!("\nPara formatar assim:\n  {}\n(entenda: usbkeeper guia sistemas-de-arquivos)", command_line(device, a));
}
```

`repair.rs`:

```rust
use anyhow::{Result, bail};
use usbkeeper_core::repair::{RepairReport, RepairStatus};
use usbkeeper_history::kind;

use crate::backend::Backend;
use crate::destructive::{Guard, confirm_and_take, precheck, take};
use crate::store;

pub fn repair(b: &Backend, device: &str, check: bool, guard: Guard, json: bool) -> Result<()> {
    let dev = b.enumerator.get(device)?;
    precheck(&dev, &guard)?;
    if !json {
        eprintln!("{} {}  {}", if check { "Verificando (não altera nada)" } else { "Reparando" }, dev.path, dev.display_name());
        if !check {
            eprintln!("O reparo pode mover arquivos danificados para pastas como FSCK0000.REC ou lost+found. Tenha backup.");
        }
    }
    let current = if check { take(b, &dev, &guard)? } else { confirm_and_take(b, &dev, &guard)? };
    let report = b.repairer.repair(&current, check, &mut |label, done, total| {
        if crate::machine::get().progress_json {
            eprintln!("{}", crate::machine::step_line_phase("repair", label, done, total));
        } else if !json {
            eprintln!("[{done}/{total}] {label}");
        }
    })?;
    if report.partitions.is_empty() {
        bail!("{} não tem partições para verificar", current.path);
    }
    store::record(&current, kind::REPAIR, Some(report.verdict()), &summary(&report), serde_json::json!(report));
    if json {
        return crate::print_json(&serde_json::json!({ "device": current.path, "report": report }));
    }
    print_report(&report);
    Ok(())
}

pub fn summary(r: &RepairReport) -> String {
    let count = |s| r.partitions.iter().filter(|p| p.status == s).count();
    format!("{}: {} sem problemas, {} corrigidas, {} com erros",
        if r.check_only { "verificação" } else { "reparo" },
        count(RepairStatus::Clean), count(RepairStatus::Fixed), count(RepairStatus::ErrorsRemain) + count(RepairStatus::Failed))
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
        println!("{}  ({})  {label}", p.path, p.fstype.as_deref().unwrap_or("?"));
        if let Some(n) = &p.note { println!("  {n}"); }
        if matches!(p.status, RepairStatus::ErrorsRemain | RepairStatus::Failed | RepairStatus::Fixed) {
            for l in p.log.iter().rev().take(6).rev() { println!("    {l}"); }
        }
    }
    if let Some(w) = &r.table_warning { println!("\n⚠ {w}"); }
    if r.verdict() == "errors" {
        println!("\nSe os erros continuarem, veja a saúde do disco: usbkeeper health <disco> --scan");
    }
    println!("Entenda: usbkeeper guia reparar");
}
```

`machine.rs`: generalizar `step_line(label, done, total)` em `step_line_phase(phase: &str, label: &str, done, total)` e manter `step_line` chamando `step_line_phase("format", …)`.

`destructive.rs`: extrair de `confirm_and_take` a parte depois da confirmação:

```rust
/// Desmonta (se `--unmount`) e relê o disco conferindo identidade e regras.
/// Usado direto pelo `repair --check`, que não pede a série.
pub fn take(b: &Backend, device: &Device, guard: &Guard) -> Result<Device> { /* corpo atual após confirm() */ }

pub fn confirm_and_take(b: &Backend, device: &Device, guard: &Guard) -> Result<Device> {
    ensure_writable(device)?;
    confirm(device, guard.yes_i_know.as_deref())?;
    take(b, device, guard)
}
```

`backend.rs`: `repairer: Box<dyn Repairer>` (Linux: `LinuxRepairer::default()`; outros: `Unsupported`).

`main.rs`: variantes

```rust
    /// Recomenda como formatar para o seu uso (apelido: recomendar)
    #[command(name = "advise", alias = "recomendar")]
    Advise {
        device: String,
        /// misto | windows | linux | aparelho | videogame
        #[arg(long)]
        uso: Usage,
        /// Vai guardar arquivos maiores que 4 GB? sim | nao | nao-sei
        #[arg(long, default_value = "nao-sei")]
        arquivos_grandes: BigFiles,
        #[arg(long)]
        json: bool,
    },
    /// Verifica (--check, não altera nada) ou repara os sistemas de arquivos (apelido: reparar)
    #[command(alias = "reparar")]
    Repair {
        device: String,
        #[arg(long)]
        check: bool,
        #[command(flatten)]
        guard: GuardArgs,
        #[arg(long)]
        json: bool,
    },
```

e no `run`: `Command::Advise { … } => advise::advise(&backend(true), &device, uso, arquivos_grandes, json)?`, `Command::Repair { … } => repair::repair(&backend(true), &device, check, guard.into(), json)?`.

- [ ] **Step 4: Rodar** — `cargo test -q --workspace --exclude usbkeeper-gui` ok; `cargo run -q -p usbkeeper -- recomendar sda --uso windows` mostra NTFS (HD interno não removível) com o comando `format`.
- [ ] **Step 5: Commit** — `feat(cli): recomendar formatação e verificar/reparar (advise/recomendar, repair/reparar)`

---

### Task 4: Guia — tópico `reparar`

**Files:** Create `crates/guide/topics/reparar.md`; Modify `crates/guide/src/lib.rs` (13 ids, apelidos `fsck`/`repair`/`reparo`, `MUST` com `("reparar", &["verificar", "FSCK", "lost+found", "chkdsk", "backup", "Saúde", "corrigido"])`, `assert_eq!(all.len(), 13)`), `apps/gui/src/lib/guideLinks.ts` (`repair: "reparar"`), `scripts/e2e-loop.sh` (13 tópicos).

- [ ] **Step 1:** atualizar testes (13 tópicos, fatos do `reparar`) → falham.
- [ ] **Step 2: Escrever o tópico** (≥ 250 palavras, “Na prática” e “Cuidado”): o que é inconsistência (desligou no meio, arrancou sem remover, queda de energia); verificar × reparar (verificar não altera; reparar corrige e verifica de novo); significado de cada status (sem problemas, corrigido, ainda tem erros, não verificado, a ferramenta falhou); arquivos recuperados em `FSCK0000.REC` (FAT/exFAT) e `lost+found` (ext4); NTFS e `chkdsk /f`; quando reparar não basta (setores ruins → ver o tópico **Saúde** e a varredura; disco falso → ver falsificados); **backup antes de reparar**.
- [ ] **Step 3:** `cargo test -q -p usbkeeper-guide` ok. **Step 4: Commit** — `docs(guide): tópico verificar e reparar`

---

### Task 5: Interface — tema Sistema/Claro/Escuro

**Files:** Create `apps/gui/src/lib/theme.ts`, `theme.test.ts`, `apps/gui/src/features/theme/ThemeSwitch.tsx`; Modify `apps/gui/index.html`, `src/index.css`, `src/App.tsx`.

**Interfaces — Produces:** `type ThemePref = "system" | "light" | "dark"`; `getTheme(): ThemePref`; `setTheme(t: ThemePref): void` (salva e aplica); `applyTheme(t: ThemePref): void`.

- [ ] **Step 1: Teste**

```ts
import { beforeEach, describe, expect, it } from "vitest";
import { applyTheme, getTheme, setTheme } from "./theme";

beforeEach(() => { localStorage.clear(); document.documentElement.removeAttribute("data-theme"); });

describe("tema", () => {
  it("padrão é seguir o sistema", () => {
    expect(getTheme()).toBe("system");
  });
  it("escuro e claro gravam e aplicam data-theme; sistema remove", () => {
    setTheme("dark");
    expect(getTheme()).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    setTheme("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    setTheme("system");
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });
  it("valor inválido no armazenamento volta para sistema", () => {
    localStorage.setItem("usbkeeper.theme", "roxo");
    expect(getTheme()).toBe("system");
    applyTheme(getTheme());
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });
});
```

- [ ] **Step 2: Implementação**

`theme.ts`:

```ts
export type ThemePref = "system" | "light" | "dark";
const KEY = "usbkeeper.theme";

export function getTheme(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(t: ThemePref) {
  const root = document.documentElement;
  if (t === "system") root.removeAttribute("data-theme");
  else root.dataset.theme = t;
}

export function setTheme(t: ThemePref) {
  try {
    if (t === "system") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, t);
  } catch {
    /* sem armazenamento: aplica só nesta sessão */
  }
  applyTheme(t);
}
```

`index.html` (no `<head>`, antes do CSS):

```html
<script>
  try { var t = localStorage.getItem("usbkeeper.theme"); if (t === "light" || t === "dark") document.documentElement.dataset.theme = t; } catch (e) {}
</script>
```

(ajustar a CSP do `tauri.conf.json` com o hash desse script, ou mover o conteúdo para `src/theme-init.ts` importado primeiro no `main.tsx` — preferir a segunda opção para não afrouxar a CSP.)

`index.css`: trocar o bloco `@media (prefers-color-scheme: dark) { :root { … } }` por

```css
:root[data-theme="dark"] { /* tokens escuros */ }
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { /* tokens escuros */ } }
:root[data-theme="light"] { color-scheme: light; } :root[data-theme="dark"] { color-scheme: dark; }
```

`ThemeSwitch.tsx`: três botões (`Sistema`, `Claro`, `Escuro`) com `role="radiogroup"`/`aria-checked`, estado local iniciado com `getTheme()` e `setTheme` no clique; inserido no rodapé da barra lateral, acima de “Ajuda e guia”.

- [ ] **Step 3:** `npx vitest run src/lib/theme.test.ts` → 3 passed; build ok. **Step 4: Commit** — `feat(gui): tema Sistema/Claro/Escuro lembrado entre aberturas`

---

### Task 6: Interface — assistente na aba Formatar

**Files:** Create `apps/gui/src/features/format/AdviceCard.tsx`, `AdviceCard.test.tsx`; Modify `apps/gui/src-tauri/src/devices.rs` (comando `advise`), `src-tauri/src/lib.rs`, `apps/gui/src/lib/api.ts`, `features/format/FormatTab.tsx` (`initial`), `features/device/DevicePanel.tsx`.

**Interfaces:**
- Rust: `#[tauri::command] pub fn advise(device: String, usage: Usage, big_files: BigFiles) -> Result<Advice, String>` (lê o disco pelo enumerador e os fatos pelo histórico, igual a `store::last_facts`).
- TS: `api.advise(device, usage, bigFiles)`; `<AdviceCard device onApply={(a: Advice) => void} />`; `FormatTab` prop `initial?: { filesystem?: Fs; table?: "mbr" | "gpt"; maxBytes?: number }` (substitui `initialMaxBytes`).

- [ ] **Step 1: Teste** (`AdviceCard.test.tsx`)

```tsx
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AdviceCard } from "./AdviceCard";

const advise = vi.fn(() => Promise.resolve({
  filesystem: "fat32", table: "mbr", max_bytes: null,
  reasons: ["FAT32 é o formato que TVs leem."], warnings: ["Arquivos acima de 4 GB não cabem."],
}));
vi.mock("../../lib/api", () => ({ advise: (...a: unknown[]) => advise(...(a as [])) }));

describe("assistente de formatação", () => {
  it("pergunta, mostra a recomendação explicada e aplica", async () => {
    const onApply = vi.fn();
    render(<AdviceCard device="/dev/sdb" onApply={onApply} />);
    fireEvent.click(screen.getByRole("radio", { name: /TV, som, carro/ }));
    fireEvent.click(screen.getByRole("radio", { name: /Sim/ }));
    fireEvent.click(screen.getByRole("button", { name: "Ver recomendação" }));
    await waitFor(() => expect(screen.getByText(/FAT32 é o formato/)).toBeTruthy());
    expect(advise).toHaveBeenCalledWith("/dev/sdb", "appliance", "yes");
    expect(screen.getByText(/4 GB não cabem/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Usar esta configuração" }));
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({ filesystem: "fat32", table: "mbr" }));
  });
});
```

E em `FormatTab.test.tsx`, o teste do limite passa a usar `initial={{ maxBytes: 7_000_000_000 }}` e um novo caso `initial={{ filesystem: "ntfs", table: "gpt" }}` confere os selects.

- [ ] **Step 2: Implementação**

`AdviceCard.tsx`: duas perguntas com `role="radio"` (uso: “Computadores variados (Windows, Mac e Linux)” → `mixed`, “Só Windows” → `windows_only`, “Só Linux” → `linux_only`, “TV, som, carro ou aparelho antigo” → `appliance`, “Videogame” → `console`; arquivos > 4 GB: “Sim”/“Não”/“Não sei”), botão “Ver recomendação” (habilitado com uso escolhido) → `advise(device, usage, bigFiles)` → mostra `Recomendado: <fs> em <tabela>[, só os primeiros N MB]`, lista de motivos, avisos em tom `warn`, botão “Usar esta configuração” → `onApply(advice)`; “?” para `GUIDE.filesystems`.

`DevicePanel.tsx`: estado `formatInitial` (inicia com `{ maxBytes: suggested_max_bytes }`), `showAdvice`; na aba Formatar, botão “Me ajude a escolher” alterna o `AdviceCard`; `onApply(a)` → `setFormatInitial({ filesystem: a.filesystem, table: a.table, maxBytes: a.max_bytes ?? undefined })`, fecha o cartão; o `FormatTab` é remontado por `key={JSON.stringify(formatInitial)}`. “Reformatar só a parte real” passa a `setFormatInitial({ maxBytes: m })`.

- [ ] **Step 3:** `cargo test -q -p usbkeeper-gui`; `npx vitest run` ok; build ok. **Step 4: Commit** — `feat(gui): assistente 'me ajude a escolher' na aba Formatar`

---

### Task 7: Interface — aba Reparar

**Files:** Create `apps/gui/src/features/repair/RepairTab.tsx`, `RepairTab.test.tsx`; Modify `apps/gui/src-tauri/src/cli.rs` (`OpRequest::Repair { device: String, check: bool }`), `features/device/DevicePanel.tsx` (aba `repair`, destrutiva), `lib/format.ts` (rótulos `fixed`/`errors`).

**Interfaces:** `cli::args(Repair{check:true})` → `["repair", dev, "--check", "--unmount", "--json", "--progress-json", "--cancel-stdin"]`; `check:false` → `["repair", dev, "--unmount", "--yes-i-know", token, …]`. `kind()` = `"repair"`.

- [ ] **Step 1: Testes**

Rust (`cli.rs`):

```rust
    #[test]
    fn repair_check_never_confirms_and_repair_does() {
        let a = args(&OpRequest::Repair { device: "/dev/sdb".into(), check: true }, "ABC", false);
        assert_eq!(a, s(&["repair", "/dev/sdb", "--check", "--unmount", "--json", "--progress-json", "--cancel-stdin"]));
        let a = args(&OpRequest::Repair { device: "/dev/sdb".into(), check: false }, "ABC", false);
        assert!(a.windows(2).any(|w| w == ["--yes-i-know", "ABC"]));
        assert!(!a.contains(&"--check".to_string()));
    }
```

`RepairTab.test.tsx`:

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { RepairTab } from "./RepairTab";
import { useApp } from "../../lib/store";
import { view } from "../../test/fixtures";

const start = vi.fn(() => Promise.resolve(1));
vi.mock("../../lib/api", () => ({ startOperation: (...a: unknown[]) => start(...(a as [])) }));
beforeEach(() => { start.mockClear(); useApp.setState({ ops: {}, finished: {} }); });

describe("aba Reparar", () => {
  it("verificar inicia sem pedir a série", () => {
    render(<RepairTab view={view()} />);
    fireEvent.click(screen.getByRole("button", { name: /Verificar/ }));
    expect(start).toHaveBeenCalledWith({ kind: "repair", device: "/dev/sdb", check: true }, "ABC");
  });

  it("reparar pede a série antes", () => {
    render(<RepairTab view={view()} />);
    fireEvent.click(screen.getByRole("button", { name: /^Reparar/ }));
    expect(start).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("mostra o resultado por partição", () => {
    useApp.setState({ finished: { "/dev/sdb": { repair: { id: 1, device: "/dev/sdb", kind: "repair", ok: true, error: null, holders: [],
      result: { report: { check_only: true, table_warning: null, partitions: [
        { path: "/dev/sdb1", fstype: "vfat", status: "errors_remain", log: ["Dirty bit is set."], note: null }] } } } } } });
    render(<RepairTab view={view()} />);
    expect(screen.getByText(/\/dev\/sdb1/)).toBeTruthy();
    expect(screen.getByText(/tem problemas/)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Implementação** — `cli.rs` (variante, `device()`, `kind()`, `args`); `RepairTab.tsx` com os dois botões (“Verificar (não altera nada)” e “Reparar…” com `ConfirmDialog` e as linhas do aviso de `FSCK*.REC`/`lost+found`), resultado por partição (selo: sem problemas/corrigido/tem problemas — rode o reparo/ainda tem erros/não verificado/a ferramenta falhou), nota, aviso da tabela, `<details>` com o log, “?” para `GUIDE.repair`; `DevicePanel`: aba `{ id: "repair", label: "Reparar", destructive: true }` entre Formatar e Histórico.

- [ ] **Step 3:** testes Rust e vitest ok; build ok. **Step 4: Commit** — `feat(gui): aba Reparar (verificar sem alterar e reparar com confirmação)`

---

### Task 8: e2e, README, versão 0.6.0, PR

- [ ] **Step 1: e2e** (antes do bloco M3):

```bash
# M6a: recomendar e reparar
a=$("$BIN" recomendar "$LOOP" --uso aparelho --json | jq -c .advice || echo null)
expect "recomendar para aparelho → FAT32/MBR" "$a" '.filesystem == "fat32" and .table == "mbr"'

"$BIN" format "$LOOP" --fs fat32 --allow-non-usb --yes-i-know "$LOOP" >/dev/null
printf '\x01' | dd of="${LOOP}p1" bs=1 seek=65 conv=notrunc status=none   # liga o "dirty bit" do FAT32
r=$("$BIN" repair "$LOOP" --check --allow-non-usb --json | jq -c .report || echo null)
expect "verificar acusa FAT32 sujo" "$r" '.partitions[0].status == "errors_remain"'
r=$("$BIN" repair "$LOOP" --allow-non-usb --yes-i-know "$LOOP" --json | jq -c .report || echo null)
expect "reparar corrige o FAT32" "$r" '.partitions[0].status == "fixed"'
r=$("$BIN" repair "$LOOP" --check --allow-non-usb --json | jq -c .report || echo null)
expect "nova verificação do FAT32 limpa" "$r" '.partitions[0].status == "clean"'

"$BIN" format "$LOOP" --fs ext4 --allow-non-usb --yes-i-know "$LOOP" >/dev/null
mount "${LOOP}p1" "$WORK/mnt" && echo x > "$WORK/mnt/x" && umount "$WORK/mnt"
debugfs -w -R "set_inode_field /x links_count 5" "${LOOP}p1" >/dev/null 2>&1
r=$("$BIN" repair "$LOOP" --check --allow-non-usb --json | jq -c .report || echo null)
expect "verificar acusa ext4 inconsistente" "$r" '.partitions[0].status == "errors_remain"'
r=$("$BIN" repair "$LOOP" --allow-non-usb --yes-i-know "$LOOP" --json | jq -c .report || echo null)
expect "reparar corrige o ext4" "$r" '.partitions[0].status == "fixed"'
```

  e o contador do guia passa para 13.
- [ ] **Step 2: README** — seções “Tema”, “Me ajude a escolher (recomendar)” e “Verificar e reparar”; roadmap `- [x] **M6a**`.
- [ ] **Step 3: Versão 0.6.0** (Cargo.toml, package.json, tauri.conf.json, `npm install --package-lock-only`); `cargo fmt/clippy/test`, `npm test/build`, `npx tauri build`.
- [ ] **Step 4: Commit, push, PR → dev** com checklist manual: alternar tema (Sistema/Claro/Escuro, fechar e abrir); “Me ajude a escolher” com TV num pendrive → FAT32/MBR preenchido; Verificar e Reparar num pendrive de teste.
