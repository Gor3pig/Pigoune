use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::{Asset, AssetId};

#[derive(Debug, Default)]
pub(super) struct QueryCache {
    stamp: Option<u64>,
    pub all: Option<Arc<Vec<Asset>>>,
    pub trash: Option<Arc<Vec<Asset>>>,
    pub tags: Option<Arc<HashMap<AssetId, Vec<String>>>>,
    pub texts: HashMap<AssetId, String>,
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
    pub fn lock(&self, stamp: u64) -> MutexGuard<'_, QueryCache> {
        let mut cache = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        cache.follow(stamp);
        cache
    }
}
