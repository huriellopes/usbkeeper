//! Guia de ajuda: um arquivo Markdown por tópico, embutido no binário e
//! mostrado igual na CLI (texto) e na interface (HTML).

mod terminal;

use pulldown_cmark::{Event, Options, Parser, html};

pub use terminal::to_terminal;

macro_rules! sources {
    ($($id:literal),* $(,)?) => {
        /// Ids estáveis (usados pelos links "?" da interface e pela CLI).
        pub const IDS: [&str; 12] = [$($id),*];
        const SOURCES: [(&str, &str); 12] =
            [$(($id, include_str!(concat!("../topics/", $id, ".md")))),*];
    };
}

sources!(
    "comecar",
    "tipos",
    "usb",
    "mbr-gpt",
    "sistemas-de-arquivos",
    "formatar",
    "falsificados",
    "saude",
    "velocidade",
    "remover",
    "protecoes",
    "glossario",
);

const ALIASES: [(&str, &str); 12] = [
    ("mbr", "mbr-gpt"),
    ("gpt", "mbr-gpt"),
    ("fat32", "sistemas-de-arquivos"),
    ("exfat", "sistemas-de-arquivos"),
    ("ntfs", "sistemas-de-arquivos"),
    ("ext4", "sistemas-de-arquivos"),
    ("fs", "sistemas-de-arquivos"),
    ("fake", "falsificados"),
    ("falso", "falsificados"),
    ("smart", "saude"),
    ("ejetar", "remover"),
    ("eject", "remover"),
];

#[derive(Debug, Clone)]
pub struct Topic {
    pub id: &'static str,
    pub title: String,
    pub summary: String,
    pub markdown: &'static str,
}

/// Primeira linha `# Título`, segunda linha não vazia = resumo.
fn parse(id: &'static str, markdown: &'static str) -> Topic {
    let mut lines = markdown.lines().map(str::trim).filter(|l| !l.is_empty());
    let title = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches('#')
        .trim()
        .to_string();
    let summary = lines.next().unwrap_or_default().to_string();
    Topic {
        id,
        title,
        summary,
        markdown,
    }
}

pub fn topics() -> Vec<Topic> {
    SOURCES.iter().map(|(id, md)| parse(id, md)).collect()
}

/// Busca por id ou apelido (sem diferenciar maiúsculas).
pub fn find(query: &str) -> Option<Topic> {
    let q = query.trim().to_lowercase();
    let id = ALIASES
        .iter()
        .find(|(alias, _)| *alias == q)
        .map(|(_, id)| *id)
        .unwrap_or(q.as_str());
    SOURCES
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(i, md)| parse(i, md))
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_DEFINITION_LIST
}

/// HTML do tópico. HTML bruto escrito no Markdown é descartado.
pub fn to_html(md: &str) -> String {
    let events = Parser::new_ext(md, options())
        .filter(|e| !matches!(e, Event::Html(_) | Event::InlineHtml(_)));
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

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
        for (alias, id) in [
            ("mbr", "mbr-gpt"),
            ("GPT", "mbr-gpt"),
            ("exfat", "sistemas-de-arquivos"),
            ("fake", "falsificados"),
            ("smart", "saude"),
            ("eject", "remover"),
            ("remover", "remover"),
        ] {
            assert_eq!(find(alias).map(|t| t.id), Some(id), "{alias}");
        }
        assert!(find("nada").is_none());
    }

    #[test]
    fn html_has_tables_and_drops_raw_html() {
        let html =
            to_html("| a | b |\n|---|---|\n| 1 | 2 |\n\n<script>x</script>\n\ntexto <b>cru</b>");
        assert!(html.contains("<table>"));
        assert!(!html.contains("<script>") && !html.contains("<b>"));
    }

    /// Fatos obrigatórios por tópico: o texto pode variar, estes pontos não.
    const MUST: [(&str, &[&str]); 12] = [
        ("comecar", &["backup", "disco do sistema", "série", "senha"]),
        (
            "tipos",
            &[
                "pendrive",
                "cartão",
                "HD externo",
                "SSD",
                "NVMe",
                "SMART",
                "gaveta",
            ],
        ),
        (
            "usb",
            &["USB 2.0", "USB 3", "480", "5 Gb/s", "cabo", "hub", "porta"],
        ),
        (
            "mbr-gpt",
            &["MBR", "GPT", "2 TiB", "4 partições", "UEFI", "BIOS", "TV"],
        ),
        (
            "sistemas-de-arquivos",
            &[
                "FAT32",
                "exFAT",
                "NTFS",
                "ext4",
                "4 GB",
                "macOS",
                "journaling",
            ],
        ),
        (
            "formatar",
            &[
                "índice",
                "recuper",
                "apagar com segurança",
                "antes de formatar",
            ],
        ),
        (
            "falsificados",
            &[
                "volta",
                "descart",
                "rápido",
                "completo",
                "--max-size",
                "reembolso",
            ],
        ),
        (
            "saude",
            &[
                "realocados",
                "pendentes",
                "incorrigíveis",
                "CRC",
                "pendrive",
                "varredura",
                "trocar",
            ],
        ),
        ("velocidade", &["MB/s", "IOPS", "4K", "sequencial", "cache"]),
        (
            "remover",
            &[
                "cache",
                "em uso",
                "gerenciador de arquivos",
                "LED",
                "Remover com segurança",
            ],
        ),
        (
            "protecoes",
            &["disco do sistema", "USB", "série", "O_EXCL", "restaura"],
        ),
        (
            "glossario",
            &["Setor", "Partição", "Montar", "Firmware", "TRIM"],
        ),
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

    #[test]
    fn every_gui_help_link_points_to_an_existing_topic() {
        let ts = include_str!("../../../apps/gui/src/lib/guideLinks.ts");
        let ids: Vec<&str> = ts.split('"').skip(1).step_by(2).collect();
        assert!(!ids.is_empty());
        for id in ids {
            assert!(
                IDS.contains(&id),
                "guideLinks.ts aponta para tópico inexistente: {id}"
            );
        }
    }

    #[test]
    fn glossary_renders_as_definition_list() {
        let md = find("glossario").unwrap().markdown;
        assert!(to_html(md).contains("<dl>"));
        let txt = to_terminal(md);
        assert!(txt.contains("Setor\n    A menor unidade"), "{txt}");
        assert!(
            !txt.lines().any(|l| l.trim_start().starts_with(": ")),
            "{txt}"
        );
    }

    #[test]
    fn real_topics_render_without_markdown_leftovers() {
        for t in topics() {
            let txt = to_terminal(t.markdown);
            assert!(
                !txt.contains("**") && !txt.contains('|') && !txt.contains("\n#"),
                "{}:\n{txt}",
                t.id
            );
        }
    }

    #[test]
    fn terminal_has_no_markdown_syntax() {
        let md = "# Título\n\nResumo **forte**.\n\n## Seção\n\n- item `código`\n\n| Coluna | Outra |\n|---|---|\n| a | bb |\n";
        let txt = to_terminal(md);
        assert!(txt.starts_with("TÍTULO\n======"), "{txt}");
        assert!(txt.contains("• item código"), "{txt}");
        assert!(
            !txt.contains("**") && !txt.contains("# ") && !txt.contains('|'),
            "{txt}"
        );
        assert!(txt.contains("Coluna  Outra"), "{txt}");
    }
}
