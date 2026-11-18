//! Histórico em SQLite: cada dispositivo (por VID:PID + número de série) e
//! os eventos registrados para ele (fake-check, bench, saúde, varredura).

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;

pub use rusqlite::Error;
pub type Result<T> = rusqlite::Result<T>;

const SCHEMA_VERSION: i64 = 1;

/// O que o histórico precisa saber de um dispositivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub model: Option<String>,
    pub advertised_bytes: u64,
}

impl DeviceInfo {
    /// Chave estável. Pendrives baratos repetem a mesma série entre unidades,
    /// então VID:PID entram na chave; sem série, o tamanho anunciado a substitui.
    pub fn key(&self) -> String {
        // Nunca começa com "-": a chave é passada como argumento de linha de
        // comando (`history <chave>`), e o parser a confundiria com uma opção.
        let origin = match (&self.vendor_id, &self.product_id) {
            (Some(vid), Some(pid)) => format!("{vid}:{pid}"),
            _ => "sem-usb".to_string(),
        };
        match self
            .serial
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(serial) => format!("{origin}:{serial}"),
            None => format!("{origin}:sem-serie:{}", self.advertised_bytes),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceRecord {
    pub key: String,
    pub serial: Option<String>,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub model: Option<String>,
    pub advertised_bytes: u64,
    pub first_seen: i64,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceSummary {
    #[serde(flatten)]
    pub device: DeviceRecord,
    pub events: u64,
    /// Último veredito de fake-check, se houver.
    pub last_fake_verdict: Option<String>,
    /// Último veredito de saúde (SMART ou varredura).
    pub last_health_verdict: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: i64,
    pub at: i64,
    pub kind: String,
    pub verdict: Option<String>,
    pub summary: String,
    pub data: Value,
}

pub mod kind {
    pub const FAKE_CHECK: &str = "fake_check";
    pub const BENCH: &str = "bench";
    pub const HEALTH: &str = "health";
    pub const SCAN: &str = "scan";
}

pub struct History {
    conn: Connection,
}

impl History {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < 1 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS devices (
                    id INTEGER PRIMARY KEY,
                    key TEXT NOT NULL UNIQUE,
                    serial TEXT,
                    vendor_id TEXT,
                    product_id TEXT,
                    model TEXT,
                    advertised_bytes INTEGER NOT NULL,
                    first_seen INTEGER NOT NULL,
                    last_seen INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS events (
                    id INTEGER PRIMARY KEY,
                    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
                    at INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    verdict TEXT,
                    summary TEXT NOT NULL,
                    data TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS events_by_device ON events(device_id, at);",
            )?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Self { conn })
    }

    /// Registra um evento (criando ou atualizando o dispositivo). `at` em
    /// segundos Unix.
    pub fn record(
        &mut self,
        device: &DeviceInfo,
        kind: &str,
        verdict: Option<&str>,
        summary: &str,
        data: &Value,
        at: i64,
    ) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO devices (key, serial, vendor_id, product_id, model, advertised_bytes, first_seen, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(key) DO UPDATE SET
                last_seen = max(last_seen, excluded.last_seen),
                model = coalesce(excluded.model, model),
                advertised_bytes = excluded.advertised_bytes",
            params![
                device.key(),
                device.serial,
                device.vendor_id,
                device.product_id,
                device.model,
                device.advertised_bytes as i64,
                at
            ],
        )?;
        let device_id: i64 = tx.query_row(
            "SELECT id FROM devices WHERE key = ?1",
            [device.key()],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO events (device_id, at, kind, verdict, summary, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![device_id, at, kind, verdict, summary, data.to_string()],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    fn device_from_row(r: &rusqlite::Row<'_>) -> Result<DeviceRecord> {
        Ok(DeviceRecord {
            key: r.get("key")?,
            serial: r.get("serial")?,
            vendor_id: r.get("vendor_id")?,
            product_id: r.get("product_id")?,
            model: r.get("model")?,
            advertised_bytes: r.get::<_, i64>("advertised_bytes")? as u64,
            first_seen: r.get("first_seen")?,
            last_seen: r.get("last_seen")?,
        })
    }

    pub fn devices(&self) -> Result<Vec<DeviceSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.*,
                (SELECT count(*) FROM events e WHERE e.device_id = d.id) AS n,
                (SELECT verdict FROM events e WHERE e.device_id = d.id AND e.kind = 'fake_check'
                    ORDER BY at DESC, id DESC LIMIT 1) AS fake,
                (SELECT verdict FROM events e WHERE e.device_id = d.id AND e.kind IN ('health', 'scan')
                    ORDER BY at DESC, id DESC LIMIT 1) AS health
             FROM devices d ORDER BY d.last_seen DESC",
        )?;
        stmt.query_map([], |r| {
            Ok(DeviceSummary {
                device: Self::device_from_row(r)?,
                events: r.get::<_, i64>("n")? as u64,
                last_fake_verdict: r.get("fake")?,
                last_health_verdict: r.get("health")?,
            })
        })?
        .collect()
    }

    /// Busca por chave exata ou número de série.
    pub fn find_device(&self, query: &str) -> Result<Option<DeviceRecord>> {
        self.conn
            .query_row(
                "SELECT * FROM devices WHERE key = ?1 OR serial = ?1 ORDER BY last_seen DESC LIMIT 1",
                [query],
                Self::device_from_row,
            )
            .optional()
    }

    /// Eventos do dispositivo, do mais antigo para o mais recente.
    pub fn events(&self, key: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.at, e.kind, e.verdict, e.summary, e.data
             FROM events e JOIN devices d ON d.id = e.device_id
             WHERE d.key = ?1 ORDER BY e.at, e.id",
        )?;
        stmt.query_map([key], |r| {
            let data: String = r.get(5)?;
            Ok(Event {
                id: r.get(0)?,
                at: r.get(1)?,
                kind: r.get(2)?,
                verdict: r.get(3)?,
                summary: r.get(4)?,
                data: serde_json::from_str(&data).unwrap_or(Value::Null),
            })
        })?
        .collect()
    }
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// "2026-11-18 14:57 UTC" sem depender de crate de datas
/// (algoritmo civil_from_days de Howard Hinnant).
pub fn format_utc(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pen(serial: Option<&str>) -> DeviceInfo {
        DeviceInfo {
            serial: serial.map(String::from),
            vendor_id: Some("0951".into()),
            product_id: Some("1666".into()),
            model: Some("Kingston DataTraveler 3.0".into()),
            advertised_bytes: 64_000_000_000,
        }
    }

    #[test]
    fn keys() {
        assert_eq!(pen(Some("ABC")).key(), "0951:1666:ABC");
        assert_eq!(pen(Some("  ")).key(), "0951:1666:sem-serie:64000000000");
        let mut sata = pen(Some("WD-1"));
        sata.vendor_id = None;
        sata.product_id = None;
        assert_eq!(sata.key(), "sem-usb:WD-1");
        sata.serial = None;
        assert_eq!(sata.key(), "sem-usb:sem-serie:64000000000");
        assert!(!sata.key().starts_with('-'));
    }

    #[test]
    fn records_and_summarizes() {
        let mut h = History::open_in_memory().unwrap();
        let d = pen(Some("ABC"));
        h.record(
            &d,
            kind::FAKE_CHECK,
            Some("fake"),
            "8 GB reais de 64",
            &json!({"real_bytes": 8}),
            100,
        )
        .unwrap();
        h.record(&d, kind::BENCH, None, "leitura 30 MB/s", &json!({}), 200)
            .unwrap();
        h.record(
            &d,
            kind::HEALTH,
            Some("unknown"),
            "sem SMART",
            &json!({}),
            300,
        )
        .unwrap();
        h.record(
            &pen(Some("XYZ")),
            kind::FAKE_CHECK,
            Some("genuine"),
            "ok",
            &json!({}),
            50,
        )
        .unwrap();

        let all = h.devices().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].device.key, "0951:1666:ABC"); // visto por último
        assert_eq!(all[0].events, 3);
        assert_eq!(all[0].last_fake_verdict.as_deref(), Some("fake"));
        assert_eq!(all[0].last_health_verdict.as_deref(), Some("unknown"));
        assert_eq!(all[0].device.first_seen, 100);
        assert_eq!(all[0].device.last_seen, 300);

        let ev = h.events("0951:1666:ABC").unwrap();
        assert_eq!(
            ev.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
            ["fake_check", "bench", "health"]
        );
        assert_eq!(ev[0].data["real_bytes"], 8);

        assert_eq!(h.find_device("XYZ").unwrap().unwrap().key, "0951:1666:XYZ");
        assert!(h.find_device("nada").unwrap().is_none());
    }

    #[test]
    fn reopening_keeps_data_and_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("h.db");
        {
            let mut h = History::open(&path).unwrap();
            h.record(
                &pen(Some("A")),
                kind::SCAN,
                Some("clean"),
                "ok",
                &json!({}),
                1,
            )
            .unwrap();
        }
        let h = History::open(&path).unwrap();
        assert_eq!(h.devices().unwrap().len(), 1);
        // No Windows um arquivo aberto não pode ser apagado: fecha antes do
        // TempDir limpar o diretório.
        drop(h);
    }

    #[test]
    fn formats_dates() {
        assert_eq!(format_utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(format_utc(1_795_014_816), "2026-11-18 15:13 UTC");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00 UTC");
    }
}
