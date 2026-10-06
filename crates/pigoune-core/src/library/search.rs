use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use super::{Asset, AssetId, AssetView, Library, LibraryError};
use crate::media::{AssetColor, AssetFormat, AssetShape, Dimensions, Rgb};

const FIELD_SEPARATOR: char = '\n';
const ALTERNATIVE_SEPARATOR: char = ',';
pub const MAX_QUERY_WORDS: usize = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetFilter {
    pub text: String,
    pub formats: Vec<AssetFormat>,
    pub favorites_only: bool,
    pub colors: Vec<AssetColor>,
    pub custom_color: Option<Rgb>,
    pub shapes: Vec<AssetShape>,
    pub fits_screen: bool,
    pub screen: Option<Dimensions>,
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
        self.formats.len()
            + self.colors.len()
            + usize::from(self.custom_color.is_some())
            + usize::from(self.favorites_only)
            + self.shapes.len()
            + usize::from(self.fits_screen)
    }

    #[must_use]
    pub fn narrows(&self) -> bool {
        self.chosen_filters() > 0 || !search_groups(&self.text).is_empty()
    }

    fn keeps(&self, asset: &Asset, groups: &[Vec<String>], tags: &[String]) -> bool {
        (self.formats.is_empty() || self.formats.contains(&asset.format))
            && (!self.favorites_only || asset.is_favorite)
            && self.keeps_colors_of(asset)
            && self.keeps_shape_of(asset)
            && self.keeps_size_of(asset)
            && (groups.is_empty() || {
                let text = searchable_text(asset, tags);
                groups
                    .iter()
                    .any(|words| words.iter().all(|word| text.contains(word.as_str())))
            })
    }
}

impl AssetFilter {
    fn keeps_shape_of(&self, asset: &Asset) -> bool {
        self.shapes.is_empty()
            || asset
                .dimensions
                .is_some_and(|size| self.shapes.contains(&AssetShape::of(size)))
    }

    fn keeps_size_of(&self, asset: &Asset) -> bool {
        let Some(screen) = self.screen.filter(|_| self.fits_screen) else {
            return true;
        };
        asset.format != AssetFormat::Svg
            && asset.dimensions.is_some_and(|size| {
                size.width() >= screen.width() && size.height() >= screen.height()
            })
    }

    fn keeps_colors_of(&self, asset: &Asset) -> bool {
        if self.colors.is_empty() && self.custom_color.is_none() {
            return true;
        }
        asset.colors.iter().any(|color| {
            self.colors.contains(&color.family)
                || self
                    .custom_color
                    .is_some_and(|custom| custom.is_close_to(color.average))
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
        let filter = &AssetFilter {
            screen: filter.screen.or(self.screen),
            ..filter.clone()
        };
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
    let mut remaining = MAX_QUERY_WORDS;
    all_query_groups(query)
        .into_iter()
        .map(|words| {
            let kept: Vec<String> = words.into_iter().take(remaining).collect();
            remaining -= kept.len();
            kept
        })
        .filter(|words| !words.is_empty())
        .collect()
}

#[must_use]
pub fn query_word_count(query: &str) -> usize {
    all_query_groups(query).iter().map(Vec::len).sum()
}

fn all_query_groups(query: &str) -> Vec<Vec<String>> {
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
    use super::{
        MAX_QUERY_WORDS, comparable, query_groups, query_text, query_word_count, search_groups,
        search_words,
    };

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
    fn only_the_first_words_of_a_long_query_are_kept() {
        let query = "un deux trois, quatre cinq six, sept huit neuf dix";
        let kept: usize = query_groups(query).iter().map(Vec::len).sum();
        assert_eq!(kept, MAX_QUERY_WORDS);
        assert_eq!(query_word_count(query), 10);
        assert_eq!(
            query_groups(query).last().map(Vec::as_slice),
            Some(["sept".to_owned(), "huit".to_owned()].as_slice())
        );
    }

    #[test]
    fn commas_separate_alternatives() {
        assert_eq!(
            search_groups("logo rouge, chèvre,, "),
            [vec!["logo", "rouge"], vec!["chevre"]]
        );
    }
}
