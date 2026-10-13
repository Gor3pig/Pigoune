use std::collections::HashMap;

use super::{AssetId, Library, LibraryError};
use crate::media::VisualHash;

pub const DUPLICATE_DISTANCE: u32 = 6;
pub const SIMILAR_DISTANCE: u32 = 10;
const HASH_BITS: u32 = 64;
const LARGEST_PROPORTION_GAP: f64 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimilarAsset {
    pub asset: AssetId,
    pub distance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimilarSearch {
    Pending,
    NoFingerprint,
    Found(Vec<SimilarAsset>),
}

struct Fingerprint {
    asset: AssetId,
    hash: VisualHash,
    proportion: Option<f64>,
}

impl Library {
    pub fn similar_assets(
        &self,
        id: AssetId,
        max_distance: u32,
    ) -> Result<SimilarSearch, LibraryError> {
        let stored: Option<Option<Vec<u8>>> = self
            .connection
            .query_row(
                "SELECT visual_hash FROM assets WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let Some(stored) = stored else {
            return Ok(SimilarSearch::NoFingerprint);
        };
        let Some(bytes) = stored else {
            return Ok(SimilarSearch::Pending);
        };
        let Some(target) = VisualHash::from_bytes(&bytes) else {
            return Ok(SimilarSearch::NoFingerprint);
        };
        let mut similar: Vec<SimilarAsset> = self
            .fingerprints()?
            .into_iter()
            .filter(|other| other.asset != id)
            .map(|other| SimilarAsset {
                asset: other.asset,
                distance: target.distance(other.hash),
            })
            .filter(|similar| similar.distance <= max_distance)
            .collect();
        similar.sort_by_key(|similar| (similar.distance, similar.asset));
        Ok(SimilarSearch::Found(similar))
    }

    pub fn duplicate_groups(&self, max_distance: u32) -> Result<Vec<Vec<AssetId>>, LibraryError> {
        let fingerprints = self.fingerprints()?;
        let mut groups = Groups::new(fingerprints.len());
        for (first, second) in close_pairs(&fingerprints, max_distance) {
            groups.join(first, second);
        }
        Ok(groups.into_groups(&fingerprints))
    }

    fn fingerprints(&self) -> Result<Vec<Fingerprint>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id, visual_hash, width, height FROM assets
             WHERE trashed_at_unix_ms IS NULL AND length(visual_hash) = ?1
             ORDER BY id",
        )?;
        let rows = statement.query_map([VisualHash::BYTE_LENGTH], |row| {
            Ok((
                row.get::<_, AssetId>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Option<u32>>(2)?,
                row.get::<_, Option<u32>>(3)?,
            ))
        })?;
        let mut fingerprints = Vec::new();
        for row in rows {
            let (asset, bytes, width, height) = row?;
            let Some(hash) = VisualHash::from_bytes(&bytes) else {
                continue;
            };
            let proportion = width
                .zip(height)
                .filter(|(width, height)| *width > 0 && *height > 0)
                .map(|(width, height)| f64::from(width) / f64::from(height));
            fingerprints.push(Fingerprint {
                asset,
                hash,
                proportion,
            });
        }
        Ok(fingerprints)
    }
}

fn similar_proportions(first: Option<f64>, second: Option<f64>) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => {
            first.max(second) / first.min(second) <= LARGEST_PROPORTION_GAP
        }
        _ => true,
    }
}

fn close_pairs(fingerprints: &[Fingerprint], max_distance: u32) -> Vec<(usize, usize)> {
    let bands = max_distance + 1;
    let mut pairs = Vec::new();
    for band in 0..bands {
        let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
        for (index, fingerprint) in fingerprints.iter().enumerate() {
            buckets
                .entry(band_of(fingerprint.hash.bits(), band, bands))
                .or_default()
                .push(index);
        }
        for members in buckets.values() {
            for (position, first) in members.iter().enumerate() {
                for second in &members[position + 1..] {
                    let (a, b) = (&fingerprints[*first], &fingerprints[*second]);
                    if a.hash.distance(b.hash) <= max_distance
                        && similar_proportions(a.proportion, b.proportion)
                    {
                        pairs.push((*first, *second));
                    }
                }
            }
        }
    }
    pairs
}

fn band_of(bits: u64, band: u32, bands: u32) -> u64 {
    let mut value = 0_u64;
    for position in (band..HASH_BITS).step_by(bands as usize) {
        value = (value << 1) | ((bits >> position) & 1);
    }
    value
}

struct Groups {
    parent: Vec<usize>,
}

impl Groups {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
        }
    }

    fn root(&mut self, mut index: usize) -> usize {
        while self.parent[index] != index {
            self.parent[index] = self.parent[self.parent[index]];
            index = self.parent[index];
        }
        index
    }

    fn join(&mut self, first: usize, second: usize) {
        let (first, second) = (self.root(first), self.root(second));
        self.parent[first.max(second)] = first.min(second);
    }

    fn into_groups(mut self, fingerprints: &[Fingerprint]) -> Vec<Vec<AssetId>> {
        let mut by_root: HashMap<usize, Vec<AssetId>> = HashMap::new();
        for (index, fingerprint) in fingerprints.iter().enumerate() {
            let root = self.root(index);
            by_root.entry(root).or_default().push(fingerprint.asset);
        }
        let mut groups: Vec<Vec<AssetId>> = by_root
            .into_values()
            .filter(|group| group.len() > 1)
            .collect();
        groups.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
        groups
    }
}
