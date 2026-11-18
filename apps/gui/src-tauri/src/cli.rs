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
}

impl OpRequest {
    pub fn device(&self) -> &str {
        match self {
            OpRequest::FakeCheck { device, .. }
            | OpRequest::Health { device, .. }
            | OpRequest::Bench { device, .. }
            | OpRequest::Format { device, .. }
            | OpRequest::Eject { device } => device,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            OpRequest::FakeCheck { .. } => "fake_check",
            OpRequest::Health { .. } => "health",
            OpRequest::Bench { .. } => "bench",
            OpRequest::Format { .. } => "format",
            OpRequest::Eject { .. } => "eject",
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
        OpRequest::Eject { device } => {
            a.extend(["eject".into(), device.clone(), "--json".into()]);
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
    Error(String),
    Log(String),
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
            return StreamLine::Error(e.to_string());
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
    fn eject_has_no_cancel_stdin() {
        let a = args(
            &OpRequest::Eject {
                device: "/dev/sdb".into(),
            },
            "ABC",
            false,
        );
        assert_eq!(a, s(&["eject", "/dev/sdb", "--json"]));
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
            matches!(parse_line(r#"{"error":"cancelado"}"#), StreamLine::Error(e) if e == "cancelado")
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
