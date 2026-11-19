//! `history` (linha do tempo no terminal) e `report` (laudo HTML autocontido).

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_json::Value;
use usbkeeper_core::units::human_bytes;
use usbkeeper_history::{DeviceRecord, Event, History, format_utc, kind};

use crate::backend::Backend;
use crate::store;

/// Aceita o nome atual (`sdb`), a chave do histórico ou o número de série.
fn resolve(b: &Backend, h: &History, query: &str) -> Result<DeviceRecord> {
    if let Ok(device) = b.enumerator.get(query) {
        let key = store::device_info(&device).key();
        return h
            .find_device(&key)?
            .with_context(|| format!("{} ({key}) ainda não tem histórico", device.path));
    }
    h.find_device(query)?.with_context(|| {
        format!("nada no histórico para {query:?} (use o nome, a chave ou a série)")
    })
}

fn kind_label(kind: &str) -> &str {
    match kind {
        kind::FAKE_CHECK => "fake-check",
        kind::BENCH => "velocidade",
        kind::HEALTH => "SMART",
        kind::SCAN => "varredura",
        other => other,
    }
}

fn or_dash(s: &str) -> &str {
    if s.is_empty() { "-" } else { s }
}

fn verdict_label(verdict: Option<&str>) -> &'static str {
    match verdict {
        Some("genuine") => "genuíno",
        Some("fake") => "FALSO",
        Some("damaged") => "defeito",
        Some("good") => "saudável",
        Some("warning") => "atenção",
        Some("failing") => "FALHANDO",
        Some("unknown") => "sem SMART",
        Some("clean") => "sem erros",
        Some("unreadable") => "ILEGÍVEL",
        _ => "",
    }
}

pub fn history(b: &Backend, query: Option<String>, json: bool) -> Result<()> {
    let path = store::db_path().context("sem caminho para o histórico")?;
    if !path.exists() {
        bail!(
            "ainda não há histórico ({}); ele é criado por fake-check, bench e health",
            path.display()
        );
    }
    let h = store::open()?;
    match query {
        None => {
            let devices = h.devices()?;
            if json {
                return crate::print_json(&devices);
            }
            if devices.is_empty() {
                println!("Histórico vazio.");
            }
            for d in devices {
                println!(
                    "{}  {}  {}",
                    d.device.key,
                    d.device.model.as_deref().unwrap_or("-"),
                    human_bytes(d.device.advertised_bytes)
                );
                println!(
                    "    {} {}, visto por último em {}; fake-check: {}; saúde: {}",
                    d.events,
                    if d.events == 1 { "evento" } else { "eventos" },
                    format_utc(d.device.last_seen),
                    or_dash(verdict_label(d.last_fake_verdict.as_deref())),
                    or_dash(verdict_label(d.last_health_verdict.as_deref()))
                );
            }
            println!("\nArquivo: {}", path.display());
        }
        Some(q) => {
            let device = resolve(b, &h, &q)?;
            let events = h.events(&device.key)?;
            if json {
                return crate::print_json(
                    &serde_json::json!({ "device": device, "events": events }),
                );
            }
            println!(
                "{}  {}  {}",
                device.key,
                device.model.as_deref().unwrap_or("-"),
                human_bytes(device.advertised_bytes)
            );
            for e in &events {
                println!(
                    "  {}  {:<10} {:<10} {}",
                    format_utc(e.at),
                    kind_label(&e.kind),
                    verdict_label(e.verdict.as_deref()),
                    e.summary
                );
            }
        }
    }
    Ok(())
}

pub fn report(b: &Backend, query: String, output: Option<PathBuf>) -> Result<()> {
    let h = store::open()?;
    let device = resolve(b, &h, &query)?;
    let events = h.events(&device.key)?;
    let html = render(&device, &events, usbkeeper_history::now());
    let path = output.unwrap_or_else(|| {
        let safe: String = device
            .key
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        PathBuf::from(format!("usbkeeper-{safe}.html"))
    });
    std::fs::write(&path, html).with_context(|| format!("gravando {}", path.display()))?;
    store::give_back(&path);
    println!(
        "Laudo salvo em {}\nAbra no navegador; para PDF use Imprimir → Salvar como PDF.",
        path.display()
    );
    Ok(())
}

// ------------------------------------------------------------------- HTML

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn tone(verdict: Option<&str>) -> &'static str {
    match verdict {
        Some("genuine" | "good" | "clean") => "ok",
        Some("warning" | "damaged") => "warn",
        Some("fake" | "failing" | "unreadable") => "bad",
        _ => "muted",
    }
}

fn last<'a>(events: &'a [Event], k: &str) -> Option<&'a Event> {
    events.iter().rev().find(|e| e.kind == k)
}

fn bytes(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_u64).map(human_bytes)
}

fn card(title: &str, event: Option<&Event>, body: String) -> String {
    match event {
        None => format!(
            r#"<section class="card"><h2>{}</h2><p class="muted">Nenhum teste registrado.</p></section>"#,
            esc(title)
        ),
        Some(e) => format!(
            r#"<section class="card"><h2>{}</h2><p><span class="badge {}">{}</span> <span class="muted">{}</span></p>{body}</section>"#,
            esc(title),
            tone(e.verdict.as_deref()),
            esc(verdict_label(e.verdict.as_deref()).trim()),
            esc(&format_utc(e.at)),
        ),
    }
}

fn fake_body(e: &Event) -> String {
    let d = &e.data;
    let mut rows = vec![
        ("Anunciado", bytes(d, "advertised_bytes")),
        ("Capacidade real", bytes(d, "real_bytes")),
        ("Dá a volta a cada", bytes(d, "wrap_module_bytes")),
        ("1º setor ruim", bytes(d, "first_bad_offset")),
        (
            "Modo",
            d.get("mode").and_then(Value::as_str).map(|m| {
                if m == "full" {
                    "completo (todos os setores)"
                } else {
                    "rápido (amostragem)"
                }
                .to_string()
            }),
        ),
    ];
    if let Some(w) = d.get("write_mbps").and_then(Value::as_f64) {
        rows.push(("Escrita", Some(format!("{w:.1} MB/s"))));
    }
    table(&rows)
}

fn bench_body(e: &Event) -> String {
    let d = &e.data;
    let f = |k: &str, unit: &str, digits: usize| {
        d.get(k)
            .and_then(Value::as_f64)
            .map(|x| format!("{x:.digits$} {unit}"))
    };
    table(&[
        ("Leitura sequencial", f("seq_read_mbps", "MB/s", 1)),
        ("Leitura 4K", f("rand_read_iops", "IOPS", 0)),
        ("Escrita sequencial", f("seq_write_mbps", "MB/s", 1)),
        ("Escrita 4K", f("rand_write_iops", "IOPS", 0)),
    ])
}

fn health_body(smart: Option<&Event>, scan: Option<&Event>) -> String {
    let mut out = String::new();
    if let Some(e) = smart {
        if let Some(notes) = e.data.get("notes").and_then(Value::as_array)
            && !notes.is_empty()
        {
            out.push_str("<ul>");
            for n in notes.iter().filter_map(Value::as_str) {
                out.push_str(&format!("<li>{}</li>", esc(n)));
            }
            out.push_str("</ul>");
        }
        let n = |k: &str| e.data.get(k).and_then(Value::as_u64).map(|x| x.to_string());
        out.push_str(&table(&[
            (
                "Temperatura",
                e.data
                    .get("temperature_c")
                    .and_then(Value::as_i64)
                    .map(|t| format!("{t} °C")),
            ),
            ("Horas ligado", n("power_on_hours")),
            ("Setores realocados", n("reallocated_sectors")),
            ("Setores pendentes", n("pending_sectors")),
            (
                "Vida útil usada (NVMe)",
                n("percentage_used").map(|p| format!("{p}%")),
            ),
        ]));
        if let Some(attrs) = e.data.get("attributes").and_then(Value::as_array)
            && !attrs.is_empty()
        {
            out.push_str("<details><summary>Atributos SMART</summary><table><thead><tr><th>ID</th><th>Nome</th><th>Valor</th><th>Pior</th><th>Limite</th><th>Bruto</th></tr></thead><tbody>");
            for a in attrs {
                let g = |k: &str| {
                    a.get(k)
                        .map(|v| v.to_string().trim_matches('"').to_string())
                        .unwrap_or_default()
                };
                out.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    esc(&g("id")),
                    esc(&g("name")),
                    esc(&g("value")),
                    esc(&g("worst")),
                    esc(&g("threshold")),
                    esc(&g("raw"))
                ));
            }
            out.push_str("</tbody></table></details>");
        }
    }
    if let Some(e) = scan {
        out.push_str(&format!(
            r#"<p>Varredura de superfície ({}): <span class="badge {}">{}</span> {}</p>"#,
            esc(&format_utc(e.at)),
            tone(e.verdict.as_deref()),
            esc(verdict_label(e.verdict.as_deref())),
            esc(&e.summary)
        ));
    }
    out
}

fn table(rows: &[(&str, Option<String>)]) -> String {
    let body: String = rows
        .iter()
        .filter_map(|(k, v)| {
            v.as_ref()
                .map(|v| format!("<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(v)))
        })
        .collect();
    if body.is_empty() {
        String::new()
    } else {
        format!(r#"<table class="kv">{body}</table>"#)
    }
}

pub fn render(device: &DeviceRecord, events: &[Event], generated_at: i64) -> String {
    let fake = last(events, kind::FAKE_CHECK);
    let bench = last(events, kind::BENCH);
    let smart = last(events, kind::HEALTH);
    let scan = last(events, kind::SCAN);
    let health_event = match (smart, scan) {
        (Some(a), Some(b)) => Some(if a.at >= b.at { a } else { b }),
        (a, b) => a.or(b),
    };
    let ident = table(&[
        ("Modelo", device.model.clone()),
        ("Número de série", device.serial.clone()),
        (
            "USB VID:PID",
            device
                .vendor_id
                .as_ref()
                .map(|v| format!("{v}:{}", device.product_id.as_deref().unwrap_or("-"))),
        ),
        (
            "Capacidade anunciada",
            Some(human_bytes(device.advertised_bytes)),
        ),
        ("Primeiro registro", Some(format_utc(device.first_seen))),
        ("Último registro", Some(format_utc(device.last_seen))),
    ]);
    let timeline: String = events
        .iter()
        .rev()
        .map(|e| {
            format!(
                r#"<tr><td>{}</td><td>{}</td><td><span class="badge {}">{}</span></td><td>{}</td></tr>"#,
                esc(&format_utc(e.at)),
                esc(kind_label(&e.kind)),
                tone(e.verdict.as_deref()),
                esc(verdict_label(e.verdict.as_deref())),
                esc(&e.summary)
            )
        })
        .collect();
    let headline = match fake.and_then(|e| e.verdict.as_deref()) {
        Some("fake") => format!(
            "Dispositivo FALSIFICADO: {} reais de {} anunciados",
            fake.and_then(|e| bytes(&e.data, "real_bytes"))
                .unwrap_or_default(),
            human_bytes(device.advertised_bytes)
        ),
        Some("genuine") => "Capacidade verificada: genuína".into(),
        Some("damaged") => "Capacidade correta, mas com setores defeituosos".into(),
        _ => "Laudo do dispositivo".into(),
    };

    format!(
        r#"<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Laudo usbkeeper</title>
<style>
:root {{ --bg:#f7f7f5; --fg:#1c1c1a; --muted:#6b6b66; --card:#fff; --line:#e3e3df;
  --ok:#1f7a4d; --ok-bg:#e3f4ea; --warn:#8a5a00; --warn-bg:#fcf0d6; --bad:#b42318; --bad-bg:#fde7e4; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg:#141413; --fg:#ecebe6; --muted:#9c9b95; --card:#1d1d1b; --line:#2e2e2b;
  --ok:#6fd39b; --ok-bg:#16301f; --warn:#f2c46b; --warn-bg:#33280f; --bad:#ff8a7a; --bad-bg:#3a1a16; }} }}
* {{ box-sizing:border-box }}
body {{ margin:0; background:var(--bg); color:var(--fg); font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif; }}
main {{ max-width:880px; margin:0 auto; padding:32px 16px 48px; }}
h1 {{ font-size:1.6rem; margin:0 0 4px; }} h2 {{ font-size:1.05rem; margin:0 0 8px; }}
.muted {{ color:var(--muted); }}
.grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(260px,1fr)); gap:16px; margin:24px 0; }}
.card {{ background:var(--card); border:1px solid var(--line); border-radius:10px; padding:16px; }}
.badge {{ display:inline-block; padding:1px 8px; border-radius:999px; font-size:.82rem; font-weight:600; }}
.ok {{ color:var(--ok); background:var(--ok-bg); }} .warn {{ color:var(--warn); background:var(--warn-bg); }}
.bad {{ color:var(--bad); background:var(--bad-bg); }} .badge.muted {{ background:var(--line); }}
table {{ width:100%; border-collapse:collapse; font-size:.92rem; }}
th, td {{ text-align:left; padding:6px 8px; border-bottom:1px solid var(--line); vertical-align:top; }}
table.kv th {{ width:45%; font-weight:500; color:var(--muted); }}
.timeline {{ overflow-x:auto; }}
details {{ margin-top:8px; }} summary {{ cursor:pointer; color:var(--muted); }}
footer {{ margin-top:32px; font-size:.85rem; }}
@media print {{ body {{ background:#fff; color:#000; }} .card {{ break-inside:avoid; }} }}
</style>
</head>
<body><main>
<h1>{headline}</h1>
<p class="muted">{model} · chave <code>{key}</code></p>
<div class="grid">
<section class="card"><h2>Identificação</h2>{ident}</section>
{fake_card}
{health_card}
{bench_card}
</div>
<section class="card timeline"><h2>Histórico completo</h2>
<table><thead><tr><th>Quando</th><th>Teste</th><th>Resultado</th><th>Detalhes</th></tr></thead><tbody>{timeline}</tbody></table>
</section>
<footer class="muted">Gerado por usbkeeper {version} em {generated}. Os testes de capacidade gravam padrões únicos por setor e conferem a leitura; o modo completo cobre todos os setores.</footer>
</main></body></html>
"#,
        model = esc(device.model.as_deref().unwrap_or("dispositivo")),
        key = esc(&device.key),
        headline = esc(&headline),
        fake_card = card(
            "Capacidade (fake-check)",
            fake,
            fake.map(fake_body).unwrap_or_default()
        ),
        health_card = card("Saúde", health_event, health_body(smart, scan)),
        bench_card = card(
            "Velocidade",
            bench,
            bench.map(bench_body).unwrap_or_default()
        ),
        version = env!("CARGO_PKG_VERSION"),
        generated = esc(&format_utc(generated_at)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn device() -> DeviceRecord {
        DeviceRecord {
            key: "090c:1000:<script>".into(),
            serial: Some("<script>alert(1)</script>".into()),
            vendor_id: Some("090c".into()),
            product_id: Some("1000".into()),
            model: Some("Pen \"128GB\" & cia".into()),
            advertised_bytes: 128_000_000_000,
            first_seen: 100,
            last_seen: 200,
        }
    }

    fn event(kind: &str, verdict: &str, at: i64, data: Value) -> Event {
        Event {
            id: at,
            at,
            kind: kind.into(),
            verdict: Some(verdict.into()),
            summary: format!("resumo {kind}"),
            data,
        }
    }

    #[test]
    fn fake_report_headline_and_escaping() {
        let events = vec![
            event(
                kind::FAKE_CHECK,
                "fake",
                100,
                json!({"advertised_bytes": 128_000_000_000u64, "real_bytes": 8_000_000_000u64, "mode": "full"}),
            ),
            event(
                kind::HEALTH,
                "unknown",
                150,
                json!({"notes": ["sem <b>SMART</b>"]}),
            ),
            event(kind::SCAN, "clean", 160, json!({})),
            event(
                kind::BENCH,
                "",
                200,
                json!({"seq_read_mbps": 21.5, "rand_read_iops": 900.0}),
            ),
        ];
        let html = render(&device(), &events, 300);
        assert!(html.contains("Dispositivo FALSIFICADO: 8.0 GB reais de 128.0 GB anunciados"));
        assert!(html.contains("completo (todos os setores)"));
        assert!(html.contains("21.5 MB/s"));
        assert!(
            !html.contains("<script>"),
            "texto do dispositivo deve ser escapado"
        );
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("sem &lt;b&gt;SMART&lt;/b&gt;"));
        assert!(html.contains("Pen &quot;128GB&quot; &amp; cia"));
    }

    #[test]
    fn empty_history_still_renders() {
        let html = render(&device(), &[], 300);
        assert!(html.contains("Laudo do dispositivo"));
        assert!(html.contains("Nenhum teste registrado."));
    }
}
