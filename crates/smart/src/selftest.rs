//! Autoteste SMART: o próprio disco se testa no firmware (curto ~2 min,
//! longo lê a superfície inteira). Não altera dados. Leitura do estado pura
//! (`parse_state`, testada com fixtures) e execução com o smartctl.

use std::process::Command;

use serde::Serialize;
use serde_json::Value;

use crate::find_smartctl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfTestKind {
    Short,
    Long,
}

impl SelfTestKind {
    fn arg(self) -> &'static str {
        match self {
            SelfTestKind::Short => "short",
            SelfTestKind::Long => "long",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SelfTestEntry {
    /// Tipo como o disco descreve ("Short offline", "Extended offline", "Short"…).
    pub kind: String,
    pub status: String,
    pub passed: Option<bool>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub hours: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SelfTestState {
    pub running: bool,
    pub remaining_percent: Option<u8>,
    /// Duração estimada do teste pedido, em minutos (só ATA informa).
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub polling_minutes: Option<u64>,
    /// Log do disco, do mais recente para o mais antigo.
    pub log: Vec<SelfTestEntry>,
    /// Status do último autoteste informado pelo disco (ATA), quando não
    /// está rodando: segunda fonte quando o log é ambíguo.
    pub last_passed: Option<bool>,
}

/// Resultado do autoteste que acabou de rodar, comparando com o log de
/// antes. A entrada nova pode ser idêntica à anterior (mesmo tipo e mesma
/// hora de uso), então também conta o log que cresceu ou se deslocou. Sem
/// entrada nova reconhecível, usa o status que o disco informa.
pub fn new_result(before: &[SelfTestEntry], now: &SelfTestState) -> Option<SelfTestEntry> {
    let log = &now.log;
    let shifted =
        log.len() >= 2 && before.len() >= log.len() - 1 && log[1..] == before[..log.len() - 1];
    let added =
        !log.is_empty() && (log.len() > before.len() || log.first() != before.first() || shifted);
    if added {
        return log.first().cloned();
    }
    now.last_passed.map(|passed| SelfTestEntry {
        kind: "autoteste".into(),
        status: if passed {
            "Completed without error".into()
        } else {
            "falhou (status informado pelo disco)".into()
        },
        passed: Some(passed),
        hours: None,
    })
}

fn str_at(v: &Value, path: &str) -> Option<String> {
    v.pointer(path).and_then(Value::as_str).map(String::from)
}

fn parse_ata(v: &Value, kind: SelfTestKind) -> Option<SelfTestState> {
    let status = v.pointer("/ata_smart_data/self_test/status")?;
    let value = status.get("value").and_then(Value::as_u64).unwrap_or(0);
    // 0xF0..=0xF9: em andamento; o nibble baixo é o restante em dezenas de %.
    let running = (240..=249).contains(&value);
    let remaining = status
        .get("remaining_percent")
        .and_then(Value::as_u64)
        .map(|p| p as u8)
        .or_else(|| running.then(|| ((value & 0x0F) * 10) as u8));
    let polling = v
        .pointer(match kind {
            SelfTestKind::Short => "/ata_smart_data/self_test/polling_minutes/short",
            SelfTestKind::Long => "/ata_smart_data/self_test/polling_minutes/extended",
        })
        .and_then(Value::as_u64);
    let log = v
        .pointer("/ata_smart_self_test_log/standard/table")
        .and_then(Value::as_array)
        .map(|t| {
            t.iter()
                .map(|e| SelfTestEntry {
                    kind: str_at(e, "/type/string").unwrap_or_else(|| "?".into()),
                    status: str_at(e, "/status/string").unwrap_or_default(),
                    passed: e.pointer("/status/passed").and_then(Value::as_bool),
                    hours: e.get("lifetime_hours").and_then(Value::as_u64),
                })
                .collect()
        })
        .unwrap_or_default();
    let last_passed = if running {
        None
    } else {
        status.get("passed").and_then(Value::as_bool)
    };
    Some(SelfTestState {
        running,
        remaining_percent: running.then_some(remaining).flatten(),
        polling_minutes: polling,
        log,
        last_passed,
    })
}

fn parse_nvme(v: &Value) -> Option<SelfTestState> {
    let log_root = v.get("nvme_self_test_log")?;
    let running = log_root
        .pointer("/current_self_test_operation/value")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        != 0;
    let done = log_root
        .get("current_self_test_completion_percent")
        .and_then(Value::as_u64);
    let log = log_root
        .get("table")
        .and_then(Value::as_array)
        .map(|t| {
            t.iter()
                .map(|e| SelfTestEntry {
                    kind: str_at(e, "/self_test_code/string").unwrap_or_else(|| "?".into()),
                    status: str_at(e, "/self_test_result/string").unwrap_or_default(),
                    passed: e
                        .pointer("/self_test_result/value")
                        .and_then(Value::as_u64)
                        .map(|r| r == 0),
                    hours: e.get("power_on_hours").and_then(Value::as_u64),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(SelfTestState {
        running,
        remaining_percent: running.then(|| 100u8.saturating_sub(done.unwrap_or(0) as u8)),
        polling_minutes: None,
        log,
        last_passed: None,
    })
}

/// Estado a partir de `smartctl -j -c -l selftest`.
pub fn parse_state(json: &str, kind: SelfTestKind) -> Result<SelfTestState, String> {
    let v: Value =
        serde_json::from_str(json).map_err(|e| format!("saída do smartctl ilegível: {e}"))?;
    parse_ata(&v, kind).or_else(|| parse_nvme(&v)).ok_or_else(|| {
        "o disco não informou o estado do autoteste (ponte USB sem suporte, ou pendrive sem SMART)"
            .to_string()
    })
}

fn smartctl() -> Result<std::path::PathBuf, String> {
    find_smartctl().ok_or_else(|| crate::NOT_INSTALLED.into())
}

fn first_message(v: &Value) -> Option<String> {
    v.pointer("/smartctl/messages/0/string")
        .and_then(Value::as_str)
        .map(String::from)
}

/// Dispara o autoteste no firmware do disco. `dtype` é o tipo (`-d`) que
/// trouxe o SMART (ver `read`).
pub fn start(device: &str, dtype: &str, kind: SelfTestKind) -> Result<(), String> {
    let out = Command::new(smartctl()?)
        .args([
            "-j",
            "-d",
            dtype,
            "-t",
            kind.arg(),
            &crate::smartctl_device(device),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    let exit = v
        .pointer("/smartctl/exit_status")
        .and_then(Value::as_u64)
        .unwrap_or(u64::from(!out.status.success()));
    // bits 0-1: comando inválido / não foi possível abrir ou enviar o pedido
    if exit & 0b11 != 0 || (exit & 0b100 != 0) {
        return Err(first_message(&v)
            .unwrap_or_else(|| "o disco recusou o autoteste (ponte USB sem suporte?)".into()));
    }
    Ok(())
}

pub fn poll(device: &str, dtype: &str, kind: SelfTestKind) -> Result<SelfTestState, String> {
    let out = Command::new(smartctl()?)
        .args([
            "-j",
            "-d",
            dtype,
            "-c",
            "-l",
            "selftest",
            &crate::smartctl_device(device),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    parse_state(&String::from_utf8_lossy(&out.stdout), kind)
}

pub fn abort(device: &str, dtype: &str) -> Result<(), String> {
    let out = Command::new(smartctl()?)
        .args(["-d", dtype, "-X", &crate::smartctl_device(device)])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}
