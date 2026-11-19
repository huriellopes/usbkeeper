# M5b — Guia de ajuda e remoção segura Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Guia didático (12 tópicos) mostrado igual na CLI (`usbkeeper guia`) e na interface (painel Ajuda + botões “?”), e remoção segura em destaque, protegida e explicada.

**Architecture:** Conteúdo em Markdown num crate `usbkeeper-guide` (embutido com `include_str!`), com renderização para HTML (`pulldown-cmark`, sem HTML bruto) e para terminal. A CLI ganha `guia`, apelido `remover`, erro JSON com os processos que prendem o disco e sucesso parcial quando só desmonta. A GUI busca os tópicos uma vez, abre o painel de ajuda por “?” e trata remoção com cartão “em uso” e aviso “pode tirar”.

**Tech Stack:** Rust 1.99, pulldown-cmark 0.13, clap 4, Tauri 2, React 19 + TS, zustand, vitest.

Spec: `docs/superpowers/specs/2026-11-18-m5b-guia-e-remocao-design.md`.

## Global Constraints

- Um único conteúdo: `crates/guide/topics/<id>.md`; primeira linha `# Título`, segunda linha não vazia = resumo.
- Ids estáveis: `comecar, tipos, usb, mbr-gpt, sistemas-de-arquivos, formatar, falsificados, saude, velocidade, remover, protecoes, glossario`.
- Apelidos: `mbr`,`gpt`→`mbr-gpt`; `fat32`,`exfat`,`ntfs`,`ext4`,`fs`→`sistemas-de-arquivos`; `fake`,`falso`→`falsificados`; `smart`→`saude`; `ejetar`,`eject`→`remover`.
- HTML bruto no Markdown é descartado na renderização.
- Textos em pt-BR, didáticos, frases curtas, sem jargão sem explicação.
- Aviso de sucesso da remoção some em 15 s ou ao clicar.
- Commits sem coautoria do Claude.

## File Structure

```
crates/guide/Cargo.toml, src/lib.rs, src/terminal.rs, topics/*.md (12)
apps/cli/src/main.rs (guia, alias remover), apps/cli/src/destructive.rs (BusyError, eject parcial, dicas),
apps/cli/src/checks.rs, apps/cli/src/health.rs (dicas)
apps/gui/src-tauri/src/guide.rs (guide_topics), cli.rs (eject args, erro com holders), ops.rs (OpFinished.holders), lib.rs
apps/gui/src/lib/guideLinks.ts, lib/api.ts, lib/store.ts
apps/gui/src/features/help/{HelpPanel,HelpButton}.tsx (+ testes)
apps/gui/src/features/remove/{RemoveButton,InUseCard,Notice}.tsx (+ testes)
apps/gui/src/features/{sidebar/Sidebar,device/DevicePanel,device/Overview,check/FakeCheckTab,health/HealthTab,bench/BenchTab,format/FormatTab}.tsx
apps/gui/src/index.css (estilos .guide), App.tsx
scripts/e2e-loop.sh, README.md
```

---

### Task 1: Crate `usbkeeper-guide` (motor + registro dos 12 tópicos)

**Files:**
- Create: `crates/guide/Cargo.toml`, `crates/guide/src/lib.rs`, `crates/guide/src/terminal.rs`, `crates/guide/topics/<id>.md` (12 arquivos, nesta task só com título + resumo + um parágrafo)
- Modify: `Cargo.toml` (members, dep `pulldown-cmark = "0.13"`, `usbkeeper-guide = { path = "crates/guide" }`)

**Interfaces:**
- Produces: `pub struct Topic { pub id: &'static str, pub title: String, pub summary: String, pub markdown: &'static str }`; `pub fn topics() -> Vec<Topic>`; `pub fn find(query: &str) -> Option<Topic>`; `pub fn to_html(md: &str) -> String`; `pub fn to_terminal(md: &str) -> String`; `pub const IDS: [&str; 12]`.

- [ ] **Step 1: Testes que falham** (`crates/guide/src/lib.rs`, `mod tests`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_has_title_summary_and_unique_id() {
        let all = topics();
        assert_eq!(all.len(), 12);
        let mut ids: Vec<_> = all.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 12);
        for t in &all {
            assert!(!t.title.is_empty() && !t.title.starts_with('#'), "{}", t.id);
            assert!(t.summary.len() > 20, "resumo curto em {}", t.id);
        }
    }

    #[test]
    fn aliases_resolve() {
        for (alias, id) in [("mbr", "mbr-gpt"), ("GPT", "mbr-gpt"), ("exfat", "sistemas-de-arquivos"),
            ("fake", "falsificados"), ("smart", "saude"), ("eject", "remover"), ("remover", "remover")] {
            assert_eq!(find(alias).map(|t| t.id), Some(id), "{alias}");
        }
        assert!(find("nada").is_none());
    }

    #[test]
    fn html_has_tables_and_drops_raw_html() {
        let html = to_html("| a | b |\n|---|---|\n| 1 | 2 |\n\n<script>x</script>\n\ntexto <b>cru</b>");
        assert!(html.contains("<table>"));
        assert!(!html.contains("<script>") && !html.contains("<b>"));
    }

    #[test]
    fn terminal_has_no_markdown_syntax() {
        let md = "# Título\n\nResumo **forte**.\n\n## Seção\n\n- item `código`\n\n| Coluna | Outra |\n|---|---|\n| a | bb |\n";
        let txt = to_terminal(md);
        assert!(txt.starts_with("TÍTULO\n======"));
        assert!(txt.contains("• item código"));
        assert!(!txt.contains("**") && !txt.contains("# ") && !txt.contains('|'));
        assert!(txt.contains("Coluna  Outra"));
    }
}
```

- [ ] **Step 2: Ver falhar** — `cargo test -q -p usbkeeper-guide` → erro de compilação.

- [ ] **Step 3: Implementação**

`crates/guide/Cargo.toml`:

```toml
[package]
name = "usbkeeper-guide"
description = "Guia de ajuda do usbkeeper (Markdown embutido; HTML e terminal)"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
pulldown-cmark = { workspace = true }
```

`crates/guide/src/lib.rs`:

```rust
//! Guia de ajuda: um arquivo Markdown por tópico, embutido no binário e
//! mostrado igual na CLI (texto) e na interface (HTML).

mod terminal;

use pulldown_cmark::{Event, Options, Parser, html};

pub use terminal::to_terminal;

macro_rules! sources {
    ($($id:literal),* $(,)?) => {
        pub const IDS: [&str; 12] = [$($id),*];
        const SOURCES: [(&str, &str); 12] = [$(($id, include_str!(concat!("../topics/", $id, ".md")))),*];
    };
}

sources!(
    "comecar", "tipos", "usb", "mbr-gpt", "sistemas-de-arquivos", "formatar",
    "falsificados", "saude", "velocidade", "remover", "protecoes", "glossario",
);

const ALIASES: [(&str, &str); 12] = [
    ("mbr", "mbr-gpt"), ("gpt", "mbr-gpt"),
    ("fat32", "sistemas-de-arquivos"), ("exfat", "sistemas-de-arquivos"), ("ntfs", "sistemas-de-arquivos"),
    ("ext4", "sistemas-de-arquivos"), ("fs", "sistemas-de-arquivos"),
    ("fake", "falsificados"), ("falso", "falsificados"), ("smart", "saude"),
    ("ejetar", "remover"), ("eject", "remover"),
];

#[derive(Debug, Clone)]
pub struct Topic {
    pub id: &'static str,
    pub title: String,
    pub summary: String,
    pub markdown: &'static str,
}

fn parse(id: &'static str, markdown: &'static str) -> Topic {
    let mut lines = markdown.lines().map(str::trim).filter(|l| !l.is_empty());
    let title = lines.next().unwrap_or_default().trim_start_matches('#').trim().to_string();
    let summary = lines.next().unwrap_or_default().to_string();
    Topic { id, title, summary, markdown }
}

pub fn topics() -> Vec<Topic> {
    SOURCES.iter().map(|(id, md)| parse(id, md)).collect()
}

pub fn find(query: &str) -> Option<Topic> {
    let q = query.trim().to_lowercase();
    let id = ALIASES.iter().find(|(a, _)| *a == q).map(|(_, id)| *id).unwrap_or(q.as_str());
    SOURCES.iter().find(|(i, _)| *i == id).map(|(i, md)| parse(i, md))
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

/// HTML do tópico. HTML bruto no Markdown é descartado.
pub fn to_html(md: &str) -> String {
    let events = Parser::new_ext(md, options()).filter(|e| !matches!(e, Event::Html(_) | Event::InlineHtml(_)));
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}
```

`crates/guide/src/terminal.rs`:

```rust
//! Markdown → texto de terminal: títulos sublinhados, listas com "•",
//! tabelas alinhadas, sem sintaxe Markdown visível.

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

const WIDTH: usize = 88;

fn wrap(text: &str, indent: &str, first: &str) -> String {
    let mut out = String::new();
    let mut line = first.to_string();
    let mut empty = true;
    for word in text.split_whitespace() {
        if !empty && line.chars().count() + 1 + word.chars().count() > WIDTH {
            out.push_str(line.trim_end());
            out.push('\n');
            line = indent.to_string();
            empty = true;
        }
        if !empty {
            line.push(' ');
        }
        line.push_str(word);
        empty = false;
    }
    out.push_str(line.trim_end());
    out
}

pub fn to_terminal(md: &str) -> String {
    let mut out = String::new();
    let mut buf = String::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut heading: Option<HeadingLevel> = None;
    let mut table: Vec<Vec<String>> = Vec::new();
    let mut in_table = false;
    let mut code_block = false;
    let mut link: Option<String> = None;

    for event in Parser::new_ext(md, super::options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => heading = Some(level),
            Event::End(TagEnd::Heading(_)) => {
                let text = buf.trim().to_string();
                buf.clear();
                match heading.take() {
                    Some(HeadingLevel::H1) => {
                        let up = text.to_uppercase();
                        out.push_str(&format!("{up}\n{}\n\n", "=".repeat(up.chars().count())));
                    }
                    Some(HeadingLevel::H2) => out.push_str(&format!("{text}\n{}\n\n", "-".repeat(text.chars().count()))),
                    _ => out.push_str(&format!("▸ {text}\n\n")),
                }
            }
            Event::Start(Tag::List(start)) => lists.push(start),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                if lists.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Item) => buf.clear(),
            Event::End(TagEnd::Item) => {
                let depth = lists.len().saturating_sub(1);
                let pad = "  ".repeat(depth + 1);
                let marker = match lists.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "• ".to_string(),
                };
                let first = format!("{pad}{marker}");
                let indent = " ".repeat(first.chars().count());
                out.push_str(&wrap(buf.trim(), &indent, &first));
                out.push('\n');
                buf.clear();
            }
            Event::End(TagEnd::Paragraph) if !lists.is_empty() => buf.push(' '),
            Event::End(TagEnd::Paragraph) => {
                out.push_str(&wrap(buf.trim(), "", ""));
                out.push_str("\n\n");
                buf.clear();
            }
            Event::Start(Tag::CodeBlock(_)) => code_block = true,
            Event::End(TagEnd::CodeBlock) => {
                for l in buf.lines() {
                    out.push_str(&format!("    {l}\n"));
                }
                out.push('\n');
                buf.clear();
                code_block = false;
            }
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table.clear();
            }
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => table.push(Vec::new()),
            Event::End(TagEnd::TableCell) => {
                if let Some(row) = table.last_mut() {
                    row.push(buf.trim().to_string());
                }
                buf.clear();
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                let cols = table.iter().map(Vec::len).max().unwrap_or(0);
                let widths: Vec<usize> = (0..cols)
                    .map(|c| table.iter().filter_map(|r| r.get(c)).map(|s| s.chars().count()).max().unwrap_or(0))
                    .collect();
                for (i, row) in table.iter().enumerate() {
                    let line: Vec<String> = row.iter().enumerate()
                        .map(|(c, cell)| format!("{cell:<w$}", w = widths[c]))
                        .collect();
                    out.push_str(&format!("  {}\n", line.join("  ").trim_end()));
                    if i == 0 {
                        let total: usize = widths.iter().sum::<usize>() + 2 * cols.saturating_sub(1);
                        out.push_str(&format!("  {}\n", "-".repeat(total)));
                    }
                }
                out.push('\n');
            }
            Event::Start(Tag::Link { dest_url, .. }) => link = Some(dest_url.to_string()),
            Event::End(TagEnd::Link) => {
                if let Some(url) = link.take().filter(|u| u.starts_with("http")) {
                    buf.push_str(&format!(" ({url})"));
                }
            }
            Event::Text(t) | Event::Code(t) => buf.push_str(&t),
            Event::SoftBreak => buf.push(if code_block { '\n' } else { ' ' }),
            Event::HardBreak => buf.push('\n'),
            Event::Rule => out.push_str(&format!("{}\n\n", "─".repeat(40))),
            _ => {}
        }
        let _ = in_table;
    }
    out.trim_end().to_string() + "\n"
}
```

Cada `crates/guide/topics/<id>.md` (provisório nesta task), por exemplo `mbr-gpt.md`:

```markdown
# Tabela de partições: MBR × GPT

Antes do sistema de arquivos vem a tabela de partições: o “mapa” que diz onde cada partição começa e termina.

(conteúdo completo na Task 2)
```

(com o título de cada id conforme a tabela do spec, seção 2).

- [ ] **Step 4: Rodar** — `cargo test -q -p usbkeeper-guide` → 4 passed; `cargo clippy -q -p usbkeeper-guide --all-targets -- -D warnings` limpo.

- [ ] **Step 5: Commit** — `git commit -m "feat(guide): crate do guia (Markdown embutido → HTML e terminal)"`

---

### Task 2: Conteúdo dos 12 tópicos

**Files:** Modify: `crates/guide/topics/*.md`; Test: `crates/guide/src/lib.rs` (novo teste de conteúdo).

**Interfaces:** Consumes `topics()`, `find()`. Produces o texto final.

- [ ] **Step 1: Teste de conteúdo que falha**

```rust
    /// Fatos obrigatórios por tópico (o texto pode variar; estes pontos não podem faltar).
    const MUST: [(&str, &[&str]); 12] = [
        ("comecar", &["backup", "disco do sistema", "série", "senha"]),
        ("tipos", &["pendrive", "cartão", "HD externo", "SSD", "NVMe", "SMART", "gaveta"]),
        ("usb", &["USB 2.0", "USB 3", "480", "5 Gb/s", "cabo", "hub", "porta"]),
        ("mbr-gpt", &["MBR", "GPT", "2 TiB", "4 partições", "UEFI", "BIOS", "TV"]),
        ("sistemas-de-arquivos", &["FAT32", "exFAT", "NTFS", "ext4", "4 GB", "macOS", "journaling"]),
        ("formatar", &["índice", "recuper", "apagar com segurança", "antes de formatar"]),
        ("falsificados", &["volta", "descart", "rápido", "completo", "--max-size", "reembolso"]),
        ("saude", &["realocados", "pendentes", "incorrigíveis", "CRC", "pendrive", "varredura", "trocar"]),
        ("velocidade", &["MB/s", "IOPS", "4K", "sequencial", "cache"]),
        ("remover", &["cache", "em uso", "gerenciador de arquivos", "LED", "Remover com segurança"]),
        ("protecoes", &["disco do sistema", "USB", "série", "O_EXCL", "restaura"]),
        ("glossario", &["Setor", "Partição", "Montar", "Firmware", "TRIM"]),
    ];

    #[test]
    fn topics_cover_required_facts_and_are_substantial() {
        for (id, facts) in MUST {
            let t = find(id).unwrap();
            for f in facts {
                assert!(t.markdown.contains(f), "tópico {id} sem \"{f}\"");
            }
            let words = t.markdown.split_whitespace().count();
            assert!(words >= 250, "tópico {id} curto demais ({words} palavras)");
        }
    }
```

- [ ] **Step 2: Ver falhar** — `cargo test -q -p usbkeeper-guide topics_cover` → FAIL (`tópico comecar sem "backup"`).

- [ ] **Step 3: Escrever os 12 textos.** Regras para todos: `# Título` + resumo de uma frase; seções `##`; parágrafos curtos; um bloco “**Na prática**” com o que fazer; um bloco “**Cuidado**” com o erro mais comum; jargão sempre explicado na primeira vez; números corretos (USB 2.0 = 480 Mb/s ≈ 35–42 MB/s reais; USB 3.2 Gen 1 = 5 Gb/s; Gen 2 = 10 Gb/s; FAT32 arquivo ≤ 4 GiB − 1 byte; MBR ≤ 2 TiB com setor de 512 B e 4 primárias; GPT 128 partições). Conteúdo mínimo por tópico (além dos fatos do teste):
  - `comecar`: o que o usbkeeper faz; regras de ouro numeradas (backup, conferir modelo/tamanho/série, nunca o disco do sistema, não desplugar no meio, ler o aviso de cada operação); por que pede senha e o que a confirmação pela série evita.
  - `tipos`: como cada um guarda dados (flash NAND × prato magnético), controlador e firmware, durabilidade (ciclos de escrita × choque mecânico), quais têm SMART e por quê, gavetas/adaptadores USB-SATA/USB-NVMe e o efeito deles no SMART.
  - `usb`: tabela das versões (nome comercial, velocidade teórica, real aproximada), como reconhecer porta USB 3, cabo e hub limitando, porta da frente do gabinete, HD 2,5” que precisa de mais energia (Y-cable/hub alimentado), o aviso “USB 3 em porta USB 2” do usbkeeper.
  - `mbr-gpt`: analogia do índice de um livro; tabela comparativa; quando usar MBR (aparelhos antigos, TVs, som de carro, pendrives até 2 TB, `--max-size`) e GPT (discos > 2 TB, boot UEFI); o padrão do usbkeeper (MBR até 2 TiB).
  - `sistemas-de-arquivos`: tabela de compatibilidade (Windows, macOS, Linux, TV/som/carro, videogame) com leitura/escrita; limite de 4 GB do FAT32 com exemplo (vídeo/ISO); journaling explicado; recomendação por uso (pendrive para todos → exFAT; aparelho antigo → FAT32; só Windows/HD grande → NTFS; só Linux → ext4); menção a APFS/HFS+ do macOS.
  - `formatar`: formatar = recriar o índice; arquivos ficam recuperáveis; formatação rápida × completa; por que formatar não é apagar com segurança (e flash tem sobra que nem sobrescrever garante); checklist antes de formatar (backup, disco certo, desmontar); o que o usbkeeper faz (wipefs, tabela, mkfs).
  - `falsificados`: como a fraude é feita (firmware anuncia mais); os dois tipos (dá a volta × descarta) com exemplo numérico; sinais (preço, “2 TB” por pouco, marca genérica); teste rápido × completo (o que cada um prova e quanto demora); o que fazer (não usar, laudo `--json`/HTML, reembolso, `format --max-size` para aproveitar a parte real).
  - `saude`: o que é SMART; tabela dos atributos principais (5 realocados, 197 pendentes, 198 incorrigíveis, 199 CRC, 9 horas, 194 temperatura, NVMe vida útil/erros de mídia) com “o que significa” e “quando preocupar”; por que pendrive/cartão quase nunca tem SMART e o adaptador pode esconder; varredura de superfície; quando trocar o disco.
  - `velocidade`: MB/s × IOPS com exemplos (copiar filme × abrir muitos arquivos pequenos); por que 4K importa para rodar programas/sistema; cache SLC (rápido no começo, despenca depois); faixas típicas (pendrive barato, pendrive bom, HD externo, SSD externo); USB limitando.
  - `remover`: o sistema guarda escritas em cache e grava depois; o que acontece ao arrancar no meio (arquivo corrompido, partição precisando de reparo); o que é “em uso” e como resolver (fechar gerenciador de arquivos/terminal/programa aberto no pendrive; o usbkeeper mostra qual); passo a passo do botão “Remover com segurança”; LED; quando só desmonta.
  - `protecoes`: cada trava em linguagem simples (disco do sistema nunca, só USB por padrão, desmontar antes, digitar a série, reconferir identidade, O_EXCL, teste rápido restaura os blocos, histórico); o que **não** protege (escolher o pendrive errado entre dois iguais sem conferir a série; backup é com você).
  - `glossario`: verbetes em ordem alfabética (Bloco, Cache, Firmware, Flash NAND, IOPS, LBA, Montar, Partição, Ponte USB, Setor, SMART, Sistema de arquivos, Tabela de partições, TRIM, …), cada um com uma ou duas frases.

- [ ] **Step 4: Rodar** — `cargo test -q -p usbkeeper-guide` → 5 passed.

- [ ] **Step 5: Commit** — `git commit -m "docs(guide): conteúdo dos 12 tópicos do guia"`

---

### Task 3: CLI — `guia`, apelido `remover`, dicas, erro com processos, remoção parcial

**Files:** Modify `Cargo.toml` (`apps/cli` dep `usbkeeper-guide`), `apps/cli/src/main.rs`, `apps/cli/src/destructive.rs`, `apps/cli/src/checks.rs`, `apps/cli/src/health.rs`, `apps/cli/src/machine.rs`.

**Interfaces:**
- Produces: `Command::Guia { topic: Option<String>, html: bool }`; `Eject` com `#[command(alias = "remover")]`; `pub struct BusyError { pub message: String, pub holders: Vec<Holder> }` (Display + Error); `machine::error_value(e: &anyhow::Error) -> serde_json::Value` (`{"error", "holders"?}`); `eject --json` em sucesso parcial: `{"device", "powered_off": false, "note"}`.

- [ ] **Step 1: Testes que falham**

Em `apps/cli/src/main.rs` (`mod tests`):

```rust
    #[test]
    fn guia_and_remover_parse() {
        let cli = Cli::try_parse_from(["usbkeeper", "guia", "mbr"]).unwrap();
        assert!(matches!(cli.command, Command::Guia { topic: Some(t), html: false } if t == "mbr"));
        let cli = Cli::try_parse_from(["usbkeeper", "remover", "sdb"]).unwrap();
        assert!(matches!(cli.command, Command::Eject { device, .. } if device == "sdb"));
    }
```

Em `apps/cli/src/machine.rs` (`mod tests`):

```rust
    #[test]
    fn busy_error_json_carries_holders() {
        let e = anyhow::Error::new(crate::destructive::BusyError {
            message: "/media/x está em uso".into(),
            holders: vec![usbkeeper_core::Holder { pid: 42, command: "vlc".into(), path: "/media/x/f.mkv".into() }],
        });
        let v = error_value(&e);
        assert_eq!(v["holders"][0]["pid"], 42);
        assert!(v["error"].as_str().unwrap().contains("em uso"));
        let plain = error_value(&anyhow::anyhow!("outro"));
        assert!(plain.get("holders").is_none());
    }
```

- [ ] **Step 2: Ver falhar** — `cargo test -q -p usbkeeper` → erros de compilação.

- [ ] **Step 3: Implementação**

`apps/cli/src/destructive.rs` — substituir `busy_error`:

```rust
/// Disco em uso: a mensagem para pessoas e os processos estruturados (a
/// interface mostra a lista; `--progress-json` a inclui no erro JSON).
#[derive(Debug)]
pub struct BusyError {
    pub message: String,
    pub holders: Vec<usbkeeper_core::Holder>,
}

impl std::fmt::Display for BusyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BusyError {}

fn busy_error(e: CoreError) -> anyhow::Error {
    if let CoreError::Busy { mountpoint, holders } = e {
        let list = holders
            .iter()
            .map(|h| format!("  PID {} {} ({})", h.pid, h.command, h.path))
            .collect::<Vec<_>>()
            .join("\n");
        let message = if holders.is_empty() {
            format!("{mountpoint} está em uso por um processo que não consigo ver (talvez de outro usuário)\nveja: usbkeeper guia remover")
        } else {
            format!("{mountpoint} está em uso por:\n{list}\nfeche esses programas e tente de novo (veja: usbkeeper guia remover)")
        };
        return anyhow::Error::new(BusyError { message, holders });
    }
    e.into()
}
```

Em `precheck`, a dica de montado passa a: `"\n(use --unmount para desmontar automaticamente; veja: usbkeeper guia remover)"`.

`eject`: trocar o bloco de power-off por:

```rust
    let mut note: Option<String> = None;
    if power_off && let Err(e) = b.ejector.power_off(&device) {
        // Desmontou, mas o desligamento não é suportado (ex.: alguns leitores
        // de cartão): ainda é seguro tirar depois que o LED parar.
        note = Some(e.to_string());
    }
    let powered_off = power_off && note.is_none();
    if json {
        crate::print_json(&serde_json::json!({
            "device": device.path,
            "powered_off": powered_off,
            "note": note,
        }))?;
    } else if powered_off {
        println!("✔ {} desligado. Pode remover com segurança.", device.path);
    } else if let Some(n) = note {
        println!("✔ {} desmontado (não deu para desligar: {n}).\n  Pode tirar quando o LED parar de piscar.", device.path);
    } else {
        println!("✔ {} desmontado.", device.path);
    }
```

`apps/cli/src/machine.rs` — acrescentar:

```rust
/// Erro final em JSON; inclui os processos quando o disco está em uso.
pub fn error_value(e: &anyhow::Error) -> serde_json::Value {
    let mut v = serde_json::json!({ "error": format!("{e:#}") });
    if let Some(busy) = e.downcast_ref::<crate::destructive::BusyError>() {
        v["holders"] = serde_json::json!(busy.holders);
    }
    v
}
```

e `main` passa a usar `eprintln!("{}", machine::error_value(&e))` no modo `--progress-json` (remover `error_line` se ficar sem uso, ajustando o teste `error_line_is_json` para `error_value`).

`apps/cli/src/main.rs`:
- `Eject` recebe `#[command(alias = "remover")]` e o doc `/// Remove com segurança: desmonta e desliga (apelido: remover)`.
- Novo comando:

```rust
    /// Guia: pendrive, HD externo, MBR × GPT, sistemas de arquivos, falsificados… (ex.: usbkeeper guia mbr)
    Guia {
        /// Tópico (id ou apelido). Sem tópico, lista todos.
        topic: Option<String>,
        /// Imprime o HTML do tópico
        #[arg(long)]
        html: bool,
    },
```

  e no `run`:

```rust
        Command::Guia { topic, html } => match topic {
            None => {
                println!("Tópicos do guia (usbkeeper guia <tópico>):\n");
                for t in usbkeeper_guide::topics() {
                    println!("  {:<22} {}\n  {:<22} {}\n", t.id, t.title, "", t.summary);
                }
            }
            Some(q) => {
                let t = usbkeeper_guide::find(&q).with_context(|| {
                    format!("tópico desconhecido: {q:?}; rode `usbkeeper guia` para ver a lista")
                })?;
                if html {
                    println!("{}", usbkeeper_guide::to_html(t.markdown));
                } else {
                    print!("{}", usbkeeper_guide::to_terminal(t.markdown));
                }
            }
        },
```

(`use anyhow::Context;` no topo do `main.rs`).

Dicas: `checks.rs::print_report` no veredito `Fake` acrescenta `println!("Entenda: usbkeeper guia falsificados")`; `health.rs::print_smart` com veredito `Warning|Failing` acrescenta `println!("Entenda cada item: usbkeeper guia saude")`; em `destructive::format`, o erro de rótulo vira `anyhow::anyhow!("{e} (veja: usbkeeper guia sistemas-de-arquivos)")`.

- [ ] **Step 4: Rodar** — `cargo fmt && cargo clippy -q --workspace --exclude usbkeeper-gui --all-targets -- -D warnings && cargo test -q --workspace --exclude usbkeeper-gui` → tudo ok; `cargo run -q -p usbkeeper -- guia` lista 12; `cargo run -q -p usbkeeper -- guia gpt | head -3` mostra `TABELA DE PARTIÇÕES: MBR × GPT`.

- [ ] **Step 5: Commit** — `git commit -m "feat(cli): usbkeeper guia, apelido remover, dicas para o guia e erro JSON com os processos que prendem o disco"`

---

### Task 4: Interface — painel de Ajuda e botões “?”

**Files:**
- Create: `apps/gui/src-tauri/src/guide.rs`; `apps/gui/src/lib/guideLinks.ts`; `apps/gui/src/features/help/HelpPanel.tsx`, `HelpButton.tsx`, `HelpPanel.test.tsx`
- Modify: `apps/gui/src-tauri/Cargo.toml` (dep `usbkeeper-guide`), `src/lib.rs`, `apps/gui/src/lib/{api,store}.ts`, `App.tsx`, `index.css`, `features/sidebar/Sidebar.tsx`, `features/{check/FakeCheckTab,health/HealthTab,bench/BenchTab,format/FormatTab,device/Overview}.tsx`; Test: `crates/guide/src/lib.rs` (consistência com `guideLinks.ts`)

**Interfaces:**
- Produces (Rust): `#[derive(Serialize, TS)] #[ts(export)] struct GuideTopic { id: String, title: String, summary: String, html: String }`; comando `guide_topics() -> Vec<GuideTopic>`.
- Produces (TS): `GUIDE` (ids constantes) em `guideLinks.ts`; `api.guideTopics()`; store `help: { open: boolean; topic: string }`, `openHelp(topic?: string)`, `closeHelp()`; `<HelpButton topic={GUIDE.x} label="…" />`; `<HelpPanel topics onClose />`.

- [ ] **Step 1: Testes que falham**

`crates/guide/src/lib.rs` (`mod tests`):

```rust
    #[test]
    fn every_gui_help_link_points_to_an_existing_topic() {
        let ts = include_str!("../../../apps/gui/src/lib/guideLinks.ts");
        let ids: Vec<&str> = ts.split('"').skip(1).step_by(2).collect();
        assert!(!ids.is_empty());
        for id in ids {
            assert!(IDS.contains(&id), "guideLinks.ts aponta para tópico inexistente: {id}");
        }
    }
```

`apps/gui/src/features/help/HelpPanel.test.tsx`:

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { HelpPanel } from "./HelpPanel";
import { HelpButton } from "./HelpButton";
import { useApp } from "../../lib/store";

const topics = [
  { id: "mbr-gpt", title: "Tabela de partições: MBR × GPT", summary: "O mapa do disco.", html: "<h1>MBR</h1><p>Mapa</p>" },
  { id: "remover", title: "Remover com segurança", summary: "Por que não arrancar.", html: "<h1>Remover</h1><p>Cache</p>" },
];

describe("ajuda", () => {
  it("o botão ? abre o tópico certo", () => {
    render(<HelpButton topic="remover" label="Remoção segura" />);
    fireEvent.click(screen.getByRole("button", { name: "Ajuda: Remoção segura" }));
    expect(useApp.getState().help).toEqual({ open: true, topic: "remover" });
  });

  it("mostra o tópico selecionado e a busca filtra o índice", () => {
    useApp.setState({ help: { open: true, topic: "remover" } });
    render(<HelpPanel topics={topics} />);
    expect(screen.getByText("Cache")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Buscar no guia"), { target: { value: "mapa" } });
    expect(screen.queryByRole("button", { name: /Remover com segurança/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /MBR × GPT/ }));
    expect(screen.getByText("Mapa")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Ver falhar** — `cargo test -q -p usbkeeper-guide every_gui` e `npx vitest run src/features/help` → falham (arquivos inexistentes).

- [ ] **Step 3: Implementação**

`apps/gui/src/lib/guideLinks.ts`:

```ts
// Ids dos tópicos do guia usados pela interface. Um teste Rust
// (crates/guide) confere que cada id existe.
export const GUIDE = {
  start: "comecar",
  types: "tipos",
  usb: "usb",
  partitionTable: "mbr-gpt",
  filesystems: "sistemas-de-arquivos",
  format: "formatar",
  fake: "falsificados",
  health: "saude",
  speed: "velocidade",
  remove: "remover",
  protections: "protecoes",
  glossary: "glossario",
} as const;
```

`apps/gui/src-tauri/src/guide.rs`:

```rust
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct GuideTopic {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub html: String,
}

#[tauri::command]
pub fn guide_topics() -> Vec<GuideTopic> {
    usbkeeper_guide::topics()
        .into_iter()
        .map(|t| GuideTopic {
            id: t.id.to_string(),
            html: usbkeeper_guide::to_html(t.markdown),
            title: t.title,
            summary: t.summary,
        })
        .collect()
}
```

(registrar `pub mod guide;` e `guide::guide_topics` no `lib.rs`.)

`api.ts`: `export const guideTopics = () => invoke<GuideTopic[]>("guide_topics");` (import do binding `GuideTopic`).

`store.ts`: estado `help: { open: false, topic: "comecar" }`, ações `openHelp: (topic = get().help.topic) => set({ help: { open: true, topic } })`, `closeHelp: () => set((s) => ({ help: { ...s.help, open: false } }))`.

`apps/gui/src/features/help/HelpButton.tsx`:

```tsx
import { useApp } from "../../lib/store";

export function HelpButton({ topic, label }: { topic: string; label: string }) {
  const openHelp = useApp((s) => s.openHelp);
  return (
    <button type="button" aria-label={`Ajuda: ${label}`} title={`Entenda: ${label}`} onClick={() => openHelp(topic)}
      className="inline-grid h-5 w-5 place-items-center rounded-full text-xs font-bold align-middle"
      style={{ border: "1px solid var(--line)", color: "var(--accent)" }}>?</button>
  );
}
```

`apps/gui/src/features/help/HelpPanel.tsx`:

```tsx
import { useMemo, useState } from "react";
import type { GuideTopic } from "../../lib/bindings/GuideTopic";
import { useApp } from "../../lib/store";

const plain = (html: string) => html.replace(/<[^>]+>/g, " ").toLowerCase();

export function HelpPanel({ topics }: { topics: GuideTopic[] }) {
  const { help, openHelp, closeHelp } = useApp();
  const [query, setQuery] = useState("");
  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return topics;
    return topics.filter((t) => t.title.toLowerCase().includes(q) || t.summary.toLowerCase().includes(q) || plain(t.html).includes(q));
  }, [query, topics]);
  const current = topics.find((t) => t.id === help.topic) ?? topics[0];
  return (
    <div className="grid h-full" style={{ gridTemplateColumns: "280px 1fr" }}>
      <nav className="flex flex-col gap-2 overflow-auto border-r p-3" style={{ borderColor: "var(--line)" }}>
        <button type="button" onClick={closeHelp} className="self-start rounded-md px-2 py-1 text-sm" style={{ border: "1px solid var(--line)" }}>← Voltar</button>
        <input aria-label="Buscar no guia" placeholder="Buscar…" value={query} onChange={(e) => setQuery(e.target.value)} className="rounded-md px-2 py-1 text-sm" />
        <ul className="flex flex-col gap-1">
          {visible.map((t) => (
            <li key={t.id}>
              <button type="button" onClick={() => openHelp(t.id)} className="w-full rounded-md px-2 py-1.5 text-left"
                style={{ background: t.id === current?.id ? "var(--line)" : "transparent" }}>
                <span className="block text-sm font-medium">{t.title}</span>
                <span className="block text-xs" style={{ color: "var(--muted)" }}>{t.summary}</span>
              </button>
            </li>
          ))}
          {visible.length === 0 && <li className="text-sm" style={{ color: "var(--muted)" }}>Nada encontrado.</li>}
        </ul>
      </nav>
      {/* Conteúdo compilado no binário (crates/guide), sem HTML bruto: seguro de inserir. */}
      <article className="guide overflow-auto px-8 py-6" dangerouslySetInnerHTML={{ __html: current?.html ?? "" }} />
    </div>
  );
}
```

`index.css` += estilos do guia:

```css
.guide { max-width: 860px; line-height: 1.6; }
.guide h1 { font-size: 1.6rem; font-weight: 700; margin: 0 0 .5rem; }
.guide h2 { font-size: 1.15rem; font-weight: 650; margin: 1.6rem 0 .5rem; }
.guide h3 { font-weight: 600; margin: 1.2rem 0 .4rem; }
.guide p, .guide ul, .guide ol, .guide table { margin: 0 0 .8rem; }
.guide ul { list-style: disc; padding-left: 1.4rem; } .guide ol { list-style: decimal; padding-left: 1.4rem; }
.guide code { font-family: ui-monospace, monospace; background: var(--line); padding: 0 .3rem; border-radius: 4px; }
.guide table { border-collapse: collapse; width: 100%; font-size: .92rem; }
.guide th, .guide td { border: 1px solid var(--line); padding: .35rem .55rem; text-align: left; vertical-align: top; }
.guide strong { font-weight: 650; }
```

`App.tsx`: carregar `guideTopics()` uma vez num `useState`; no `<main>`: `help.open ? <HelpPanel topics={topics} /> : current ? <DevicePanel …/> : null`; rodapé da barra lateral com `<button onClick={() => openHelp()}>Ajuda e guia</button>`.

Botões “?” (todos com `import { GUIDE } from "../../lib/guideLinks"`):
- `FakeCheckTab`: ao lado dos dois cartões, `<HelpButton topic={GUIDE.fake} label="pendrives falsificados" />`.
- `HealthTab`: ao lado de “Ler SMART”, `GUIDE.health`.
- `BenchTab`: ao lado de “Medir leitura”, `GUIDE.speed`.
- `FormatTab`: no rótulo “Sistema de arquivos” (`GUIDE.filesystems`), “Tabela” (`GUIDE.partitionTable`), “Usar só os primeiros” (`GUIDE.fake`); acima do botão, `GUIDE.format`.
- `Overview`: no título “Operações destrutivas”, `GUIDE.protections`.

- [ ] **Step 4: Rodar** — `cargo test -q -p usbkeeper-guide -p usbkeeper-gui` ok; `cd apps/gui && npx vitest run && npm run build` ok (os testes antigos que buscam rótulos por texto continuam passando porque os “?” têm `aria-label` próprio).

- [ ] **Step 5: Commit** — `git commit -m "feat(gui): painel de ajuda com busca e botões ? contextuais"`

---

### Task 5: Interface — remover com segurança

**Files:**
- Create: `apps/gui/src/features/remove/{RemoveButton,InUseCard,Notice}.tsx`, `remove.test.tsx`
- Modify: `apps/gui/src-tauri/src/cli.rs` (args do eject; `StreamLine::Error` com holders), `ops.rs` (`OpFinished.holders`), `apps/gui/src/lib/store.ts` (`notice`), `features/sidebar/Sidebar.tsx`, `features/device/DevicePanel.tsx`, `App.tsx`

**Interfaces:**
- Consumes: erro JSON `{"error","holders"}` (Task 3); `Holder` (binding).
- Produces: `StreamLine::Error { message: String, holders: Vec<Holder> }`; `OpFinished { …, holders: Vec<Holder> }`; store `notice: { text: string; tone: "ok" | "warn" } | null`, `dismissNotice()`; `opFinished` gera o aviso para `kind === "eject"`; `<RemoveButton view compact? />`, `<InUseCard holders error onRetry />`, `<Notice />`.

- [ ] **Step 1: Testes que falham**

Rust (`cli.rs`, ajustar/adicionar):

```rust
    #[test]
    fn eject_asks_for_json_errors() {
        let a = args(&OpRequest::Eject { device: "/dev/sdb".into() }, "ABC", false);
        assert_eq!(a, s(&["eject", "/dev/sdb", "--json", "--progress-json"]));
    }

    #[test]
    fn error_line_keeps_holders() {
        match parse_line(r#"{"error":"em uso","holders":[{"pid":7,"command":"vlc","path":"/m/x"}]}"#) {
            StreamLine::Error { message, holders } => {
                assert_eq!(message, "em uso");
                assert_eq!(holders[0].pid, 7);
            }
            other => panic!("{other:?}"),
        }
    }
```

(o teste antigo `eject_has_no_cancel_stdin` é substituído por `eject_asks_for_json_errors`; `parses_stream_lines` passa a casar `StreamLine::Error { message, .. }`).

`apps/gui/src/features/remove/remove.test.tsx`:

```tsx
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { useApp } from "../../lib/store";
import { view } from "../../test/fixtures";
import { RemoveButton } from "./RemoveButton";
import { InUseCard } from "./InUseCard";
import { Notice } from "./Notice";

vi.mock("../../lib/api", () => ({ startOperation: vi.fn(() => Promise.resolve(1)) }));

beforeEach(() => useApp.setState({ ops: {}, finished: {}, notice: null, devices: [view()] }));

describe("remover com segurança", () => {
  it("bloqueado durante uma operação no disco", () => {
    useApp.setState({ ops: { "/dev/sdb": { id: 1, kind: "fake_check", startedAt: 0, phaseStartedAt: 0, progress: null, cancelling: false } } });
    render(<RemoveButton view={view()} />);
    const b = screen.getByRole("button", { name: /Remover com segurança/ }) as HTMLButtonElement;
    expect(b.disabled).toBe(true);
    expect(b.title).toMatch(/operação em andamento/);
  });

  it("cartão em uso lista os processos e tenta de novo", () => {
    const onRetry = vi.fn();
    render(<InUseCard error="/media/x está em uso" holders={[{ pid: 7, command: "dolphin", path: "/media/x" }]} onRetry={onRetry} />);
    expect(screen.getByText(/dolphin/)).toBeTruthy();
    expect(screen.getByText(/gerenciador de arquivos/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    expect(onRetry).toHaveBeenCalled();
  });

  it("sucesso gera o aviso 'pode tirar' com o modelo, mesmo depois de o disco sumir", () => {
    useApp.getState().opFinished({ id: 1, device: "/dev/sdb", kind: "eject", ok: true, result: { device: "/dev/sdb", powered_off: true, note: null }, error: null, holders: [] });
    useApp.setState({ devices: [] });
    render(<Notice />);
    expect(screen.getByText(/Pode tirar o Pen \(sdb\)/)).toBeTruthy();
    fireEvent.click(screen.getByRole("status"));
    expect(useApp.getState().notice).toBeNull();
  });

  it("só desmontou: avisa para esperar o LED", () => {
    useApp.getState().opFinished({ id: 1, device: "/dev/sdb", kind: "eject", ok: true, result: { device: "/dev/sdb", powered_off: false, note: "não suportado" }, error: null, holders: [] });
    expect(useApp.getState().notice?.text).toMatch(/LED parar de piscar/);
  });
});
```

- [ ] **Step 2: Ver falhar** — `cargo test -q -p usbkeeper-gui` e `npx vitest run src/features/remove` → falham.

- [ ] **Step 3: Implementação**

`cli.rs`:
- Eject: `a.extend(["eject".into(), device.clone(), "--json".into(), "--progress-json".into()]); return a;`
- `StreamLine::Error { message: String, holders: Vec<usbkeeper_core::Holder> }`; em `parse_line`, quando houver `error`:

```rust
            let holders = v
                .get("holders")
                .and_then(|h| serde_json::from_value::<Vec<HolderIn>>(h.clone()).ok())
                .unwrap_or_default()
                .into_iter()
                .map(|h| usbkeeper_core::Holder { pid: h.pid, command: h.command, path: h.path })
                .collect();
            return StreamLine::Error { message: e.to_string(), holders };
```

  com `#[derive(Deserialize)] struct HolderIn { pid: u32, command: String, path: String }` (o `Holder` do core só deriva `Serialize`).

`ops.rs`: guardar `last_holders` junto de `last_error`; `OpFinished` ganha `pub holders: Vec<usbkeeper_core::Holder>`; `exit_message(status, last_error.as_deref())` inalterado.

`store.ts`:

```ts
  notice: { text: string; tone: "ok" | "warn" } | null;
  dismissNotice: () => void;
  // em opFinished, antes de remover a operação:
  //   const notice = f.kind === "eject" && f.ok ? ejectNotice(s.devices, f) : s.notice;
```

com

```ts
function ejectNotice(devices: DeviceView[], f: OpFinished): AppState["notice"] {
  const d = devices.find((v) => v.device.path === f.device)?.device;
  const name = d ? `${d.model ?? d.name} (${d.name})` : f.device;
  const r = (f.result ?? {}) as { powered_off?: boolean };
  return r.powered_off
    ? { text: `✔ Pode tirar o ${name}.`, tone: "ok" }
    : { text: `✔ ${name} desmontado. Pode tirar quando o LED parar de piscar.`, tone: "warn" };
}
```

`RemoveButton.tsx`:

```tsx
import type { DeviceView } from "../../lib/bindings/DeviceView";
import { useOperation } from "../ops/useOperation";

export function RemoveButton({ view, compact = false }: { view: DeviceView; compact?: boolean }) {
  const op = useOperation(view);
  const busy = !!op.running;
  const title = busy ? "Aguarde: há uma operação em andamento neste disco" : "Desmonta e desliga para você poder tirar o dispositivo";
  return (
    <button type="button" disabled={busy} title={title} aria-label={compact ? `Remover com segurança ${view.device.name}` : undefined}
      onClick={(e) => { e.stopPropagation(); op.run({ kind: "eject", device: view.device.path }); }}
      className={compact ? "rounded-md px-2 py-0.5 text-sm disabled:opacity-40" : "rounded-md px-3 py-1 text-sm font-medium disabled:opacity-40"}
      style={{ border: "1px solid var(--line)" }}>
      {compact ? "⏏" : "⏏ Remover com segurança"}
    </button>
  );
}
```

`InUseCard.tsx`:

```tsx
import type { Holder } from "../../lib/bindings/Holder";
import { GUIDE } from "../../lib/guideLinks";
import { HelpButton } from "../help/HelpButton";

export function InUseCard({ error, holders, onRetry }: { error: string; holders: Holder[]; onRetry: () => void }) {
  return (
    <section className="rounded-xl p-4 text-sm" style={{ border: "1px solid var(--warn)", background: "var(--warn-bg)" }}>
      <h3 className="font-semibold">Não dá para remover ainda <HelpButton topic={GUIDE.remove} label="remover com segurança" /></h3>
      {holders.length > 0 ? (
        <>
          <p className="mt-1">Estes programas estão usando o dispositivo:</p>
          <ul className="mt-1">{holders.map((h) => <li key={h.pid}>• <strong>{h.command}</strong> (PID {h.pid}): {h.path}</li>)}</ul>
          <p className="mt-2">Feche-os (muitas vezes é a janela do gerenciador de arquivos aberta no pendrive ou um terminal dentro dele) e tente de novo.</p>
        </>
      ) : (
        <p className="mt-1 whitespace-pre-wrap">{error}</p>
      )}
      <button type="button" onClick={onRetry} className="mt-3 rounded-md px-3 py-1" style={{ border: "1px solid var(--line)", background: "var(--panel)" }}>Tentar de novo</button>
    </section>
  );
}
```

`Notice.tsx`:

```tsx
import { useEffect } from "react";
import { useApp } from "../../lib/store";

const TIMEOUT_MS = 15_000;

export function Notice() {
  const notice = useApp((s) => s.notice);
  const dismiss = useApp((s) => s.dismissNotice);
  useEffect(() => {
    if (!notice) return;
    const t = window.setTimeout(dismiss, TIMEOUT_MS);
    return () => window.clearTimeout(t);
  }, [notice, dismiss]);
  if (!notice) return null;
  return (
    <div role="status" onClick={dismiss} className={`cursor-pointer px-6 py-2 text-sm font-medium tone-${notice.tone}`}>
      {notice.text} <span style={{ opacity: 0.7 }}>(clique para fechar)</span>
    </div>
  );
}
```

`DevicePanel.tsx`: trocar o botão Ejetar por `<RemoveButton view={view} />`; abaixo do header, quando `ejectOp.last("eject")` falhou: `<InUseCard error={…error} holders={…holders} onRetry={() => ejectOp.run({ kind: "eject", device: d.path })} />` (o `ejectOp` deixa de mostrar o texto cru). `Sidebar.tsx`: cada item vira `SidebarItem` com o botão do disco e, para USB que não é do sistema, `<RemoveButton view={v} compact />` ao lado (fora do `<button>` do item, em um `div` flex). `App.tsx`: `<Notice />` no topo do `<main>`.

- [ ] **Step 4: Rodar** — `cargo test -q -p usbkeeper-gui` ok; `cd apps/gui && npx vitest run && npm run build` ok (testes do DevicePanel que procuravam “Ejetar” passam a procurar “Remover com segurança”).

- [ ] **Step 5: Commit** — `git commit -m "feat(gui): remover com segurança em destaque, cartão 'em uso' e aviso 'pode tirar'"`

---

### Task 6: e2e, README, pacote

**Files:** Modify `scripts/e2e-loop.sh`, `README.md`, `.github/workflows/ci.yml` (nada a mudar além do workspace, que já inclui o crate novo).

- [ ] **Step 1: e2e** — antes do bloco M3:

```bash
# M5b: guia e remoção segura
n=$("$BIN" guia | grep -cE '^  [a-z-]+ +[[:upper:]]' || true)
(( n == 12 )) && ok "guia lista 12 tópicos" || bad "guia lista $n tópicos"
"$BIN" guia gpt | head -1 | grep -q "MBR × GPT" && ok "guia aceita apelido (gpt)" || bad "guia gpt"
mount "${LOOP}p1" "$WORK/mnt"; sleep 300 < /dev/null > "$WORK/mnt/segura.txt" & HOLDER=$!
sleep 0.5
err=$("$BIN" remover "$LOOP" --no-power-off --allow-non-usb --json --progress-json 2>&1 >/dev/null || true)
jq -e --argjson p "$HOLDER" '.holders | map(.pid) | index($p) != null' <<<"$(tail -1 <<<"$err")" >/dev/null \
  && ok "remover em uso devolve os processos em JSON" || bad "holders: $(tail -1 <<<"$err")"
kill "$HOLDER"; wait "$HOLDER" 2>/dev/null || true; HOLDER=""
"$BIN" remover "$LOOP" --no-power-off --allow-non-usb --json | jq -e '.powered_off == false' >/dev/null \
  && ok "remover --no-power-off desmonta" || bad "remover --no-power-off"
```

- [ ] **Step 2: README** — seção “Guia” (`usbkeeper guia`, `usbkeeper guia mbr`, painel Ajuda e “?” na janela) e “Remover com segurança” (botão, apelido `remover`, o que acontece quando está em uso).

- [ ] **Step 3: Verificar tudo** — `cargo fmt --all --check && cargo clippy -q --workspace --all-targets -- -D warnings && cargo test -q --workspace`; `cd apps/gui && npm test && npm run build && npx tauri build` (pacote 0.5.1: subir `version` no `Cargo.toml` do workspace, `package.json` e `tauri.conf.json`).

- [ ] **Step 4: Commit + push + PR para `dev`** com checklist manual: abrir Ajuda, buscar “exfat”, “?” do Formatar abre `sistemas-de-arquivos`, ⏏ na lista com pendrive montado e uma janela do Dolphin aberta nele → cartão “em uso” → fechar → Tentar de novo → aviso “Pode tirar”.
