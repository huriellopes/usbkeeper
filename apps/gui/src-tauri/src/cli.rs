//! Tradução de pedidos da interface em argumentos da CLI e leitura do stream
//! `--progress-json` (puro, sem processos).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum OpRequest {
    FakeCheck {
        device: String,
        full: bool,
    },
    Health {
        device: String,
        scan: bool,
    },
    Bench {
        device: String,
        write: bool,
    },
    Format {
        device: String,
        filesystem: String,
        table: Option<String>,
        label: Option<String>,
        #[ts(type = "number | null")]
        max_bytes: Option<u64>,
    },
    Eject {
        device: String,
    },
    Repair {
        device: String,
        check: bool,
    },
    /// "selftest" (não "self_test"), igual ao nome da operação na CLI e no histórico.
    #[serde(rename = "selftest")]
    SelfTest {
        device: String,
        long: bool,
    },
    Wipe {
        device: String,
        random: bool,
        passes: u8,
        verify: bool,
        discard: bool,
    },
    ImageCreate {
        device: String,
        output: String,
    },
    ImageRestore {
        device: String,
        image: String,
    },
    BurnIso {
        device: String,
        iso: String,
        sha256: Option<String>,
    },
}

impl OpRequest {
    pub fn device(&self) -> &str {
        match self {
            OpRequest::FakeCheck { device, .. }
            | OpRequest::Health { device, .. }
            | OpRequest::Bench { device, .. }
            | OpRequest::Format { device, .. }
            | OpRequest::Eject { device }
            | OpRequest::Repair { device, .. }
            | OpRequest::SelfTest { device, .. }
            | OpRequest::Wipe { device, .. }
            | OpRequest::ImageCreate { device, .. }
            | OpRequest::ImageRestore { device, .. }
            | OpRequest::BurnIso { device, .. } => device,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            OpRequest::FakeCheck { .. } => "fake_check",
            OpRequest::Health { .. } => "health",
            OpRequest::Bench { .. } => "bench",
            OpRequest::Format { .. } => "format",
            OpRequest::Eject { .. } => "eject",
            OpRequest::Repair { .. } => "repair",
            OpRequest::SelfTest { .. } => "selftest",
            OpRequest::Wipe { .. } => "wipe",
            OpRequest::ImageCreate { .. } => "image_create",
            OpRequest::ImageRestore { .. } => "image_restore",
            OpRequest::BurnIso { .. } => "burn_iso",
        }
    }

    /// Eject usa udisks e não precisa de root.
    pub fn needs_root(&self) -> bool {
        !matches!(self, OpRequest::Eject { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OpProgress {
    pub phase: String,
    pub label: String,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub bytes: bool,
}

pub fn args(req: &OpRequest, token: &str, allow_non_usb: bool) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let confirm = |a: &mut Vec<String>| {
        a.extend([
            "--unmount".to_string(),
            "--yes-i-know".to_string(),
            token.to_string(),
        ]);
    };
    match req {
        OpRequest::FakeCheck { device, full } => {
            a.extend(["fake-check".into(), device.clone()]);
            if *full {
                a.push("--full".into());
            }
            confirm(&mut a);
        }
        OpRequest::Health { device, scan } => {
            a.extend(["health".into(), device.clone()]);
            if *scan {
                a.push("--scan".into());
            }
        }
        OpRequest::Bench { device, write } => {
            a.extend(["bench".into(), device.clone()]);
            if *write {
                a.push("--write".into());
                confirm(&mut a);
            }
        }
        OpRequest::Format {
            device,
            filesystem,
            table,
            label,
            max_bytes,
        } => {
            a.extend([
                "format".into(),
                device.clone(),
                "--fs".into(),
                filesystem.clone(),
            ]);
            if let Some(t) = table {
                a.extend(["--table".into(), t.clone()]);
            }
            if let Some(l) = label.as_deref().filter(|l| !l.trim().is_empty()) {
                a.extend(["-l".into(), l.to_string()]);
            }
            if let Some(m) = max_bytes {
                a.extend(["--max-size".into(), m.to_string()]);
            }
            confirm(&mut a);
        }
        OpRequest::Repair { device, check } => {
            a.extend(["repair".into(), device.clone()]);
            if *check {
                // Verificar não altera nada: só desmonta, sem pedir a série.
                a.extend(["--check".into(), "--unmount".into()]);
            } else {
                confirm(&mut a);
            }
        }
        OpRequest::SelfTest { device, long } => {
            // Só lê: nunca pede a série.
            a.extend([
                "selftest".into(),
                device.clone(),
                if *long { "--longo" } else { "--curto" }.into(),
                "--esperar".into(),
            ]);
        }
        OpRequest::Wipe {
            device,
            random,
            passes,
            verify,
            discard,
        } => {
            a.extend(["wipe".into(), device.clone()]);
            if *discard {
                a.push("--descartar".into());
            } else {
                if *random {
                    a.push("--aleatorio".into());
                }
                a.extend(["--passadas".into(), passes.clamp(&1, &3).to_string()]);
                if *verify {
                    a.push("--verificar".into());
                }
            }
            confirm(&mut a);
        }
        OpRequest::ImageCreate { device, output } => {
            // Só lê o disco; o diálogo de salvar já perguntou sobre sobrescrever.
            a.extend([
                "image".into(),
                "create".into(),
                device.clone(),
                "-o".into(),
                output.clone(),
                "--sobrescrever".into(),
            ]);
        }
        OpRequest::ImageRestore { device, image } => {
            a.extend([
                "image".into(),
                "restore".into(),
                image.clone(),
                device.clone(),
            ]);
            confirm(&mut a);
        }
        OpRequest::BurnIso {
            device,
            iso,
            sha256,
        } => {
            a.extend(["burn-iso".into(), iso.clone(), device.clone()]);
            if let Some(h) = sha256.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
                a.extend(["--sha256".into(), h.to_string()]);
            }
            confirm(&mut a);
        }
        OpRequest::Eject { device } => {
            // --progress-json: erro em JSON com os processos que prendem o disco.
            a.extend([
                "eject".into(),
                device.clone(),
                "--json".into(),
                "--progress-json".into(),
            ]);
            return a;
        }
    }
    if allow_non_usb {
        a.push("--allow-non-usb".into());
    }
    a.extend([
        "--json".into(),
        "--progress-json".into(),
        "--cancel-stdin".into(),
    ]);
    a
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamLine {
    Progress(OpProgress),
    Error {
        message: String,
        holders: Vec<usbkeeper_core::Holder>,
    },
    Log(String),
}

/// `Holder` do core só deriva Serialize; este é o espelho para ler o JSON.
#[derive(Deserialize)]
struct HolderIn {
    pid: u32,
    command: String,
    path: String,
}

pub fn parse_line(line: &str) -> StreamLine {
    let t = line.trim();
    if t.starts_with('{') {
        if let Ok(p) = serde_json::from_str::<OpProgress>(t) {
            return StreamLine::Progress(p);
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(t)
            && let Some(e) = v.get("error").and_then(|e| e.as_str())
        {
            let holders = v
                .get("holders")
                .and_then(|h| serde_json::from_value::<Vec<HolderIn>>(h.clone()).ok())
                .unwrap_or_default()
                .into_iter()
                .map(|h| usbkeeper_core::Holder {
                    pid: h.pid,
                    command: h.command,
                    path: h.path,
                })
                .collect();
            return StreamLine::Error {
                message: e.to_string(),
                holders,
            };
        }
    }
    StreamLine::Log(t.to_string())
}

/// Mensagem de erro final; `None` = sucesso.
pub fn exit_message(code: Option<i32>, cli_error: Option<&str>) -> Option<String> {
    match code {
        Some(0) => None,
        Some(126) => Some("autorização cancelada".into()),
        Some(127) => Some("autorização negada (ou pkexec ausente)".into()),
        _ => Some(
            cli_error
                .map(String::from)
                .unwrap_or_else(|| "a operação falhou".into()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    const M: [&str; 3] = ["--json", "--progress-json", "--cancel-stdin"];

    #[test]
    fn fake_check_quick_and_full() {
        let a = args(
            &OpRequest::FakeCheck {
                device: "/dev/sdb".into(),
                full: false,
            },
            "ABC",
            false,
        );
        assert_eq!(
            a,
            [
                s(&["fake-check", "/dev/sdb", "--unmount", "--yes-i-know", "ABC"]),
                s(&M)
            ]
            .concat()
        );
        let a = args(
            &OpRequest::FakeCheck {
                device: "/dev/sdb".into(),
                full: true,
            },
            "ABC",
            true,
        );
        assert!(a.contains(&"--full".to_string()) && a.contains(&"--allow-non-usb".to_string()));
    }

    #[test]
    fn read_only_ops_never_confirm() {
        let a = args(
            &OpRequest::Health {
                device: "/dev/sdb".into(),
                scan: true,
            },
            "ABC",
            false,
        );
        assert_eq!(a, [s(&["health", "/dev/sdb", "--scan"]), s(&M)].concat());
        let a = args(
            &OpRequest::Bench {
                device: "/dev/sdb".into(),
                write: false,
            },
            "ABC",
            false,
        );
        assert!(!a.iter().any(|x| x == "--yes-i-know"));
    }

    #[test]
    fn destructive_bench_and_format_confirm() {
        let a = args(
            &OpRequest::Bench {
                device: "/dev/sdb".into(),
                write: true,
            },
            "ABC",
            false,
        );
        assert!(a.windows(2).any(|w| w == ["--yes-i-know", "ABC"]));
        let a = args(
            &OpRequest::Format {
                device: "/dev/sdb".into(),
                filesystem: "exfat".into(),
                table: None,
                label: Some("Fotos 2026".into()),
                max_bytes: Some(7_000_000_000),
            },
            "ABC",
            false,
        );
        assert_eq!(a[..4], s(&["format", "/dev/sdb", "--fs", "exfat"]));
        assert!(a.windows(2).any(|w| w == ["-l", "Fotos 2026"]));
        assert!(a.windows(2).any(|w| w == ["--max-size", "7000000000"]));
        assert!(a.contains(&"--unmount".to_string()));
    }

    #[test]
    fn repair_check_never_confirms_and_repair_does() {
        let a = args(
            &OpRequest::Repair {
                device: "/dev/sdb".into(),
                check: true,
            },
            "ABC",
            false,
        );
        assert_eq!(
            a,
            s(&[
                "repair",
                "/dev/sdb",
                "--check",
                "--unmount",
                "--json",
                "--progress-json",
                "--cancel-stdin"
            ])
        );
        let a = args(
            &OpRequest::Repair {
                device: "/dev/sdb".into(),
                check: false,
            },
            "ABC",
            false,
        );
        assert!(a.windows(2).any(|w| w == ["--yes-i-know", "ABC"]));
        assert!(!a.contains(&"--check".to_string()));
    }

    #[test]
    fn selftest_and_wipe_args() {
        let a = args(
            &OpRequest::SelfTest {
                device: "/dev/sda".into(),
                long: false,
            },
            "ABC",
            false,
        );
        assert_eq!(
            a,
            s(&[
                "selftest",
                "/dev/sda",
                "--curto",
                "--esperar",
                "--json",
                "--progress-json",
                "--cancel-stdin"
            ])
        );
        let a = args(
            &OpRequest::Wipe {
                device: "/dev/sdb".into(),
                random: true,
                passes: 9,
                verify: true,
                discard: false,
            },
            "ABC",
            false,
        );
        assert!(a.contains(&"--aleatorio".to_string()) && a.contains(&"--verificar".to_string()));
        assert!(
            a.windows(2).any(|w| w == ["--passadas", "3"]),
            "passadas limitadas a 3"
        );
        assert!(a.windows(2).any(|w| w == ["--yes-i-know", "ABC"]));
        let d = args(
            &OpRequest::Wipe {
                device: "/dev/sdb".into(),
                random: true,
                passes: 2,
                verify: true,
                discard: true,
            },
            "ABC",
            false,
        );
        assert!(d.contains(&"--descartar".to_string()));
        assert!(!d.contains(&"--aleatorio".to_string()) && !d.contains(&"--passadas".to_string()));
    }

    #[test]
    fn image_and_iso_args() {
        let a = args(
            &OpRequest::ImageCreate {
                device: "/dev/sdb".into(),
                output: "/home/ana/pen.img.zst".into(),
            },
            "ABC",
            false,
        );
        assert_eq!(
            a[..6],
            s(&[
                "image",
                "create",
                "/dev/sdb",
                "-o",
                "/home/ana/pen.img.zst",
                "--sobrescrever"
            ])
        );
        assert!(
            !a.contains(&"--yes-i-know".to_string()),
            "criar imagem só lê"
        );
        let a = args(
            &OpRequest::ImageRestore {
                device: "/dev/sdb".into(),
                image: "/home/ana/pen.img.zst".into(),
            },
            "ABC",
            false,
        );
        assert_eq!(
            a[..4],
            s(&["image", "restore", "/home/ana/pen.img.zst", "/dev/sdb"])
        );
        assert!(a.windows(2).any(|w| w == ["--yes-i-know", "ABC"]));
        let a = args(
            &OpRequest::BurnIso {
                device: "/dev/sdb".into(),
                iso: "/home/ana/x.iso".into(),
                sha256: Some("  ".into()),
            },
            "ABC",
            false,
        );
        assert!(!a.contains(&"--sha256".to_string()), "hash vazio não vai");
        assert!(a.windows(2).any(|w| w == ["--yes-i-know", "ABC"]));
    }

    #[test]
    fn eject_asks_for_json_errors() {
        let a = args(
            &OpRequest::Eject {
                device: "/dev/sdb".into(),
            },
            "ABC",
            false,
        );
        assert_eq!(a, s(&["eject", "/dev/sdb", "--json", "--progress-json"]));
    }

    #[test]
    fn error_line_keeps_holders() {
        match parse_line(
            r#"{"error":"em uso","holders":[{"pid":7,"command":"vlc","path":"/m/x"}]}"#,
        ) {
            StreamLine::Error { message, holders } => {
                assert_eq!(message, "em uso");
                assert_eq!(holders[0].pid, 7);
                assert_eq!(holders[0].command, "vlc");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_stream_lines() {
        match parse_line(r#"{"phase":"probe","label":"x","done":3,"total":9,"bytes":false}"#) {
            StreamLine::Progress(p) => {
                assert_eq!((p.phase.as_str(), p.done, p.total), ("probe", 3, 9))
            }
            other => panic!("{other:?}"),
        }
        assert!(
            matches!(parse_line(r#"{"error":"cancelado"}"#), StreamLine::Error { message, holders } if message == "cancelado" && holders.is_empty())
        );
        assert!(matches!(parse_line("Desmontando..."), StreamLine::Log(_)));
        assert!(matches!(parse_line("{quebrado"), StreamLine::Log(_)));
    }

    #[test]
    fn pkexec_exit_codes() {
        assert_eq!(exit_message(Some(0), None), None);
        assert!(exit_message(Some(126), None).unwrap().contains("cancelada"));
        assert!(exit_message(Some(127), None).unwrap().contains("negada"));
        assert_eq!(
            exit_message(Some(1), Some("disco montado")).as_deref(),
            Some("disco montado")
        );
    }
}
