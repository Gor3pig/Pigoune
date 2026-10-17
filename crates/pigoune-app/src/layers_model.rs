use pigoune_core::annotations::LayerEntry;

pub fn row_label(
    entry: &LayerEntry,
    kind_label: &str,
    with_text: impl Fn(&str, &str) -> String,
) -> String {
    if let Some(name) = &entry.name {
        return name.clone();
    }
    if entry.kind == "text" && !entry.summary.is_empty() {
        return with_text(kind_label, &entry.summary);
    }
    format!("{kind_label} {}", entry.ordinal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &str, name: Option<&str>, ordinal: u32, summary: &str) -> LayerEntry {
        LayerEntry {
            id: 1,
            kind: kind.to_owned(),
            name: name.map(str::to_owned),
            ordinal,
            summary: summary.to_owned(),
            hidden: false,
            locked: false,
            sealed: false,
        }
    }

    fn joined(kind: &str, text: &str) -> String {
        format!("{kind}: {text}")
    }

    #[test]
    fn an_unnamed_annotation_is_called_by_its_kind_and_rank() {
        assert_eq!(
            row_label(&entry("rectangle", None, 3, ""), "Rectangle", joined),
            "Rectangle 3"
        );
    }

    #[test]
    fn a_text_is_called_by_the_start_of_its_content() {
        assert_eq!(
            row_label(&entry("text", None, 1, "Attention"), "Text", joined),
            "Text: Attention"
        );
        assert_eq!(
            row_label(&entry("text", None, 2, ""), "Text", joined),
            "Text 2"
        );
    }

    #[test]
    fn a_custom_name_always_wins() {
        assert_eq!(
            row_label(
                &entry("text", Some("Titre"), 1, "Attention"),
                "Text",
                joined
            ),
            "Titre"
        );
        assert_eq!(
            row_label(
                &entry("arrow", Some("Flèche rouge"), 4, ""),
                "Arrow",
                joined
            ),
            "Flèche rouge"
        );
    }
}
