//! SMART via `smartctl -j`. A interpretação ([`parse`]) é pura e testada com
//! fixtures; [`read`] executa o smartctl tentando os tipos de ponte USB.
//!
//! Pendrives e cartões quase nunca têm SMART; HDs/SSDs externos têm, mas só
//! se o adaptador USB repassar os comandos (SAT ou protocolo próprio da ponte).

pub mod selftest;

use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum HealthVerdict {
    Good,
    /// Sinais de desgaste/defeito (setores realocados/pendentes, erros...).
    Warning,
    /// O próprio disco declara falha iminente.
    Failing,
    /// Sem SMART (pendrive, ponte USB que não repassa, sem permissão...).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Attribute {
    pub id: u32,
    pub name: String,
    pub value: u32,
    pub worst: u32,
    pub threshold: u32,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub raw: u64,
    /// "now" ou "past" quando o atributo cruzou o limite.
    pub failed: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct HealthReport {
    pub verdict: HealthVerdict,
    /// Tipo usado pelo smartctl (`-d`), ex.: "sat", "nvme", "usbjmicron".
    pub device_type: Option<String>,
    pub protocol: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub passed: Option<bool>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub temperature_c: Option<i64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub power_on_hours: Option<u64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub power_cycles: Option<u64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub reallocated_sectors: Option<u64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub pending_sectors: Option<u64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub uncorrectable_sectors: Option<u64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub crc_errors: Option<u64>,
    /// NVMe: % da vida útil estimada já consumida.
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub percentage_used: Option<u64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub media_errors: Option<u64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub critical_warning: Option<u64>,
    pub attributes: Vec<Attribute>,
    /// Motivos do veredito (ou de não haver SMART), em português.
    pub notes: Vec<String>,
    /// SMART não lido por falta de permissão (não diz nada sobre o disco).
    pub access_denied: bool,
}

impl HealthReport {
    fn unknown(note: String) -> Self {
        Self {
            verdict: HealthVerdict::Unknown,
            device_type: None,
            protocol: None,
            model: None,
            serial: None,
            passed: None,
            temperature_c: None,
            power_on_hours: None,
            power_cycles: None,
            reallocated_sectors: None,
            pending_sectors: None,
            uncorrectable_sectors: None,
            crc_errors: None,
            percentage_used: None,
            media_errors: None,
            critical_warning: None,
            attributes: Vec::new(),
            notes: vec![note],
            access_denied: false,
        }
    }

    pub fn has_smart(&self) -> bool {
        self.verdict != HealthVerdict::Unknown
    }
}

fn u(v: &Value, path: &[&str]) -> Option<u64> {
    path.iter().try_fold(v, |acc, key| acc.get(key))?.as_u64()
}

fn s(v: &Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(v, |acc, key| acc.get(key))?
        .as_str()
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
}

fn messages(v: &Value) -> Vec<String> {
    v.pointer("/smartctl/messages")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("string").and_then(Value::as_str))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn is_permission_error(msgs: &[String]) -> bool {
    msgs.iter().any(|m| {
        let m = m.to_lowercase();
        m.contains("permission denied") || m.contains("operation not permitted")
    })
}

/// Traduz as mensagens mais comuns do smartctl para algo acionável.
fn explain(msgs: &[String]) -> String {
    let all = msgs.join(" | ");
    let lower = all.to_lowercase();
    if is_permission_error(msgs) {
        "sem permissão para ler o SMART; rode com sudo".into()
    } else if lower.contains("unknown usb bridge") || lower.contains("unsupported usb bridge") {
        "o adaptador USB não repassa comandos SMART (ponte desconhecida)".into()
    } else if lower.contains("unable to detect device type") {
        "o smartctl não reconheceu o tipo do dispositivo (comum em pendrives e cartões)".into()
    } else if all.is_empty() {
        "o dispositivo não informou dados SMART".into()
    } else {
        format!("o smartctl não leu SMART: {all}")
    }
}

fn attribute(v: &Value) -> Option<Attribute> {
    Some(Attribute {
        id: v.get("id")?.as_u64()? as u32,
        name: v.get("name")?.as_str()?.to_string(),
        value: v.get("value").and_then(Value::as_u64).unwrap_or(0) as u32,
        worst: v.get("worst").and_then(Value::as_u64).unwrap_or(0) as u32,
        threshold: v.get("thresh").and_then(Value::as_u64).unwrap_or(0) as u32,
        raw: u(v, &["raw", "value"]).unwrap_or(0),
        failed: v
            .get("when_failed")
            .and_then(Value::as_str)
            .filter(|w| !w.is_empty())
            .map(String::from),
    })
}

/// Interpreta a saída JSON do smartctl (`-j -a`).
pub fn parse(json: &str) -> HealthReport {
    let v: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return HealthReport::unknown(format!("saída do smartctl ilegível: {e}")),
    };
    let msgs = messages(&v);
    let ata_table = v
        .pointer("/ata_smart_attributes/table")
        .and_then(Value::as_array);
    let nvme = v.get("nvme_smart_health_information_log");
    let passed = v.pointer("/smart_status/passed").and_then(Value::as_bool);
    if ata_table.is_none() && nvme.is_none() && passed.is_none() {
        let mut r = HealthReport::unknown(explain(&msgs));
        r.access_denied = is_permission_error(&msgs);
        return r;
    }

    let attributes: Vec<Attribute> = ata_table
        .map(|t| t.iter().filter_map(attribute).collect())
        .unwrap_or_default();
    let raw = |id: u32| attributes.iter().find(|a| a.id == id).map(|a| a.raw);
    // Em alguns discos o raw do 194 embute min/máx nos bytes altos.
    let temperature_c = v
        .pointer("/temperature/current")
        .and_then(Value::as_i64)
        .or_else(|| raw(194).map(|r| (r & 0xFF) as i64));

    let mut r = HealthReport {
        verdict: HealthVerdict::Good,
        device_type: s(&v, &["device", "type"]),
        protocol: s(&v, &["device", "protocol"]),
        model: s(&v, &["model_name"]),
        serial: s(&v, &["serial_number"]),
        passed,
        temperature_c,
        power_on_hours: u(&v, &["power_on_time", "hours"]),
        power_cycles: u(&v, &["power_cycle_count"]),
        reallocated_sectors: raw(5),
        pending_sectors: raw(197),
        uncorrectable_sectors: raw(198).or_else(|| raw(187)),
        crc_errors: raw(199),
        percentage_used: nvme.and_then(|n| n.get("percentage_used")?.as_u64()),
        media_errors: nvme.and_then(|n| n.get("media_errors")?.as_u64()),
        critical_warning: nvme.and_then(|n| n.get("critical_warning")?.as_u64()),
        notes: Vec::new(),
        access_denied: false,
        attributes,
    };
    classify(&mut r, nvme);
    r
}

fn classify(r: &mut HealthReport, nvme: Option<&Value>) {
    let mut failing = Vec::new();
    let mut warning = Vec::new();

    if r.passed == Some(false) {
        failing.push("o disco reprovou o autoteste geral do SMART".to_string());
    }
    for a in r
        .attributes
        .iter()
        .filter(|a| a.failed.as_deref() == Some("now"))
    {
        failing.push(format!(
            "atributo {} ({}) abaixo do limite agora",
            a.id, a.name
        ));
    }
    if r.critical_warning.is_some_and(|w| w != 0) {
        failing.push("NVMe com alerta crítico ativo".into());
    }

    let mut count = |value: Option<u64>, what: &str| {
        if let Some(n) = value.filter(|&n| n > 0) {
            warning.push(format!("{n} {what}"));
        }
    };
    count(
        r.reallocated_sectors,
        "setores realocados (defeitos já substituídos)",
    );
    count(
        r.pending_sectors,
        "setores pendentes (ilegíveis, aguardando realocação)",
    );
    count(r.uncorrectable_sectors, "erros incorrigíveis");
    count(r.media_errors, "erros de mídia (NVMe)");
    if r.crc_errors.is_some_and(|n| n > 0) {
        warning.push(format!(
            "{} erros de CRC na interface (costuma ser cabo/adaptador, não o disco)",
            r.crc_errors.unwrap_or(0)
        ));
    }
    if let Some(p) = r.percentage_used.filter(|&p| p >= 90) {
        warning.push(format!("{p}% da vida útil estimada já consumida"));
    }
    if let Some(n) = nvme {
        let spare = n.get("available_spare").and_then(Value::as_u64);
        let threshold = n.get("available_spare_threshold").and_then(Value::as_u64);
        if let (Some(sp), Some(th)) = (spare, threshold)
            && sp <= th
        {
            warning.push(format!("reserva de blocos em {sp}% (limite {th}%)"));
        }
    }
    if r.temperature_c.is_some_and(|t| t >= 60) {
        warning.push(format!(
            "temperatura alta: {} °C",
            r.temperature_c.unwrap_or(0)
        ));
    }

    r.verdict = if !failing.is_empty() {
        HealthVerdict::Failing
    } else if !warning.is_empty() {
        HealthVerdict::Warning
    } else {
        HealthVerdict::Good
    };
    r.notes = failing.into_iter().chain(warning).collect();
}

/// Tipos tentados em ordem. `auto` usa a base de pontes do smartctl; os
/// demais cobrem adaptadores que ela não reconhece.
pub const DEVICE_TYPES: [&str; 5] = ["auto", "sat", "usbjmicron", "usbprolific", "usbsunplus"];

pub fn find_smartctl() -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(["/usr/sbin", "/sbin", "/usr/local/sbin"].map(PathBuf::from))
        .map(|d| {
            d.join(if cfg!(windows) {
                "smartctl.exe"
            } else {
                "smartctl"
            })
        })
        .find(|p| p.is_file())
}

/// Executa o smartctl e devolve o primeiro tipo que trouxer dados SMART; se
/// nenhum trouxer, o relatório `Unknown` com o motivo da primeira tentativa.
pub fn read(device_path: &str) -> HealthReport {
    let Some(smartctl) = find_smartctl() else {
        return HealthReport::unknown(
            "smartctl não encontrado; instale o pacote smartmontools".into(),
        );
    };
    let mut first: Option<HealthReport> = None;
    for kind in DEVICE_TYPES {
        let out = Command::new(&smartctl)
            .args(["-j", "-a", "-d", kind, device_path])
            .output();
        let report = match out {
            Ok(o) => parse(&String::from_utf8_lossy(&o.stdout)),
            Err(e) => HealthReport::unknown(format!("não foi possível executar o smartctl: {e}")),
        };
        if report.has_smart() {
            return report;
        }
        let denied = report.access_denied;
        first.get_or_insert(report);
        if denied {
            break; // não adianta tentar outros tipos sem permissão
        }
    }
    first.expect("ao menos uma tentativa")
}
