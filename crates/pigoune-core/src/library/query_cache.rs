use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::{Asset, AssetId, CollectionId, TagId};

#[derive(Debug, Default)]
pub(super) struct QueryCache {
    stamp: Option<u64>,
    pub all: Option<Arc<Vec<Asset>>>,
    pub trash: Option<Arc<Vec<Asset>>>,
    pub tags: Option<Arc<HashMap<AssetId, Vec<String>>>>,
    pub texts: HashMap<AssetId, String>,
    pub tag_counts: Option<Arc<Counted<TagId>>>,
    pub collection_counts: Option<Arc<Counted<CollectionId>>>,
}

#[derive(Debug)]
pub(super) struct Counted<K> {
    pub rows: (usize, usize, usize),
    pub counts: HashMap<K, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AssetRows {
    pub visible: usize,
    pub trashed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LinkRows {
    pub tags: usize,
    pub asset_tags: usize,
    pub collections: usize,
    pub asset_collections: usize,
    pub trashed_assets: usize,
}

impl LinkRows {
    pub fn behind_tag_counts(self) -> (usize, usize, usize) {
        (self.tags, self.asset_tags, self.trashed_assets)
    }

    pub fn behind_collection_counts(self) -> (usize, usize, usize) {
        (
            self.collections,
            self.asset_collections,
            self.trashed_assets,
        )
    }
}

fn newest_first(asset: &Asset) -> (Reverse<i64>, Reverse<AssetId>) {
    (Reverse(asset.added_at_unix_ms), Reverse(asset.id))
}

fn merged_by_key<T, K: Ord>(known: Vec<T>, mut imported: Vec<T>, key: impl Fn(&T) -> K) -> Vec<T> {
    imported.sort_by_key(&key);
    let mut merged = Vec::with_capacity(known.len() + imported.len());
    let mut known = known.into_iter().peekable();
    let mut imported = imported.into_iter().peekable();
    loop {
        let take_known = match (known.peek(), imported.peek()) {
            (Some(old), Some(new)) => key(old) <= key(new),
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => break,
        };
        merged.extend(if take_known {
            known.next()
        } else {
            imported.next()
        });
    }
    merged
}

fn imported_ones_are_new(known: &[Asset], imported: &[Asset]) -> bool {
    let incoming: HashSet<AssetId> = imported.iter().map(|asset| asset.id).collect();
    incoming.len() == imported.len() && !known.iter().any(|asset| incoming.contains(&asset.id))
}

impl QueryCache {
    fn follow(&mut self, stamp: u64) {
        if self.stamp != Some(stamp) {
            *self = Self::default();
            self.stamp = Some(stamp);
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct SharedQueryCache(Mutex<QueryCache>);

impl SharedQueryCache {
    pub fn carry(&self, before: u64, after: u64) {
        let mut cache = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.stamp == Some(before) {
            cache.stamp = Some(after);
            cache.tag_counts = None;
            cache.collection_counts = None;
        }
    }

    pub fn adopt(
        &self,
        before: u64,
        after: u64,
        imported: Vec<Asset>,
        rows: AssetRows,
        links: LinkRows,
    ) -> bool {
        let mut cache = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.stamp != Some(before) {
            return false;
        }
        if let Some(known) = cache.all.as_ref()
            && (known.len() + imported.len() != rows.visible
                || !imported_ones_are_new(known, &imported))
        {
            return false;
        }
        if cache
            .trash
            .as_ref()
            .is_some_and(|trash| trash.len() != rows.trashed)
        {
            return false;
        }
        if let Some(known) = cache.all.take() {
            let known = Arc::try_unwrap(known).unwrap_or_else(|shared| (*shared).clone());
            cache.all = Some(Arc::new(merged_by_key(known, imported, newest_first)));
        }
        if cache
            .tag_counts
            .as_ref()
            .is_some_and(|known| known.rows != links.behind_tag_counts())
        {
            cache.tag_counts = None;
        }
        if cache
            .collection_counts
            .as_ref()
            .is_some_and(|known| known.rows != links.behind_collection_counts())
        {
            cache.collection_counts = None;
        }
        cache.stamp = Some(after);
        true
    }

    pub fn lock(&self, stamp: u64) -> MutexGuard<'_, QueryCache> {
        let mut cache = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        cache.follow(stamp);
        cache
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Reverse;

    use super::merged_by_key;

    fn newest_first(entry: &(i64, u32)) -> (Reverse<i64>, Reverse<u32>) {
        (Reverse(entry.0), Reverse(entry.1))
    }

    #[test]
    fn imported_entries_take_their_place_by_date_then_identifier_not_the_front() {
        let known = vec![(90, 1), (50, 9), (50, 3), (10, 7)];
        let imported = vec![(50, 5), (95, 2), (5, 4), (50, 8)];

        let merged = merged_by_key(known, imported, newest_first);

        assert_eq!(
            merged,
            vec![
                (95, 2),
                (90, 1),
                (50, 9),
                (50, 8),
                (50, 5),
                (50, 3),
                (10, 7),
                (5, 4)
            ]
        );
    }

    #[test]
    fn an_empty_side_gives_the_other_side_unchanged() {
        assert_eq!(
            merged_by_key(vec![(3, 1), (2, 1)], Vec::new(), newest_first),
            vec![(3, 1), (2, 1)]
        );
        assert_eq!(
            merged_by_key(Vec::new(), vec![(2, 1), (3, 1)], newest_first),
            vec![(3, 1), (2, 1)]
        );
    }
}
