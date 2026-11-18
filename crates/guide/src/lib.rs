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
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
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
