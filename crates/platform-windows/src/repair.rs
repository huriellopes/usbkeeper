//! Verificar/reparar no Windows com o `chkdsk` (FAT32, exFAT e NTFS).

use usbkeeper_core::Partition;

/// Sistemas de arquivos que o chkdsk verifica.
pub fn supported(fstype: &str) -> bool {
    matches!(fstype, "vfat" | "exfat" | "ntfs")
}

/// Alvo do chkdsk: a letra (`E:`) se houver, senão o volume (`\\?\Volume{…}`).
pub fn chkdsk_target(p: &Partition) -> Option<String> {
    // Só a raiz (`E:\`) é a letra do volume; `D:\pasta\` é montagem em pasta de OUTRO volume.
    let letter = p.mountpoints.iter().find_map(|m| {
        let b = m.as_bytes();
        let root = matches!(b, [l, b':'] | [l, b':', b'\\'] if l.is_ascii_alphabetic());
        root.then(|| format!("{}:", (b[0] as char).to_ascii_uppercase()))
    });
    letter.or_else(|| {
        p.path
            .starts_with(r"\\?\Volume")
            .then(|| p.path.trim_end_matches('\\').to_string())
    })
}

/// Só verificar (somente leitura) ou corrigir (`/f`) forçando a desmontagem (`/x`).
pub fn chkdsk_args(target: &str, fix: bool) -> Vec<String> {
    let mut a = vec![target.to_string()];
    if fix {
        a.extend(["/f".into(), "/x".into()]);
    }
    a
}

#[cfg(windows)]
mod run {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    use usbkeeper_core::repair::{
        PartitionRepair, RepairReport, RepairStatus, Tool, check_status, repair_status,
    };
    use usbkeeper_core::{CoreError, Device, Repairer};

    use super::{chkdsk_args, chkdsk_target, supported};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const LOG_LINES: usize = 30;

    pub struct WindowsRepairer;

    struct Run {
        code: i32,
        lines: Vec<String>,
    }

    fn chkdsk(args: &[String]) -> Result<Run, CoreError> {
        let out = Command::new("chkdsk.exe")
            .args(args)
            .stdin(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| CoreError::Backend(format!("não consegui executar o chkdsk: {e}")))?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        Ok(Run {
            code: out.status.code().unwrap_or(-1),
            lines: text
                .lines()
                .map(str::trim_end)
                .filter(|l| !l.trim().is_empty())
                .map(String::from)
                .collect(),
        })
    }

    fn tail(lines: Vec<String>) -> Vec<String> {
        lines[lines.len().saturating_sub(LOG_LINES)..].to_vec()
    }

    impl Repairer for WindowsRepairer {
        fn repair(
            &self,
            device: &Device,
            check_only: bool,
            step: &mut dyn FnMut(&str, u64, u64),
        ) -> Result<RepairReport, CoreError> {
            let total = device.partitions.len() as u64;
            let mut partitions = Vec::new();
            for (i, p) in device.partitions.iter().enumerate() {
                let verb = if check_only {
                    "verificando"
                } else {
                    "reparando"
                };
                step(&format!("{verb} {}", p.name), i as u64 + 1, total);
                let fs = p.fstype.as_deref();
                let target = fs.filter(|f| supported(f)).and_then(|_| chkdsk_target(p));
                let Some(target) = target else {
                    partitions.push(PartitionRepair {
                        path: p.path.clone(),
                        fstype: p.fstype.clone(),
                        status: RepairStatus::Unsupported,
                        log: vec![],
                        note: Some(match fs {
                            Some("ext2" | "ext3" | "ext4") => {
                                "o Windows não verifica ext4: use o usbkeeper no Linux".into()
                            }
                            Some(f) if supported(f) => {
                                "partição sem letra nem volume: use `usbkeeper montar` antes".into()
                            }
                            Some(f) => {
                                format!("o sistema de arquivos {f} não é verificado pelo usbkeeper")
                            }
                            None => "partição sem sistema de arquivos reconhecido".into(),
                        }),
                    });
                    continue;
                };
                let (status, log) = if check_only {
                    let r = chkdsk(&chkdsk_args(&target, false))?;
                    (check_status(Tool::Chkdsk, r.code), tail(r.lines))
                } else {
                    let fixed = chkdsk(&chkdsk_args(&target, true))?;
                    let recheck = chkdsk(&chkdsk_args(&target, false))?;
                    let mut log = fixed.lines;
                    log.push("--- nova verificação ---".into());
                    log.extend(recheck.lines);
                    (
                        repair_status(Tool::Chkdsk, fixed.code, recheck.code),
                        tail(log),
                    )
                };
                partitions.push(PartitionRepair {
                    path: target,
                    fstype: p.fstype.clone(),
                    status,
                    log,
                    note: None,
                });
            }
            Ok(RepairReport {
                check_only,
                partitions,
                table_warning: None,
            })
        }
    }
}

#[cfg(windows)]
pub use run::WindowsRepairer;

#[cfg(test)]
mod tests {
    use super::*;

    fn part(path: &str, mounts: &[&str]) -> Partition {
        Partition {
            name: "PhysicalDrive1-p1".into(),
            path: path.into(),
            size_bytes: 1,
            fstype: Some("vfat".into()),
            label: None,
            mountpoints: mounts.iter().map(|m| m.to_string()).collect(),
        }
    }

    #[test]
    fn target_prefers_letter_then_volume() {
        assert_eq!(
            chkdsk_target(&part(r"\\?\Volume{a}\", &[r"e:\", r"D:\pasta\"])).as_deref(),
            Some("E:")
        );
        assert_eq!(
            chkdsk_target(&part(r"\\?\Volume{a}\", &[r"D:\pasta\"])).as_deref(),
            Some(r"\\?\Volume{a}")
        );
        assert_eq!(chkdsk_target(&part(r"\\.\PhysicalDrive1-p1", &[])), None);
    }

    #[test]
    fn args_and_support() {
        assert_eq!(chkdsk_args("E:", false), vec!["E:"]);
        assert_eq!(chkdsk_args("E:", true), vec!["E:", "/f", "/x"]);
        assert!(supported("vfat") && supported("exfat") && supported("ntfs"));
        assert!(!supported("ext4") && !supported("refs"));
    }
}
