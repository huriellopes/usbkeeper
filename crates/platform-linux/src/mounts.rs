//! Desmontar, descobrir quem prende um ponto de montagem e ejetar.

use std::fs;
use std::path::Path;
use std::process::Command;

use usbkeeper_core::{CoreError, Device, Ejector, Holder, Mounts};

use crate::format::find_tool;

/// `/proc/self/mounts` escapa espaço, tab, \n e \ como octal (`\040`).
fn unescape_mount(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u8::from_str_radix(d, 8).ok());
        if let (b'\\', Some(code)) = (bytes[i], octal) {
            out.push(code);
            i += 4;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Dispositivo de origem de cada ponto de montagem (`mounts` = conteúdo de /proc/self/mounts).
pub fn mount_source(mounts: &str, mountpoint: &str) -> Option<String> {
    mounts.lines().find_map(|line| {
        let mut fields = line.split(' ');
        let source = fields.next()?;
        let target = unescape_mount(fields.next()?);
        (target == mountpoint).then(|| unescape_mount(source))
    })
}

fn is_under(path: &str, mountpoint: &str) -> bool {
    path == mountpoint
        || path
            .strip_prefix(mountpoint)
            .is_some_and(|rest| rest.starts_with('/') || mountpoint == "/")
}

/// Varre `<proc_root>/<pid>/{cwd,exe,fd/*}` procurando caminhos dentro dos
/// pontos de montagem. Sem root, só os processos do próprio usuário aparecem.
pub fn holders_in(proc_root: &Path, mountpoints: &[&str]) -> Vec<Holder> {
    let Ok(entries) = fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let dir = entry.path();
        let mut links = vec![dir.join("cwd"), dir.join("exe")];
        if let Ok(fds) = fs::read_dir(dir.join("fd")) {
            links.extend(fds.flatten().map(|f| f.path()));
        }
        let hit = links.iter().find_map(|link| {
            let target = fs::read_link(link).ok()?;
            let target = target
                .to_string_lossy()
                .trim_end_matches(" (deleted)")
                .to_string();
            mountpoints
                .iter()
                .any(|m| is_under(&target, m))
                .then_some(target)
        });
        if let Some(path) = hit {
            let command = fs::read_to_string(dir.join("comm"))
                .map(|c| c.trim().to_string())
                .unwrap_or_else(|_| "?".into());
            found.push(Holder { pid, command, path });
        }
    }
    found.sort_by_key(|h| h.pid);
    found
}

/// Espera o udev terminar de processar eventos pendentes. Fechar um disco
/// aberto para escrita (ou desmontá-lo) dispara uma releitura pelo udev, e
/// durante ela o lsblk devolve o sistema de arquivos vazio.
pub fn settle() {
    if let Some(udevadm) = find_tool("udevadm") {
        let _ = Command::new(udevadm)
            .args(["settle", "--timeout=10"])
            .status();
    }
}

/// Descarta (TRIM) o disco inteiro com `blkdiscard -f`. Muito rápido em
/// SSD/NVMe; muitas pontes USB e pendrives não aceitam.
pub fn discard(device_path: &str) -> Result<(), CoreError> {
    let blkdiscard = find_tool("blkdiscard").ok_or_else(|| CoreError::MissingTool {
        tool: "blkdiscard".into(),
        package: "util-linux".into(),
    })?;
    let out = Command::new(blkdiscard)
        .args(["-f", device_path])
        .env("LC_ALL", "C")
        .output()?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if stderr.to_lowercase().contains("not supported") {
        return Err(CoreError::Backend(
            "este dispositivo (ou a ponte USB) não aceita descarte; use sobrescrever".into(),
        ));
    }
    Err(CoreError::CommandFailed {
        command: format!("blkdiscard -f {device_path}"),
        stderr,
    })
}

pub fn is_root() -> bool {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(2).map(|euid| euid == "0"))
        })
        .unwrap_or(false)
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Sistemas de arquivos que o gerenciador de arquivos sabe abrir.
const MOUNTABLE: [&str; 7] = ["vfat", "exfat", "ntfs", "ntfs3", "ext2", "ext3", "ext4"];

/// Partições com sistema de arquivos montável e ainda não montadas.
pub fn mountable(
    parts: &[usbkeeper_core::Partition],
) -> impl Iterator<Item = &usbkeeper_core::Partition> {
    parts.iter().filter(|p| {
        p.mountpoints.is_empty()
            && p.fstype
                .as_deref()
                .is_some_and(|f| MOUNTABLE.contains(&f) || f == "ext2")
    })
}

/// "Mounted /dev/sdb1 at /media/ana/PEN" (algumas versões põem ponto final).
pub fn parse_mounted(out: &str) -> Option<String> {
    let line = out.lines().find(|l| l.starts_with("Mounted "))?;
    let (_, at) = line.split_once(" at ")?;
    Some(at.trim().trim_end_matches('.').to_string())
}

#[derive(Default)]
pub struct LinuxMounts;

impl LinuxMounts {
    fn unmount_one(&self, mountpoint: &str, mounts_table: &str) -> Result<(), String> {
        // udisks (polkit) desmonta o que o usuário montou sem precisar de root;
        // umount direto é o plano B quando rodando como root.
        let mut last_err = String::from("nenhuma ferramenta de desmontagem disponível");
        if let (Some(udisksctl), Some(source)) = (
            find_tool("udisksctl"),
            mount_source(mounts_table, mountpoint),
        ) {
            let udisks = udisksctl.display().to_string();
            match run(
                &udisks,
                &["unmount", "--no-user-interaction", "-b", &source],
            ) {
                Ok(()) => return Ok(()),
                Err(e) => last_err = e,
            }
        }
        if is_root()
            && let Some(umount) = find_tool("umount")
        {
            match run(&umount.display().to_string(), &[mountpoint]) {
                Ok(()) => return Ok(()),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }
}

impl Mounts for LinuxMounts {
    fn unmount_all(&self, device: &Device) -> Result<(), CoreError> {
        let table = fs::read_to_string("/proc/self/mounts")?;
        // Montagens mais profundas primeiro (/media/x/sub antes de /media/x).
        let mut points: Vec<&str> = device.all_mountpoints().collect();
        points.sort_by_key(|m| std::cmp::Reverse(m.len()));
        for mountpoint in points {
            if mountpoint == "[SWAP]" {
                return Err(CoreError::Backend(
                    "o disco tem swap ativa; desative com swapoff antes".into(),
                ));
            }
            if let Err(err) = self.unmount_one(mountpoint, &table) {
                let holders = holders_in(Path::new("/proc"), &[mountpoint]);
                if holders.is_empty() && !err.to_lowercase().contains("busy") {
                    return Err(CoreError::Backend(format!(
                        "não foi possível desmontar {mountpoint}: {err}"
                    )));
                }
                return Err(CoreError::Busy {
                    mountpoint: mountpoint.to_string(),
                    holders,
                });
            }
        }
        Ok(())
    }

    fn mount_all(&self, device: &Device) -> Result<Vec<String>, CoreError> {
        let udisksctl = find_tool("udisksctl").ok_or_else(|| CoreError::MissingTool {
            tool: "udisksctl".into(),
            package: "udisks2".into(),
        })?;
        let mut mounted = Vec::new();
        for p in mountable(&device.partitions) {
            let out = Command::new(&udisksctl)
                .args(["mount", "--no-user-interaction", "-b", &p.path])
                .env("LC_ALL", "C")
                .output()?;
            if !out.status.success() {
                return Err(CoreError::CommandFailed {
                    command: format!("udisksctl mount -b {}", p.path),
                    stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
                });
            }
            if let Some(at) = parse_mounted(&String::from_utf8_lossy(&out.stdout)) {
                mounted.push(at);
            }
        }
        Ok(mounted)
    }

    fn holders(&self, device: &Device) -> Vec<Holder> {
        let points: Vec<&str> = device.all_mountpoints().collect();
        holders_in(Path::new("/proc"), &points)
    }
}

#[derive(Default)]
pub struct LinuxEjector;

impl Ejector for LinuxEjector {
    fn power_off(&self, device: &Device) -> Result<(), CoreError> {
        let udisksctl = find_tool("udisksctl").ok_or_else(|| CoreError::MissingTool {
            tool: "udisksctl".into(),
            package: "udisks2".into(),
        })?;
        run(
            &udisksctl.display().to_string(),
            &["power-off", "--no-user-interaction", "-b", &device.path],
        )
        .map_err(|stderr| CoreError::CommandFailed {
            command: format!("udisksctl power-off -b {}", device.path),
            stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn reads_udisks_mount_output() {
        assert_eq!(
            parse_mounted("Mounted /dev/sdb1 at /media/ana/MEU PEN\n").as_deref(),
            Some("/media/ana/MEU PEN")
        );
        assert_eq!(
            parse_mounted("Mounted /dev/sdb1 at /media/ana/PEN.\n").as_deref(),
            Some("/media/ana/PEN")
        );
        assert_eq!(parse_mounted("Error mounting"), None);
    }

    #[test]
    fn only_unmounted_supported_filesystems_are_mounted() {
        let part = |name: &str, fs: Option<&str>, mounted: bool| usbkeeper_core::Partition {
            name: name.into(),
            path: format!("/dev/{name}"),
            size_bytes: 1,
            fstype: fs.map(String::from),
            label: None,
            mountpoints: if mounted {
                vec!["/media/x".into()]
            } else {
                vec![]
            },
        };
        let parts = [
            part("sdb1", Some("vfat"), false),
            part("sdb2", Some("exfat"), true),
            part("sdb3", Some("crypto_LUKS"), false),
            part("sdb4", None, false),
            part("sdb5", Some("ext4"), false),
        ];
        let chosen: Vec<&str> = mountable(&parts).map(|p| p.name.as_str()).collect();
        assert_eq!(chosen, ["sdb1", "sdb5"]);
    }

    #[test]
    fn finds_mount_source_with_escaped_spaces() {
        let table = "\
/dev/nvme0n1p2 / ext4 rw,relatime 0 0
/dev/sdb1 /media/ana/MEU\\040PEN vfat rw,nosuid 0 0
/dev/mapper/luks-1 /media/ana/cofre ext4 rw 0 0
";
        assert_eq!(
            mount_source(table, "/media/ana/MEU PEN").as_deref(),
            Some("/dev/sdb1")
        );
        assert_eq!(
            mount_source(table, "/media/ana/cofre").as_deref(),
            Some("/dev/mapper/luks-1")
        );
        assert_eq!(mount_source(table, "/media/ana/outro"), None);
    }

    #[test]
    fn under_mountpoint_is_prefix_aware() {
        assert!(is_under("/media/ana/PEN/foto.jpg", "/media/ana/PEN"));
        assert!(is_under("/media/ana/PEN", "/media/ana/PEN"));
        assert!(!is_under("/media/ana/PEN2/x", "/media/ana/PEN"));
    }

    #[test]
    fn finds_processes_holding_files() {
        let proc_root = tempfile::tempdir().unwrap();
        let mk = |pid: &str, comm: &str, cwd: &str, fds: &[&str]| {
            let dir = proc_root.path().join(pid);
            fs::create_dir_all(dir.join("fd")).unwrap();
            fs::write(dir.join("comm"), format!("{comm}\n")).unwrap();
            symlink(cwd, dir.join("cwd")).unwrap();
            for (i, fd) in fds.iter().enumerate() {
                symlink(fd, dir.join("fd").join(i.to_string())).unwrap();
            }
        };
        mk("100", "bash", "/media/ana/PEN/docs", &["/dev/pts/1"]);
        mk("200", "vlc", "/home/ana", &["/media/ana/PEN/filme.mkv"]);
        mk("300", "kate", "/home/ana", &["/media/ana/PEN2/x.txt"]);
        fs::create_dir_all(proc_root.path().join("self")).unwrap();

        let holders = holders_in(proc_root.path(), &["/media/ana/PEN"]);
        let summary: Vec<_> = holders
            .iter()
            .map(|h| (h.pid, h.command.as_str(), h.path.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                (100, "bash", "/media/ana/PEN/docs"),
                (200, "vlc", "/media/ana/PEN/filme.mkv"),
            ]
        );
    }

    #[test]
    fn current_process_is_visible_in_real_proc() {
        let dir = tempfile::tempdir().unwrap();
        let file = fs::File::create(dir.path().join("aberto.txt")).unwrap();
        let mp = dir.path().to_str().unwrap();
        let holders = holders_in(Path::new("/proc"), &[mp]);
        assert!(holders.iter().any(|h| h.pid == std::process::id()));
        drop(file);
    }
}
