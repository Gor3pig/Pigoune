use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use super::{Asset, AssetId, AssetView, Library, LibraryError};
use crate::media::AssetFormat;

const FIELD_SEPARATOR: char = '\n';
const ALTERNATIVE_SEPARATOR: char = ',';

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetFilter {
    pub text: String,
    pub formats: Vec<AssetFormat>,
    pub favorites_only: bool,
}

impl AssetFilter {
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn chosen_filters(&self) -> usize {
        self.formats.len() + usize::from(self.favorites_only)
    }

    #[must_use]
    pub fn narrows(&self) -> bool {
        self.chosen_filters() > 0 || !search_groups(&self.text).is_empty()
    }

    fn keeps(&self, asset: &Asset, groups: &[Vec<String>], tags: &[String]) -> bool {
        (self.formats.is_empty() || self.formats.contains(&asset.format))
            && (!self.favorites_only || asset.is_favorite)
            && (groups.is_empty() || {
                let text = searchable_text(asset, tags);
                groups
                    .iter()
                    .any(|words| words.iter().all(|word| text.contains(word.as_str())))
            })
    }
}

impl Library {
    pub fn find_assets_in(
        &self,
        view: AssetView,
        filter: &AssetFilter,
    ) -> Result<Vec<Asset>, LibraryError> {
        let assets = self.visible_assets_in(view)?;
        if !filter.narrows() {
            return Ok(assets);
        }
        let groups = search_groups(&filter.text);
        let tags = self.tag_names_by_asset()?;
        let no_tags = Vec::new();
        Ok(assets
            .into_iter()
            .filter(|asset| filter.keeps(asset, &groups, tags.get(&asset.id).unwrap_or(&no_tags)))
            .collect())
    }

    fn tag_names_by_asset(&self) -> Result<HashMap<AssetId, Vec<String>>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT asset_tags.asset_id, tags.name FROM asset_tags
             JOIN tags ON tags.id = asset_tags.tag_id",
        )?;
        let mut names: HashMap<AssetId, Vec<String>> = HashMap::new();
        for row in statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))? {
            let (asset, name) = row?;
            names.entry(asset).or_default().push(name);
        }
        Ok(names)
    }
}

#[must_use]
pub fn query_groups(query: &str) -> Vec<Vec<String>> {
    query
        .split(ALTERNATIVE_SEPARATOR)
        .map(|group| {
            group
                .split_whitespace()
                .filter(|word| word.chars().any(char::is_alphanumeric))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|words| !words.is_empty())
        .collect()
}

#[must_use]
pub fn query_text(groups: &[Vec<String>]) -> String {
    groups
        .iter()
        .filter(|words| !words.is_empty())
        .map(|words| words.join(" "))
        .collect::<Vec<_>>()
        .join(&format!("{ALTERNATIVE_SEPARATOR} "))
}

fn search_groups(query: &str) -> Vec<Vec<String>> {
    query_groups(query)
        .into_iter()
        .map(|words| words.iter().map(|word| comparable(word)).collect())
        .collect()
}

#[cfg(test)]
fn search_words(query: &str) -> Vec<String> {
    search_groups(&query.replace(ALTERNATIVE_SEPARATOR, " "))
        .into_iter()
        .flatten()
        .collect()
}

fn searchable_text(asset: &Asset, tags: &[String]) -> String {
    let fields = [
        asset.display_name.as_str(),
        asset.note.as_str(),
        asset.source_url.as_str(),
        asset.license.as_str(),
        asset.author.as_str(),
    ];
    let tags = tags.iter().map(String::as_str);
    let mut text = String::new();
    for field in fields.into_iter().chain(tags) {
        text.push_str(&comparable(field));
        text.push(FIELD_SEPARATOR);
    }
    text
}

fn comparable(text: &str) -> String {
    text.nfd()
        .filter(|character| !is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{comparable, query_groups, query_text, search_groups, search_words};

    #[test]
    fn case_and_accents_are_ignored() {
        assert_eq!(comparable("Élève À L'École"), "eleve a l'ecole");
        assert_eq!(comparable("ÇA"), "ca");
    }

    #[test]
    fn a_query_is_split_into_comparable_words() {
        assert_eq!(search_words("  Logo   Rouge "), ["logo", "rouge"]);
        assert!(search_words("   ").is_empty());
    }

    #[test]
    fn lone_symbols_are_not_words() {
        assert_eq!(search_words("logo + chèvre & 2"), ["logo", "chevre", "2"]);
    }

    #[test]
    fn a_query_keeps_its_words_as_typed() {
        assert_eq!(
            query_groups(" Logo + Rouge ,, Chèvre"),
            [vec!["Logo", "Rouge"], vec!["Chèvre"]]
        );
    }

    #[test]
    fn groups_become_a_tidy_query_again() {
        let groups = vec![
            vec!["Logo".to_owned(), "Rouge".to_owned()],
            Vec::new(),
            vec!["Chèvre".to_owned()],
        ];
        assert_eq!(query_text(&groups), "Logo Rouge, Chèvre");
        assert_eq!(
            query_groups(&query_text(&groups)),
            query_groups("Logo Rouge, Chèvre")
        );
        assert_eq!(query_text(&[]), "");
    }

    #[test]
    fn commas_separate_alternatives() {
        assert_eq!(
            search_groups("logo rouge, chèvre,, "),
            [vec!["logo", "rouge"], vec!["chevre"]]
        );
    }
}
