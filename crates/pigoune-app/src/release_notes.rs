const FRENCH: &str = "fr";
const LANGUAGE_ATTRIBUTE: &str = "xml:lang=\"";

#[derive(Debug, PartialEq, Eq)]
enum Piece {
    Text {
        tag: String,
        language: Option<String>,
        content: String,
    },
    Marker(String),
}

#[must_use]
pub fn is_french(language_names: &[String]) -> bool {
    language_names
        .first()
        .is_some_and(|name| name.starts_with(FRENCH))
}

#[must_use]
pub fn release_notes(metainfo: &str, version: &str, french: bool) -> Option<String> {
    let release = &metainfo[metainfo.find(&format!("<release version=\"{version}\""))?..];
    let release = &release[..release.find("</release>")?];
    let description = &release[release.find("<description>")? + "<description>".len()..];
    let description = &description[..description.find("</description>")?];
    let pieces = pieces_of(description);
    let mut notes = String::new();
    let mut index = 0;
    while let Some(piece) = pieces.get(index) {
        match piece {
            Piece::Marker(tag) => {
                for part in ["<", tag, ">"] {
                    notes.push_str(part);
                }
            }
            Piece::Text {
                tag,
                language: None,
                content,
            } => {
                let translation = pieces.get(index + 1).and_then(|next| match next {
                    Piece::Text {
                        language: Some(language),
                        content,
                        ..
                    } if language == FRENCH => Some(content),
                    _ => None,
                });
                let chosen = if french {
                    translation.unwrap_or(content)
                } else {
                    content
                };
                for part in ["<", tag, ">", chosen, "</", tag, ">"] {
                    notes.push_str(part);
                }
            }
            Piece::Text { .. } => {}
        }
        index += 1;
    }
    (!notes.is_empty()).then_some(notes)
}

fn pieces_of(description: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut rest = description;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>').map(|end| start + end) else {
            break;
        };
        let tag = &rest[start + 1..end];
        let name = tag.split_whitespace().next().unwrap_or_default().to_owned();
        rest = &rest[end + 1..];
        if name == "p" || name == "li" {
            let closing = format!("</{name}>");
            let Some(close) = rest.find(&closing) else {
                break;
            };
            pieces.push(Piece::Text {
                language: language_of(tag),
                content: rest[..close].to_owned(),
                tag: name,
            });
            rest = &rest[close + closing.len()..];
        } else if matches!(name.as_str(), "ul" | "/ul" | "ol" | "/ol") {
            pieces.push(Piece::Marker(name));
        }
    }
    pieces
}

fn language_of(tag: &str) -> Option<String> {
    let start = tag.find(LANGUAGE_ATTRIBUTE)? + LANGUAGE_ATTRIBUTE.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_owned())
}

#[cfg(test)]
mod tests {
    use super::{is_french, release_notes};

    const METAINFO: &str = r#"<releases>
    <release version="2.0.0" date="2026-10-06">
      <description>
        <p>A fresh look.</p>
        <p xml:lang="fr">Un nouveau visage.</p>
        <ul>
          <li>Colors &amp; icons.</li>
          <li xml:lang="fr">Couleurs &amp; icônes.</li>
          <li>Only in English.</li>
        </ul>
      </description>
    </release>
    <release version="1.6.1" date="2026-10-05">
      <description>
        <p>Older notes.</p>
      </description>
    </release>
  </releases>"#;

    #[test]
    fn french_notes_use_the_french_sentences() {
        assert_eq!(
            release_notes(METAINFO, "2.0.0", true).as_deref(),
            Some(
                "<p>Un nouveau visage.</p><ul><li>Couleurs &amp; icônes.</li><li>Only in English.</li></ul>"
            )
        );
    }

    #[test]
    fn other_languages_get_the_english_notes() {
        assert_eq!(
            release_notes(METAINFO, "2.0.0", false).as_deref(),
            Some(
                "<p>A fresh look.</p><ul><li>Colors &amp; icons.</li><li>Only in English.</li></ul>"
            )
        );
    }

    #[test]
    fn only_the_asked_version_is_read() {
        assert_eq!(
            release_notes(METAINFO, "1.6.1", false).as_deref(),
            Some("<p>Older notes.</p>")
        );
        assert_eq!(release_notes(METAINFO, "9.9.9", false), None);
    }

    #[test]
    fn french_is_the_first_language_pigoune_uses() {
        assert!(is_french(&[
            "fr_FR.UTF-8".to_owned(),
            "fr".to_owned(),
            "C".to_owned()
        ]));
        assert!(is_french(&["fr".to_owned(), "C".to_owned()]));
        assert!(!is_french(&["de".to_owned(), "C".to_owned()]));
        assert!(!is_french(&["en".to_owned(), "fr".to_owned()]));
    }
}
