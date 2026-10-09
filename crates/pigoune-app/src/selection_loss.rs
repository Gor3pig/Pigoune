use pigoune_core::AssetId;

pub fn lost_count(before: &[AssetId], after: &[AssetId]) -> usize {
    before.iter().filter(|id| !after.contains(id)).count()
}

#[cfg(test)]
mod tests {
    use pigoune_core::AssetId;

    use super::lost_count;

    fn id(number: u8) -> AssetId {
        AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
    }

    #[test]
    fn nothing_is_lost_when_every_selected_asset_is_still_there() {
        assert_eq!(lost_count(&[id(1), id(2)], &[id(2), id(1)]), 0);
        assert_eq!(lost_count(&[], &[]), 0);
    }

    #[test]
    fn the_assets_missing_afterwards_are_counted() {
        assert_eq!(lost_count(&[id(1), id(2), id(3)], &[id(2)]), 2);
        assert_eq!(lost_count(&[id(1), id(2)], &[]), 2);
    }

    #[test]
    fn assets_selected_only_afterwards_are_not_lost() {
        assert_eq!(lost_count(&[id(1)], &[id(1), id(9)]), 0);
    }
}
