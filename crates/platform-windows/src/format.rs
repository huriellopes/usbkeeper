//! Formatar no Windows: planos de PowerShell (Storage), exibidos no
//! `--dry-run`, e o FAT32 próprio do núcleo acima de 32 GiB, onde o Windows
//! se recusa.

use usbkeeper_core::format::{
    Filesystem, FormatRequest, PARTITION_START, PartitionTable, partition_sectors,
};
use usbkeeper_core::{CoreError, Device, Step};

/// Acima disso o `Format-Volume` recusa FAT32.
pub const WINDOWS_FAT32_LIMIT: u64 = 32 << 30;

/// Subcomando oculto da CLI que grava o FAT32 próprio na partição de dados.
pub const FAT32_SUBCOMMAND: &str = "__fat32";

/// Partição de dados: a última que não é reservada/sistema/recuperação.
pub const DATA_PARTITION: &str = "Get-Partition -DiskNumber {n} | Where-Object { $_.Type -notin 'Reserved','System','Recovery','Unknown' } | Sort-Object Offset | Select-Object -Last 1";

fn ps(description: impl Into<String>, script: &str) -> Step {
    let full =
        format!("$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; {script}");
    Step::new(
        description,
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &full,
        ],
    )
}

/// Texto entre aspas simples do PowerShell (`'` vira `''`).
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// `self_exe`: a própria CLI, para o passo do FAT32 acima de 32 GiB.
pub fn plan(
    n: u32,
    device: &Device,
    req: &FormatRequest,
    self_exe: &str,
) -> Result<Vec<Step>, CoreError> {
    let fs_name = match req.filesystem {
        Filesystem::Fat32 => "FAT32",
        Filesystem::Exfat => "exFAT",
        Filesystem::Ntfs => "NTFS",
        Filesystem::Ext4 => {
            return Err(CoreError::Backend(
                "o Windows não formata nem lê ext4: para trocar arquivos com o Linux use exFAT"
                    .into(),
            ));
        }
    };
    let style = match req.table {
        PartitionTable::Gpt => "GPT",
        PartitionTable::Mbr => "MBR",
    };
    let (size_args, data_bytes) = match req.max_bytes {
        Some(max) => {
            let bytes =
                partition_sectors(device.size_bytes, max).map_err(CoreError::Backend)? * 512;
            (format!("-Offset {PARTITION_START} -Size {bytes}"), bytes)
        }
        None => (
            "-UseMaximumSize".to_string(),
            device.size_bytes.saturating_sub(PARTITION_START),
        ),
    };
    let mbr_type = match (req.table, req.filesystem) {
        (PartitionTable::Mbr, Filesystem::Fat32) => " -MbrType FAT32",
        (PartitionTable::Mbr, _) => " -MbrType IFS",
        _ => "",
    };
    let data = DATA_PARTITION.replace("{n}", &n.to_string());
    let mut steps = vec![
        ps(
            "Apagando a tabela de partições",
            &format!(
                "if ((Get-Disk -Number {n}).PartitionStyle -ne 'RAW') {{ Clear-Disk -Number {n} -RemoveData -RemoveOEM -Confirm:$false }}"
            ),
        ),
        ps(
            format!("Criando tabela {style}"),
            &format!("Initialize-Disk -Number {n} -PartitionStyle {style}"),
        ),
        // Sem letra até formatar: senão o Explorer oferece "formatar o disco"
        // enquanto a partição ainda está crua.
        ps(
            "Criando a partição",
            &format!("New-Partition -DiskNumber {n} {size_args}{mbr_type} | Out-Null"),
        ),
    ];
    let big_fat32 = req.filesystem == Filesystem::Fat32 && data_bytes > WINDOWS_FAT32_LIMIT;
    if big_fat32 {
        let mut s = Step::new(
            "Formatando em FAT32 (formatador do usbkeeper: o Windows recusa acima de 32 GB)",
            self_exe,
            &[FAT32_SUBCOMMAND, &device.path],
        );
        if let Some(l) = &req.label {
            s.args.extend(["--label".to_string(), l.clone()]);
        }
        steps.push(s);
    } else {
        let label = req
            .label
            .as_deref()
            .map(|l| format!(" -NewFileSystemLabel {}", quote(l)))
            .unwrap_or_default();
        steps.push(ps(
            format!("Formatando em {fs_name}"),
            &format!("{data} | Format-Volume -FileSystem {fs_name}{label} -Confirm:$false -Force | Out-Null"),
        ));
    }
    steps.push(ps(
        "Atribuindo uma letra",
        &format!(
            "$p = {data}; if (-not $p.DriveLetter) {{ $p | Add-PartitionAccessPath -AssignDriveLetter }}"
        ),
    ));
    Ok(steps)
}

/// Offset e tamanho da partição de dados a partir do JSON `{Offset, Size}`.
pub fn parse_offset_size(json: &str) -> Result<(u64, u64), CoreError> {
    let v: serde_json::Value = serde_json::from_str(json.trim())
        .map_err(|e| CoreError::Backend(format!("partição de dados ilegível: {e}")))?;
    let get = |k: &str| v.get(k).and_then(serde_json::Value::as_u64);
    match (get("Offset"), get("Size")) {
        (Some(o), Some(s)) if s > 0 => Ok((o, s)),
        _ => Err(CoreError::Backend(
            "partição de dados não encontrada".into(),
        )),
    }
}

#[cfg(windows)]
mod run {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    use usbkeeper_core::format::FormatRequest;
    use usbkeeper_core::{CoreError, Device, Formatter, Step};

    use super::{DATA_PARTITION, parse_offset_size, plan};
    use crate::paths::disk_number;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub struct WindowsFormatter {
        /// Caminho da própria CLI (passo do FAT32 acima de 32 GiB).
        pub self_exe: String,
    }

    fn disk(device: &Device) -> Result<u32, CoreError> {
        disk_number(&device.path)
            .ok_or_else(|| CoreError::Backend(format!("caminho inesperado: {}", device.path)))
    }

    pub fn run_step(step: &Step) -> Result<(), CoreError> {
        let out = Command::new(&step.program)
            .args(&step.args)
            .stdin(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .output()?;
        if out.status.success() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&out.stderr).into_owned()
            + &String::from_utf8_lossy(&out.stdout);
        let first = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("erro desconhecido");
        Err(CoreError::CommandFailed {
            command: step.description.clone(),
            stderr: first.to_string(),
        })
    }

    impl Formatter for WindowsFormatter {
        fn plan(&self, device: &Device, request: &FormatRequest) -> Result<Vec<Step>, CoreError> {
            plan(disk(device)?, device, request, &self.self_exe)
        }

        fn execute(&self, steps: &[Step]) -> Result<(), CoreError> {
            steps.iter().try_for_each(run_step)
        }
    }

    /// Offset e tamanho da partição de dados do disco `n` (para o FAT32 próprio).
    pub fn data_partition(n: u32) -> Result<(u64, u64), CoreError> {
        let script = format!(
            "$ErrorActionPreference='Stop'; {} | Select-Object Offset, Size | ConvertTo-Json -Compress",
            DATA_PARTITION.replace("{n}", &n.to_string())
        );
        let out = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()?;
        parse_offset_size(&String::from_utf8_lossy(&out.stdout))
    }
}

#[cfg(windows)]
pub use run::{WindowsFormatter, data_partition};

#[cfg(test)]
mod tests {
    use super::*;
    use usbkeeper_core::Transport;

    fn dev(size: u64) -> Device {
        Device {
            name: "PhysicalDrive1".into(),
            path: r"\\.\PhysicalDrive1".into(),
            size_bytes: size,
            transport: Transport::Usb,
            removable: true,
            hotplug: true,
            read_only: false,
            vendor: None,
            model: None,
            serial: Some("ABC".into()),
            partition_table: None,
            logical_sector: 512,
            physical_sector: 512,
            system_disk: false,
            mountpoints: vec![],
            partitions: vec![],
            usb: None,
        }
    }

    fn req(
        fs: Filesystem,
        table: PartitionTable,
        label: Option<&str>,
        max: Option<u64>,
    ) -> FormatRequest {
        FormatRequest {
            filesystem: fs,
            table,
            label: label.map(String::from),
            max_bytes: max,
        }
    }

    fn script(s: &Step) -> &str {
        s.args.last().unwrap()
    }

    const GIB: u64 = 1 << 30;

    #[test]
    fn small_fat32_on_mbr_uses_windows_format() {
        let steps = plan(
            1,
            &dev(16 * GIB),
            &req(
                Filesystem::Fat32,
                PartitionTable::Mbr,
                Some("MEU PEN"),
                None,
            ),
            "usbkeeper.exe",
        )
        .unwrap();
        assert_eq!(steps.len(), 5);
        assert!(steps.iter().all(|s| s.program == "powershell.exe"));
        assert!(script(&steps[0]).contains("Clear-Disk -Number 1"));
        assert!(script(&steps[1]).contains("Initialize-Disk -Number 1 -PartitionStyle MBR"));
        assert!(script(&steps[2]).contains("-UseMaximumSize -MbrType FAT32"));
        assert!(
            !script(&steps[2]).contains("AssignDriveLetter"),
            "letra só depois de formatar"
        );
        assert!(script(&steps[4]).contains("Add-PartitionAccessPath -AssignDriveLetter"));
        assert!(
            script(&steps[3])
                .contains("Format-Volume -FileSystem FAT32 -NewFileSystemLabel 'MEU PEN'")
        );
    }

    #[test]
    fn big_fat32_uses_own_formatter() {
        let steps = plan(
            2,
            &dev(64 * GIB),
            &req(Filesystem::Fat32, PartitionTable::Mbr, Some("GRANDE"), None),
            r"C:\bin\usbkeeper.exe",
        )
        .unwrap();
        let last = &steps[steps.len() - 2];
        assert_eq!(last.program, r"C:\bin\usbkeeper.exe");
        assert_eq!(
            last.args,
            vec![FAT32_SUBCOMMAND, r"\\.\PhysicalDrive1", "--label", "GRANDE"]
        );
        assert!(!steps.iter().any(|s| script(s).contains("Format-Volume")));
    }

    #[test]
    fn gpt_exfat_label_quoting_and_max_size() {
        let steps = plan(
            3,
            &dev(128 * GIB),
            &req(Filesystem::Exfat, PartitionTable::Gpt, Some("D'Ana"), None),
            "x",
        )
        .unwrap();
        assert!(
            !script(&steps[2]).contains("MbrType"),
            "GPT não leva MbrType"
        );
        assert!(script(&steps[3]).contains("-FileSystem exFAT -NewFileSystemLabel 'D''Ana'"));
        let steps = plan(
            3,
            &dev(64 * GIB),
            &req(Filesystem::Ntfs, PartitionTable::Mbr, None, Some(8 * GIB)),
            "x",
        )
        .unwrap();
        let s = script(&steps[2]);
        assert!(
            s.contains("-Offset 1048576 -Size ") && s.contains("-MbrType IFS"),
            "{s}"
        );
        assert!(!script(&steps[3]).contains("NewFileSystemLabel"));
    }

    #[test]
    fn fat32_with_max_size_under_limit_stays_windows() {
        // Falso de "64 GB" com 8 GB reais: partição de ~8 GiB, formata pelo Windows.
        let steps = plan(
            1,
            &dev(64 * GIB),
            &req(Filesystem::Fat32, PartitionTable::Mbr, None, Some(8 * GIB)),
            "x",
        )
        .unwrap();
        assert!(script(&steps[3]).contains("Format-Volume -FileSystem FAT32"));
    }

    #[test]
    fn ext4_is_refused() {
        let err = plan(
            1,
            &dev(GIB),
            &req(Filesystem::Ext4, PartitionTable::Gpt, None, None),
            "x",
        )
        .unwrap_err();
        assert!(err.to_string().contains("exFAT"));
    }

    #[test]
    fn offset_size_json() {
        assert_eq!(
            parse_offset_size(r#"{"Offset":1048576,"Size":42949672960}"#).unwrap(),
            (1 << 20, 40 * GIB)
        );
        assert!(parse_offset_size("").is_err());
        assert!(parse_offset_size(r#"{"Offset":1048576,"Size":0}"#).is_err());
    }
}
