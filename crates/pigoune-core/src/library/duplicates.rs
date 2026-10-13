use std::collections::HashMap;

use rusqlite::OptionalExtension;

use super::{AssetId, Library, LibraryError};
use crate::media::{AssetColor, VisualHash, dominant_from_text};

pub const DUPLICATE_DISTANCE: u32 = 6;
pub const SIMILAR_DISTANCE: u32 = 10;
const HASH_BITS: u32 = 64;
const DUPLICATE_PROPORTION_GAP: f64 = 1.25;
const SIMILAR_PROPORTION_GAP: f64 = 1.5;
const TRUSTED_SHAPE_DISTANCE: u32 = 4;

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
    families: Option<Vec<AssetColor>>,
}

enum Stored {
    Missing,
    Pending,
    Unusable,
    Ready(Fingerprint),
}

impl Library {
    pub fn similar_assets(
        &self,
        id: AssetId,
        max_distance: u32,
    ) -> Result<SimilarSearch, LibraryError> {
        let target = match self.stored_fingerprint(id)? {
            Stored::Missing | Stored::Unusable => return Ok(SimilarSearch::NoFingerprint),
            Stored::Pending => return Ok(SimilarSearch::Pending),
            Stored::Ready(target) => target,
        };
        let mut similar: Vec<SimilarAsset> = self
            .fingerprints()?
            .into_iter()
            .filter(|other| other.asset != id)
            .filter(|other| resembles(&target, other, max_distance, SIMILAR_PROPORTION_GAP))
            .map(|other| SimilarAsset {
                asset: other.asset,
                distance: target.hash.distance(other.hash),
            })
            .collect();
        similar.sort_by_key(|similar| (similar.distance, similar.asset));
        Ok(SimilarSearch::Found(similar))
    }

    pub fn duplicate_groups(&self, max_distance: u32) -> Result<Vec<Vec<AssetId>>, LibraryError> {
        let fingerprints = self.fingerprints()?;
        let mut groups = Groups::new(fingerprints.len());
        join_close_fingerprints(&fingerprints, max_distance, &mut groups);
        Ok(groups.into_groups(&fingerprints))
    }

    fn fingerprints(&self) -> Result<Vec<Fingerprint>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id, visual_hash, width, height, colors FROM assets
             WHERE trashed_at_unix_ms IS NULL AND length(visual_hash) = ?1
             ORDER BY id",
        )?;
        let rows = statement.query_map([VisualHash::BYTE_LENGTH], fingerprint_from_row)?;
        let mut fingerprints = Vec::new();
        for row in rows {
            if let Some(fingerprint) = row? {
                fingerprints.push(fingerprint);
            }
        }
        Ok(fingerprints)
    }

    fn stored_fingerprint(&self, id: AssetId) -> Result<Stored, LibraryError> {
        let row = self
            .connection
            .query_row(
                "SELECT id, visual_hash, width, height, colors FROM assets WHERE id = ?1",
                [id],
                |row| {
                    let pending = row.get::<_, Option<Vec<u8>>>(1)?.is_none();
                    Ok((pending, fingerprint_from_row(row)?))
                },
            )
            .optional()?;
        Ok(match row {
            None => Stored::Missing,
            Some((true, _)) => Stored::Pending,
            Some((false, None)) => Stored::Unusable,
            Some((false, Some(fingerprint))) => Stored::Ready(fingerprint),
        })
    }
}

fn fingerprint_from_row(row: &rusqlite::Row) -> rusqlite::Result<Option<Fingerprint>> {
    let bytes: Option<Vec<u8>> = row.get(1)?;
    let Some(hash) = bytes.and_then(|bytes| VisualHash::from_bytes(&bytes)) else {
        return Ok(None);
    };
    let width: Option<u32> = row.get(2)?;
    let height: Option<u32> = row.get(3)?;
    let colors: Option<String> = row.get(4)?;
    Ok(Some(Fingerprint {
        asset: row.get(0)?,
        hash,
        proportion: width
            .zip(height)
            .filter(|(width, height)| *width > 0 && *height > 0)
            .map(|(width, height)| f64::from(width) / f64::from(height)),
        families: colors.map(|text| {
            dominant_from_text(&text)
                .into_iter()
                .map(|color| color.family)
                .collect()
        }),
    }))
}

fn similar_proportions(first: Option<f64>, second: Option<f64>, largest_gap: f64) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => first.max(second) / first.min(second) <= largest_gap,
        _ => true,
    }
}

fn share_a_color(first: &Fingerprint, second: &Fingerprint) -> bool {
    match (&first.families, &second.families) {
        (Some(first), Some(second)) if !first.is_empty() && !second.is_empty() => {
            first.iter().any(|family| second.contains(family))
        }
        _ => true,
    }
}

fn resembles(
    first: &Fingerprint,
    second: &Fingerprint,
    max_distance: u32,
    proportion_gap: f64,
) -> bool {
    let distance = first.hash.distance(second.hash);
    distance <= max_distance
        && similar_proportions(first.proportion, second.proportion, proportion_gap)
        && (distance <= TRUSTED_SHAPE_DISTANCE || share_a_color(first, second))
}

fn join_close_fingerprints(fingerprints: &[Fingerprint], max_distance: u32, groups: &mut Groups) {
    let bands = max_distance + 1;
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
                    if groups.root(*first) == groups.root(*second) {
                        continue;
                    }
                    let (a, b) = (&fingerprints[*first], &fingerprints[*second]);
                    if resembles(a, b, max_distance, DUPLICATE_PROPORTION_GAP) {
                        groups.join(*first, *second);
                    }
                }
            }
        }
    }
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
