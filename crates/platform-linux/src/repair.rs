//! Verificar/reparar no Linux: fsck.vfat, fsck.exfat, ntfsfix e e2fsck.
//! Sempre com LC_ALL=C; reparar termina com uma nova verificação.

use std::path::PathBuf;
use std::process::Command;

use usbkeeper_core::repair::{
    PartitionRepair, RepairReport, RepairStatus, Tool, check_status, repair_status,
};
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

/// (programa, argumentos) para verificar (`repair = false`) ou reparar.
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
    /// Resolve o caminho de uma ferramenta; trocável nos testes.
    pub locate: fn(&str) -> Option<PathBuf>,
}

impl Default for LinuxRepairer {
    fn default() -> Self {
        Self { locate: find_tool }
    }
}

struct Run {
    code: i32,
    lines: Vec<String>,
}

fn tail(lines: Vec<String>) -> Vec<String> {
    lines[lines.len().saturating_sub(LOG_LINES)..].to_vec()
}

impl LinuxRepairer {
    fn path(&self, prog: &str) -> Result<PathBuf, CoreError> {
        (self.locate)(prog).ok_or_else(|| CoreError::MissingTool {
            tool: prog.into(),
            package: package(prog).into(),
        })
    }

    fn run(&self, prog: &str, args: &[String]) -> Result<Run, CoreError> {
        let out = Command::new(self.path(prog)?)
            .args(args)
            .env("LC_ALL", "C")
            .output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let lines = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(String::from)
            .collect();
        Ok(Run {
            code: out.status.code().unwrap_or(-1),
            lines: tail(lines),
        })
    }

    /// `sfdisk --verify`: só aponta inconsistências da tabela, não repara.
    fn table_warning(&self, device: &Device) -> Option<String> {
        let run = self
            .run("sfdisk", &["--verify".into(), device.path.clone()])
            .ok()?;
        (run.code != 0).then(|| {
            format!(
                "a tabela de partições tem inconsistências: {}",
                run.lines.join(" / ")
            )
        })
    }
}

impl Repairer for LinuxRepairer {
    fn repair(
        &self,
        device: &Device,
        check_only: bool,
        step: &mut dyn FnMut(&str, u64, u64),
    ) -> Result<RepairReport, CoreError> {
        // Todas as ferramentas resolvidas antes de começar.
        for p in &device.partitions {
            if let Some(tool) = p.fstype.as_deref().and_then(tool_for) {
                self.path(command(tool, &p.path, false).0)?;
            }
        }
        let total = device.partitions.len() as u64;
        let mut partitions = Vec::new();
        for (i, p) in device.partitions.iter().enumerate() {
            let verb = if check_only {
                "verificando"
            } else {
                "reparando"
            };
            step(&format!("{verb} {}", p.path), i as u64 + 1, total);
            let Some(tool) = p.fstype.as_deref().and_then(tool_for) else {
                partitions.push(PartitionRepair {
                    path: p.path.clone(),
                    fstype: p.fstype.clone(),
                    status: RepairStatus::Unsupported,
                    log: vec![],
                    note: Some(match &p.fstype {
                        Some(fs) => {
                            format!("o sistema de arquivos {fs} não é verificado pelo usbkeeper")
                        }
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
                (repair_status(tool, repaired.code, recheck.code), tail(log))
            };
            let note = (tool == Tool::Ntfs && status == RepairStatus::ErrorsRemain).then(|| {
                "o ntfsfix só corrige o básico: rode `chkdsk /f` no Windows para um reparo completo"
                    .to_string()
            });
            partitions.push(PartitionRepair {
                path: p.path.clone(),
                fstype: p.fstype.clone(),
                status,
                log,
                note,
            });
        }
        Ok(RepairReport {
            check_only,
            partitions,
            table_warning: self.table_warning(device),
        })
    }
}

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
        assert_eq!(
            command(Tool::Vfat, "/dev/sdb1", false),
            ("fsck.vfat", vec!["-n".to_string(), "/dev/sdb1".to_string()])
        );
        assert_eq!(
            command(Tool::Vfat, "/dev/sdb1", true).1,
            ["-a", "-w", "/dev/sdb1"]
        );
        assert_eq!(
            command(Tool::Exfat, "/dev/sdb1", true).1,
            ["-y", "/dev/sdb1"]
        );
        assert_eq!(
            command(Tool::Ntfs, "/dev/sdb1", true).1,
            ["-d", "/dev/sdb1"]
        );
        assert_eq!(
            command(Tool::Ext, "/dev/sdb1", false).1,
            ["-f", "-n", "/dev/sdb1"]
        );
        assert_eq!(
            command(Tool::Ext, "/dev/sdb1", true).1,
            ["-f", "-p", "/dev/sdb1"]
        );
    }

    fn device(parts: &[(&str, Option<&str>)]) -> Device {
        Device {
            name: "sdb".into(),
            path: "/dev/sdb".into(),
            size_bytes: 1,
            transport: Transport::Usb,
            removable: true,
            hotplug: true,
            read_only: false,
            vendor: None,
            model: None,
            serial: None,
            partition_table: Some("dos".into()),
            logical_sector: 512,
            physical_sector: 512,
            system_disk: false,
            mountpoints: vec![],
            partitions: parts
                .iter()
                .map(|(p, fs)| Partition {
                    name: p.to_string(),
                    path: format!("/dev/{p}"),
                    size_bytes: 1,
                    fstype: fs.map(String::from),
                    label: None,
                    mountpoints: vec![],
                })
                .collect(),
            usb: None,
        }
    }

    thread_local!(static BIN: std::cell::RefCell<PathBuf> = const { std::cell::RefCell::new(PathBuf::new()) });
    fn fake_locate(name: &str) -> Option<PathBuf> {
        BIN.with(|b| b.borrow().join(name))
            .is_file()
            .then(|| BIN.with(|b| b.borrow().join(name)))
    }

    /// fsck.vfat falso: com -n acusa (1) até que um -a crie o arquivo "consertado".
    fn install_fakes(dir: &std::path::Path) {
        let vfat = format!(
            "#!/bin/sh\nstate={}/consertado\nfor a in \"$@\"; do [ \"$a\" = -a ] && {{ echo 'Dirty bit is set. Fs was not properly unmounted'; touch $state; exit 1; }}; done\n[ -e $state ] && exit 0\necho 'Dirty bit is set.'; exit 1\n",
            dir.display()
        );
        for (name, body) in [
            ("fsck.vfat", vfat.as_str()),
            ("sfdisk", "#!/bin/sh\nexit 0\n"),
        ] {
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
        let r = LinuxRepairer {
            locate: fake_locate,
        };
        let dev = device(&[("sdb1", Some("vfat")), ("sdb2", Some("btrfs"))]);
        let mut steps = Vec::new();

        let check = r
            .repair(&dev, true, &mut |l, d, t| steps.push((l.to_string(), d, t)))
            .unwrap();
        assert_eq!(check.partitions[0].status, RepairStatus::ErrorsRemain);
        assert!(
            check.partitions[0]
                .log
                .iter()
                .any(|l| l.contains("Dirty bit"))
        );
        assert_eq!(check.partitions[1].status, RepairStatus::Unsupported);
        assert_eq!(check.verdict(), "errors");
        assert_eq!(steps.len(), 2, "um passo por partição");

        let fixed = r.repair(&dev, false, &mut |_, _, _| {}).unwrap();
        assert_eq!(fixed.partitions[0].status, RepairStatus::Fixed);
        assert_eq!(fixed.verdict(), "fixed");
        assert!(fixed.table_warning.is_none());
    }

    #[test]
    fn missing_tool_aborts_before_running() {
        let dir = tempfile::tempdir().unwrap();
        BIN.with(|b| *b.borrow_mut() = dir.path().to_path_buf()); // nada instalado
        let r = LinuxRepairer {
            locate: fake_locate,
        };
        let err = r
            .repair(&device(&[("sdb1", Some("exfat"))]), true, &mut |_, _, _| {})
            .unwrap_err();
        assert!(matches!(err, CoreError::MissingTool { ref tool, .. } if tool == "fsck.exfat"));
    }
}
