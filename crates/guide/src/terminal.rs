//! Markdown → texto de terminal: títulos sublinhados, listas com "•",
//! tabelas alinhadas e nenhuma sintaxe Markdown visível.

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

const WIDTH: usize = 88;

/// Quebra `text` em linhas de até WIDTH colunas; `first` prefixa a primeira
/// linha e `indent` as seguintes.
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

fn table_text(rows: &[Vec<String>]) -> String {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .filter_map(|r| r.get(c))
                .map(|s| s.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(c, cell)| format!("{cell:<w$}", w = widths[c]))
            .collect();
        out.push_str(&format!("  {}\n", cells.join("  ").trim_end()));
        if i == 0 {
            let total = widths.iter().sum::<usize>() + 2 * cols.saturating_sub(1);
            out.push_str(&format!("  {}\n", "-".repeat(total)));
        }
    }
    out.push('\n');
    out
}

pub fn to_terminal(md: &str) -> String {
    let mut out = String::new();
    let mut buf = String::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut heading: Option<HeadingLevel> = None;
    let mut table: Vec<Vec<String>> = Vec::new();
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
                    Some(HeadingLevel::H2) => {
                        out.push_str(&format!("{text}\n{}\n\n", "-".repeat(text.chars().count())));
                    }
                    _ => out.push_str(&format!("▸ {text}\n\n")),
                }
            }
            Event::Start(Tag::List(start)) => {
                // Lista aninhada: o texto do item pai já vai para a saída.
                if !buf.trim().is_empty() && !lists.is_empty() {
                    flush_item(&mut out, &mut buf, &mut lists);
                }
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                if lists.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Item) => buf.clear(),
            Event::End(TagEnd::Item) => {
                if !buf.trim().is_empty() {
                    flush_item(&mut out, &mut buf, &mut lists);
                }
                buf.clear();
            }
            Event::End(TagEnd::Paragraph) if !lists.is_empty() => buf.push(' '),
            Event::End(TagEnd::Paragraph) => {
                out.push_str(&wrap(buf.trim(), "", ""));
                out.push_str("\n\n");
                buf.clear();
            }
            Event::End(TagEnd::DefinitionListTitle) => {
                out.push_str(buf.trim());
                out.push('\n');
                buf.clear();
            }
            Event::End(TagEnd::DefinitionListDefinition) => {
                out.push_str(&wrap(buf.trim(), "    ", "    "));
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
            Event::Start(Tag::Table(_)) => table.clear(),
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => table.push(Vec::new()),
            Event::End(TagEnd::TableCell) => {
                if let Some(row) = table.last_mut() {
                    row.push(buf.trim().to_string());
                }
                buf.clear();
            }
            Event::End(TagEnd::Table) => out.push_str(&table_text(&table)),
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
    }
    out.trim_end().to_string() + "\n"
}

fn flush_item(out: &mut String, buf: &mut String, lists: &mut [Option<u64>]) {
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
