use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use super::{Asset, AssetId, AssetView, Library, LibraryError};

const FIELD_SEPARATOR: char = '\n';

impl Library {
    pub fn search_assets_in(
        &self,
        view: AssetView,
        query: &str,
    ) -> Result<Vec<Asset>, LibraryError> {
        let words = search_words(query);
        let assets = self.visible_assets_in(view)?;
        if words.is_empty() {
            return Ok(assets);
        }
        let tags = self.tag_names_by_asset()?;
        Ok(assets
            .into_iter()
            .filter(|asset| {
                let text = searchable_text(asset, tags.get(&asset.id));
                words.iter().all(|word| text.contains(word.as_str()))
            })
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

fn search_words(query: &str) -> Vec<String> {
    query.split_whitespace().map(comparable).collect()
}

fn searchable_text(asset: &Asset, tags: Option<&Vec<String>>) -> String {
    let fields = [
        asset.display_name.as_str(),
        asset.note.as_str(),
        asset.source_url.as_str(),
        asset.license.as_str(),
        asset.author.as_str(),
    ];
    let tags = tags.into_iter().flatten().map(String::as_str);
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
    use super::{comparable, search_words};

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
}
