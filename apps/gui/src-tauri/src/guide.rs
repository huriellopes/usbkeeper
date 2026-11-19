//! Guia de ajuda para a interface (HTML gerado do Markdown embutido).

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

#[cfg(test)]
mod tests {
    #[test]
    fn exposes_all_topics_as_html() {
        let topics = super::guide_topics();
        assert_eq!(topics.len(), 12);
        assert!(topics.iter().all(|t| t.html.starts_with("<h1>")));
    }
}
