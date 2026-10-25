use super::AnnotationDocument;

const SUMMARY_LENGTH: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerEntry {
    pub id: u32,
    pub kind: String,
    pub name: Option<String>,
    pub ordinal: u32,
    pub summary: String,
    pub hidden: bool,
    pub locked: bool,
}

#[must_use]
pub fn layer_entries(document: &AnnotationDocument) -> Vec<LayerEntry> {
    document
        .objects
        .iter()
        .rev()
        .filter_map(|object| {
            let shape = object.shape()?;
            Some(LayerEntry {
                id: object.id,
                kind: shape.kind.clone(),
                name: object.state.name.clone(),
                ordinal: document.ordinal(object.id).unwrap_or(1),
                summary: summary_of(shape.kind.as_str(), shape.text("content")),
                hidden: object.state.hidden,
                locked: object.state.locked,
            })
        })
        .collect()
}

fn summary_of(kind: &str, content: &str) -> String {
    if !matches!(kind, "text" | "bubble") {
        return String::new();
    }
    let first_line = content.lines().next().unwrap_or("").trim();
    let mut short: String = first_line.chars().take(SUMMARY_LENGTH).collect();
    if first_line.chars().count() > SUMMARY_LENGTH {
        short.push('…');
    }
    short
}
