//! Formatação no Linux: wipefs → sfdisk → mkfs.*, como um plano de comandos.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use usbkeeper_core::format::{
    Filesystem, FormatRequest, PARTITION_START, PartitionTable, partition_path, partition_sectors,
};
use usbkeeper_core::{CoreError, Device, Formatter, Step};

/// Tipo GPT "Microsoft basic data" (FAT/exFAT/NTFS) e "Linux filesystem".
const GPT_BASIC_DATA: &str = "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7";
const GPT_LINUX: &str = "0FC63DAF-8483-4772-8E79-3D69D8477DE4";

const SBIN_DIRS: [&str; 4] = ["/usr/local/sbin", "/usr/sbin", "/sbin", "/usr/bin"];

/// Procura no PATH e também nos sbin (o PATH de usuário comum não os inclui).
pub fn find_tool(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(SBIN_DIRS.iter().map(PathBuf::from))
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

fn package_for(tool: &str) -> &'static str {
    match tool {
        "mkfs.vfat" => "dosfstools",
        "mkfs.exfat" => "exfatprogs",
        "mkfs.ntfs" => "ntfs-3g",
        "mkfs.ext4" => "e2fsprogs",
        "udevadm" => "udev",
        _ => "util-linux",
    }
}

fn mkfs_tool(fs: Filesystem) -> &'static str {
    match fs {
        Filesystem::Fat32 => "mkfs.vfat",
        Filesystem::Exfat => "mkfs.exfat",
        Filesystem::Ntfs => "mkfs.ntfs",
        Filesystem::Ext4 => "mkfs.ext4",
    }
}

/// Script do sfdisk: uma única partição ocupando o disco todo
/// (alinhada em 1 MiB pelo próprio sfdisk).
/// Com `size_sectors`, a partição começa em 1 MiB e tem esse tamanho
/// (usado para limitar à capacidade real de um falso).
pub fn sfdisk_script(table: PartitionTable, fs: Filesystem, size_sectors: Option<u64>) -> String {
    let (label, kind) = match (table, fs) {
        (PartitionTable::Mbr, Filesystem::Fat32) => ("dos", "c"), // W95 FAT32 (LBA)
        (PartitionTable::Mbr, Filesystem::Exfat | Filesystem::Ntfs) => ("dos", "7"),
        (PartitionTable::Mbr, Filesystem::Ext4) => ("dos", "83"),
        (PartitionTable::Gpt, Filesystem::Ext4) => ("gpt", GPT_LINUX),
        (PartitionTable::Gpt, _) => ("gpt", GPT_BASIC_DATA),
    };
    match size_sectors {
        None => format!("label: {label}\ntype={kind}\n"),
        Some(n) => format!(
            "label: {label}\nstart={}, size={n}, type={kind}\n",
            PARTITION_START / 512
        ),
    }
}

fn mkfs_args(fs: Filesystem, label: Option<&str>, part: &str) -> Vec<String> {
    let mut args: Vec<&str> = match fs {
        Filesystem::Fat32 => vec!["-F", "32"],
        Filesystem::Exfat => vec![],
        Filesystem::Ntfs => vec!["-Q"],
        Filesystem::Ext4 => vec!["-F", "-q"],
    };
    if let Some(label) = label {
        args.push(match fs {
            Filesystem::Fat32 => "-n",
            _ => "-L",
        });
        args.push(label);
    }
    args.push(part);
    args.into_iter().map(String::from).collect()
}

pub struct LinuxFormatter {
    /// Resolve o caminho de uma ferramenta; trocável nos testes.
    pub locate: fn(&str) -> Option<PathBuf>,
}

impl Default for LinuxFormatter {
    fn default() -> Self {
        Self { locate: find_tool }
    }
}

impl LinuxFormatter {
    fn tool(&self, name: &str) -> Result<String, CoreError> {
        (self.locate)(name)
            .map(|p| p.display().to_string())
            .ok_or_else(|| CoreError::MissingTool {
                tool: name.to_string(),
                package: package_for(name).to_string(),
            })
    }
}

impl Formatter for LinuxFormatter {
    fn plan(&self, device: &Device, request: &FormatRequest) -> Result<Vec<Step>, CoreError> {
        // Todas as ferramentas são resolvidas antes: se faltar alguma, nada é apagado.
        let wipefs = self.tool("wipefs")?;
        let sfdisk = self.tool("sfdisk")?;
        let mkfs = self.tool(mkfs_tool(request.filesystem))?;
        let settle = self.tool("udevadm").ok();
        let size_sectors = request
            .max_bytes
            .map(|max| partition_sectors(device.size_bytes, max))
            .transpose()
            .map_err(CoreError::Backend)?;

        let mut steps = Vec::new();
        for p in &device.partitions {
            steps.push(Step::new(
                format!("apagar assinaturas de {}", p.path),
                &wipefs,
                &["-a", "-q", &p.path],
            ));
        }
        steps.push(Step::new(
            format!("apagar assinaturas de {}", device.path),
            &wipefs,
            &["-a", "-q", &device.path],
        ));
        steps.push(
            Step::new(
                format!("criar tabela {} com uma partição", request.table),
                &sfdisk,
                &[
                    "--quiet",
                    "--wipe",
                    "always",
                    "--wipe-partitions",
                    "always",
                    &device.path,
                ],
            )
            .with_stdin(sfdisk_script(
                request.table,
                request.filesystem,
                size_sectors,
            )),
        );
        if let Some(udevadm) = &settle {
            steps.push(Step::new(
                "aguardar o kernel criar a partição",
                udevadm,
                &["settle"],
            ));
        }
        let part = partition_path(&device.path, 1);
        let args = mkfs_args(request.filesystem, request.label.as_deref(), &part);
        steps.push(Step {
            description: format!("criar sistema de arquivos {}", request.filesystem),
            program: mkfs,
            args,
            stdin: None,
        });
        if let Some(udevadm) = &settle {
            steps.push(Step::new("atualizar dispositivos", udevadm, &["settle"]));
        }
        Ok(steps)
    }

    fn execute(&self, steps: &[Step]) -> Result<(), CoreError> {
        steps.iter().try_for_each(run_step)
    }
}

pub fn run_step(step: &Step) -> Result<(), CoreError> {
    let mut child = Command::new(Path::new(&step.program))
        .args(&step.args)
        .stdin(if step.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    if let (Some(input), Some(mut stdin)) = (&step.stdin, child.stdin.take()) {
        stdin.write_all(input.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(CoreError::CommandFailed {
            command: step.command_line(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usbkeeper_core::{Partition, Transport};

    fn fake_locate(name: &str) -> Option<PathBuf> {
        Some(PathBuf::from(format!("/sbin/{name}")))
    }

    fn device(name: &str, parts: &[&str]) -> Device {
        Device {
            name: name.into(),
            path: format!("/dev/{name}"),
            size_bytes: 16_000_000_000,
            transport: Transport::Usb,
            removable: true,
            hotplug: true,
            read_only: false,
            vendor: None,
            model: None,
            serial: Some("X".into()),
            partition_table: Some("dos".into()),
            logical_sector: 512,
            physical_sector: 512,
            system_disk: false,
            mountpoints: vec![],
            partitions: parts
                .iter()
                .map(|p| Partition {
                    name: p.to_string(),
                    path: format!("/dev/{p}"),
                    size_bytes: 1,
                    fstype: None,
                    label: None,
                    mountpoints: vec![],
                })
                .collect(),
            usb: None,
        }
    }

    fn lines(steps: &[Step]) -> Vec<String> {
        steps.iter().map(Step::command_line).collect()
    }

    #[test]
    fn fat32_mbr_plan() {
        let f = LinuxFormatter {
            locate: fake_locate,
        };
        let req = FormatRequest {
            filesystem: Filesystem::Fat32,
            table: PartitionTable::Mbr,
            label: Some("PEN".into()),
            max_bytes: None,
        };
        let steps = f.plan(&device("sdb", &["sdb1", "sdb2"]), &req).unwrap();
        assert_eq!(
            lines(&steps),
            [
                "/sbin/wipefs -a -q /dev/sdb1",
                "/sbin/wipefs -a -q /dev/sdb2",
                "/sbin/wipefs -a -q /dev/sdb",
                "/sbin/sfdisk --quiet --wipe always --wipe-partitions always /dev/sdb  <<< \"label: dos\\ntype=c\\n\"",
                "/sbin/udevadm settle",
                "/sbin/mkfs.vfat -F 32 -n PEN /dev/sdb1",
                "/sbin/udevadm settle",
            ]
        );
    }

    #[test]
    fn gpt_ext4_on_nvme_style_name() {
        let f = LinuxFormatter {
            locate: fake_locate,
        };
        let req = FormatRequest {
            filesystem: Filesystem::Ext4,
            table: PartitionTable::Gpt,
            label: None,
            max_bytes: None,
        };
        let steps = f.plan(&device("mmcblk0", &[]), &req).unwrap();
        assert_eq!(
            steps[1].stdin.as_deref(),
            Some("label: gpt\ntype=0FC63DAF-8483-4772-8E79-3D69D8477DE4\n")
        );
        assert_eq!(
            steps.iter().rev().nth(1).unwrap().command_line(),
            "/sbin/mkfs.ext4 -F -q /dev/mmcblk0p1"
        );
    }

    #[test]
    fn exfat_and_ntfs_use_capital_l_label() {
        assert_eq!(
            mkfs_args(Filesystem::Exfat, Some("Fotos 2026"), "/dev/sdb1"),
            ["-L", "Fotos 2026", "/dev/sdb1"]
        );
        assert_eq!(
            mkfs_args(Filesystem::Ntfs, Some("HD"), "/dev/sdb1"),
            ["-Q", "-L", "HD", "/dev/sdb1"]
        );
        assert_eq!(
            sfdisk_script(PartitionTable::Mbr, Filesystem::Exfat, None),
            "label: dos\ntype=7\n"
        );
    }

    #[test]
    fn max_size_limits_partition_in_plan() {
        let f = LinuxFormatter {
            locate: fake_locate,
        };
        let mut dev = device("sdb", &[]);
        dev.size_bytes = 64_000_000_000;
        let req = FormatRequest {
            filesystem: Filesystem::Exfat,
            table: PartitionTable::Mbr,
            label: None,
            max_bytes: Some(7_800 << 20),
        };
        let steps = f.plan(&dev, &req).unwrap();
        assert_eq!(
            steps[1].stdin.as_deref(),
            Some(format!("label: dos\nstart=2048, size={}, type=7\n", 7_799 * 2048).as_str())
        );
        req_too_big(&f, &dev);
    }

    fn req_too_big(f: &LinuxFormatter, dev: &Device) {
        let req = FormatRequest {
            filesystem: Filesystem::Exfat,
            table: PartitionTable::Mbr,
            label: None,
            max_bytes: Some(dev.size_bytes),
        };
        assert!(matches!(f.plan(dev, &req), Err(CoreError::Backend(_))));
    }

    #[test]
    fn missing_mkfs_aborts_before_any_step() {
        fn no_exfat(name: &str) -> Option<PathBuf> {
            (name != "mkfs.exfat").then(|| PathBuf::from(format!("/sbin/{name}")))
        }
        let f = LinuxFormatter { locate: no_exfat };
        let req = FormatRequest {
            filesystem: Filesystem::Exfat,
            table: PartitionTable::Mbr,
            label: None,
            max_bytes: None,
        };
        match f.plan(&device("sdb", &[]), &req) {
            Err(CoreError::MissingTool { tool, package }) => {
                assert_eq!(tool, "mkfs.exfat");
                assert_eq!(package, "exfatprogs");
            }
            other => panic!("esperava MissingTool, veio {other:?}"),
        }
    }

    #[test]
    fn run_step_reports_failure_with_stderr() {
        let step = Step::new("falha", "/bin/sh", &["-c", "echo boom >&2; exit 3"]);
        match run_step(&step) {
            Err(CoreError::CommandFailed { stderr, .. }) => assert_eq!(stderr, "boom"),
            other => panic!("esperava CommandFailed, veio {other:?}"),
        }
        let ok = Step::new("ok", "/bin/sh", &["-c", "read x; test \"$x\" = oi"]).with_stdin("oi\n");
        assert!(run_step(&ok).is_ok());
    }
}
