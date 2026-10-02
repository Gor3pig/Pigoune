use std::cmp::Ordering;

use pigoune_core::{Asset, AssetFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortCriterion {
    #[default]
    Added,
    Name,
    Type,
    Dimensions,
    Size,
}

impl SortCriterion {
    pub fn from_setting(value: &str) -> Self {
        match value {
            "name" => Self::Name,
            "type" => Self::Type,
            "dimensions" => Self::Dimensions,
            "size" => Self::Size,
            _ => Self::Added,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SortOrder {
    pub criterion: SortCriterion,
    pub reversed: bool,
}

impl SortOrder {
    pub fn compare<Key: Ord + ?Sized>(
        self,
        first: &SortedAsset<Key>,
        second: &SortedAsset<Key>,
    ) -> Ordering {
        let natural = match self.criterion {
            SortCriterion::Added => Ordering::Equal,
            SortCriterion::Name => first.name_key.cmp(second.name_key),
            SortCriterion::Type => {
                format_rank(first.asset.format).cmp(&format_rank(second.asset.format))
            }
            SortCriterion::Dimensions => area(second.asset).cmp(&area(first.asset)),
            SortCriterion::Size => second.asset.byte_size.cmp(&first.asset.byte_size),
        };
        let ordering = natural.then_with(|| newest_first(first.asset, second.asset));
        if self.reversed {
            ordering.reverse()
        } else {
            ordering
        }
    }
}

pub struct SortedAsset<'a, Key: Ord + ?Sized> {
    pub asset: &'a Asset,
    pub name_key: &'a Key,
}

fn newest_first(first: &Asset, second: &Asset) -> Ordering {
    second
        .added_at_unix_ms
        .cmp(&first.added_at_unix_ms)
        .then_with(|| second.id.cmp(&first.id))
}

fn area(asset: &Asset) -> u64 {
    asset.dimensions.map_or(0, |dimensions| {
        u64::from(dimensions.width()) * u64::from(dimensions.height())
    })
}

fn format_rank(format: AssetFormat) -> usize {
    AssetFormat::ALL
        .iter()
        .position(|candidate| *candidate == format)
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Asset, AssetFormat, AssetId, Dimensions};

    use super::{SortCriterion, SortOrder, SortedAsset};

    fn asset(name: &str, format: AssetFormat, side: u32, byte_size: u64, added: i64) -> Asset {
        Asset {
            id: AssetId::parse(&format!("00000000-0000-7000-8000-{added:012}")).expect("valid id"),
            display_name: name.to_owned(),
            original_file_name: name.to_owned(),
            stored_path: name.into(),
            format,
            dimensions: Dimensions::new(side, side),
            byte_size,
            content_hash: String::new(),
            is_animated: false,
            embedded_sizes: Vec::new(),
            added_at_unix_ms: added,
            trashed_at_unix_ms: None,
        }
    }

    fn sorted_names(assets: &[Asset], keys: &[&str], order: SortOrder) -> Vec<String> {
        let mut entries: Vec<SortedAsset<str>> = assets
            .iter()
            .zip(keys)
            .map(|(asset, name_key)| SortedAsset {
                asset,
                name_key: *name_key,
            })
            .collect();
        entries.sort_by(|first, second| order.compare(first, second));
        entries
            .iter()
            .map(|entry| entry.asset.display_name.clone())
            .collect()
    }

    fn samples() -> Vec<Asset> {
        vec![
            asset("b", AssetFormat::Png, 64, 300, 1),
            asset("a", AssetFormat::Svg, 16, 100, 2),
            asset("c", AssetFormat::Gif, 256, 200, 3),
        ]
    }

    fn order(criterion: SortCriterion, reversed: bool) -> SortOrder {
        SortOrder {
            criterion,
            reversed,
        }
    }

    #[test]
    fn every_criterion_has_its_natural_direction() {
        let assets = samples();
        let keys = ["b", "a", "c"];
        let by = |criterion| sorted_names(&assets, &keys, order(criterion, false));
        assert_eq!(by(SortCriterion::Added), ["c", "a", "b"]);
        assert_eq!(by(SortCriterion::Name), ["a", "b", "c"]);
        assert_eq!(by(SortCriterion::Type), ["a", "b", "c"]);
        assert_eq!(by(SortCriterion::Dimensions), ["c", "b", "a"]);
        assert_eq!(by(SortCriterion::Size), ["b", "c", "a"]);
    }

    #[test]
    fn the_reverse_order_flips_the_natural_direction() {
        let assets = samples();
        let keys = ["b", "a", "c"];
        assert_eq!(
            sorted_names(&assets, &keys, order(SortCriterion::Name, true)),
            ["c", "b", "a"]
        );
        assert_eq!(
            sorted_names(&assets, &keys, order(SortCriterion::Added, true)),
            ["b", "a", "c"]
        );
    }

    #[test]
    fn ties_are_broken_by_newest_first() {
        let assets = vec![
            asset("old", AssetFormat::Png, 64, 100, 1),
            asset("new", AssetFormat::Png, 64, 100, 2),
        ];
        let keys = ["same", "same"];
        assert_eq!(
            sorted_names(&assets, &keys, order(SortCriterion::Type, false)),
            ["new", "old"]
        );
    }

    #[test]
    fn numbers_inside_names_are_compared_by_value() {
        let names = ["logo 10", "logo 2", "logo 1"];
        let assets: Vec<Asset> = names
            .iter()
            .zip(1..)
            .map(|(name, added)| asset(name, AssetFormat::Png, 64, 100, added))
            .collect();
        let keys: Vec<gtk::glib::FilenameCollationKey> = names
            .iter()
            .map(gtk::glib::FilenameCollationKey::from)
            .collect();
        let mut entries: Vec<SortedAsset<gtk::glib::FilenameCollationKey>> = assets
            .iter()
            .zip(&keys)
            .map(|(asset, name_key)| SortedAsset { asset, name_key })
            .collect();
        entries.sort_by(|first, second| order(SortCriterion::Name, false).compare(first, second));
        let sorted: Vec<&str> = entries
            .iter()
            .map(|entry| entry.asset.display_name.as_str())
            .collect();
        assert_eq!(sorted, ["logo 1", "logo 2", "logo 10"]);
    }

    #[test]
    fn unknown_settings_fall_back_to_the_date_added() {
        assert_eq!(SortCriterion::from_setting("bogus"), SortCriterion::Added);
        assert_eq!(SortCriterion::from_setting("size"), SortCriterion::Size);
    }
}
