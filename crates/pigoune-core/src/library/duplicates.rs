use std::collections::HashMap;
use std::ops::ControlFlow;

use rusqlite::OptionalExtension;

use super::{AssetId, Library, LibraryError};
use crate::media::{DominantColor, VisualHash, dominant_from_text};

pub const DUPLICATE_DISTANCE: u32 = 6;
pub const SIMILAR_DISTANCE: u32 = 10;
pub const ALMOST_IDENTICAL_DISTANCE: u32 = 2;
pub const VERY_CLOSE_DISTANCE: u32 = 6;
const HASH_BITS: u32 = 64;
const DUPLICATE_PROPORTION_GAP: f64 = 1.25;
const SIMILAR_PROPORTION_GAP: f64 = 1.5;
const TRUSTED_SHAPE_DISTANCE: u32 = 4;
const PROGRESS_UNITS_PER_BAND: u64 = 1000;
const BUCKETS_PER_REPORT: usize = 64;

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
    colors: Option<Vec<DominantColor>>,
    size_rank: (u64, u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateProgress {
    pub done: u64,
    pub total: u64,
}

pub struct DuplicatePlan {
    fingerprints: Vec<Fingerprint>,
    max_distance: u32,
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
        let mut ranked: Vec<(SimilarAsset, f64)> = self
            .fingerprints()?
            .into_iter()
            .filter(|other| other.asset != id)
            .filter(|other| resembles(&target, other, max_distance, SIMILAR_PROPORTION_GAP))
            .map(|other| {
                let similar = SimilarAsset {
                    asset: other.asset,
                    distance: target.hash.distance(other.hash),
                };
                (
                    similar,
                    palette_gap(&target, &other).unwrap_or(f64::INFINITY),
                )
            })
            .collect();
        ranked.sort_by(|(first, first_gap), (second, second_gap)| {
            closeness_rank(first.distance)
                .cmp(&closeness_rank(second.distance))
                .then(first_gap.total_cmp(second_gap))
                .then(first.distance.cmp(&second.distance))
                .then(first.asset.cmp(&second.asset))
        });
        let similar = ranked.into_iter().map(|(similar, _)| similar).collect();
        Ok(SimilarSearch::Found(similar))
    }

    pub fn duplicate_plan(&self, max_distance: u32) -> Result<DuplicatePlan, LibraryError> {
        Ok(DuplicatePlan {
            fingerprints: self.fingerprints()?,
            max_distance,
        })
    }

    pub fn duplicate_groups(&self, max_distance: u32) -> Result<Vec<Vec<AssetId>>, LibraryError> {
        let plan = self.duplicate_plan(max_distance)?;
        Ok(plan.run(|_| ControlFlow::Continue(())).unwrap_or_default())
    }

    fn fingerprints(&self) -> Result<Vec<Fingerprint>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id, visual_hash, width, height, colors, byte_size FROM assets
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
                "SELECT id, visual_hash, width, height, colors, byte_size FROM assets WHERE id = ?1",
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
    let byte_size: u64 = row.get(5)?;
    Ok(Some(Fingerprint {
        asset: row.get(0)?,
        hash,
        proportion: width
            .zip(height)
            .filter(|(width, height)| *width > 0 && *height > 0)
            .map(|(width, height)| f64::from(width) / f64::from(height)),
        colors: colors.map(|text| dominant_from_text(&text)),
        size_rank: (
            width
                .zip(height)
                .map_or(0, |(width, height)| u64::from(width) * u64::from(height)),
            byte_size,
        ),
    }))
}

fn similar_proportions(first: Option<f64>, second: Option<f64>, largest_gap: f64) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => first.max(second) / first.min(second) <= largest_gap,
        _ => true,
    }
}

fn known_colors(fingerprint: &Fingerprint) -> Option<&[DominantColor]> {
    fingerprint
        .colors
        .as_deref()
        .filter(|colors| !colors.is_empty())
}

fn share_a_color(first: &Fingerprint, second: &Fingerprint) -> bool {
    match (known_colors(first), known_colors(second)) {
        (Some(first), Some(second)) => first
            .iter()
            .any(|color| second.iter().any(|other| other.family == color.family)),
        _ => true,
    }
}

fn closeness_rank(distance: u32) -> u8 {
    if distance <= ALMOST_IDENTICAL_DISTANCE {
        0
    } else if distance <= VERY_CLOSE_DISTANCE {
        1
    } else {
        2
    }
}

fn palette_gap(first: &Fingerprint, second: &Fingerprint) -> Option<f64> {
    let (first, second) = (known_colors(first)?, known_colors(second)?);
    Some(directed_gap(first, second).max(directed_gap(second, first)))
}

fn directed_gap(from: &[DominantColor], to: &[DominantColor]) -> f64 {
    let total: f64 = from
        .iter()
        .map(|color| {
            to.iter()
                .map(|other| color.average.distance_to(other.average))
                .fold(f64::INFINITY, f64::min)
        })
        .sum();
    total / f64::from(u32::try_from(from.len()).unwrap_or(u32::MAX))
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

impl DuplicatePlan {
    #[must_use]
    pub fn run(
        &self,
        mut progress: impl FnMut(DuplicateProgress) -> ControlFlow<()>,
    ) -> Option<Vec<Vec<AssetId>>> {
        let fingerprints = &self.fingerprints;
        let bands = self.max_distance + 1;
        let total = u64::from(bands) * PROGRESS_UNITS_PER_BAND;
        let mut groups = Groups::new(fingerprints.len());
        for band in 0..bands {
            let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
            for (index, fingerprint) in fingerprints.iter().enumerate() {
                buckets
                    .entry(band_of(fingerprint.hash.bits(), band, bands))
                    .or_default()
                    .push(index);
            }
            let bucket_count = u64::try_from(buckets.len().max(1)).unwrap_or(u64::MAX);
            for (number, members) in buckets.values().enumerate() {
                if number % BUCKETS_PER_REPORT == 0 {
                    let within =
                        u64::try_from(number).unwrap_or(0) * PROGRESS_UNITS_PER_BAND / bucket_count;
                    let done = u64::from(band) * PROGRESS_UNITS_PER_BAND + within;
                    if progress(DuplicateProgress { done, total }).is_break() {
                        return None;
                    }
                }
                self.join_bucket(members, &mut groups);
            }
        }
        if progress(DuplicateProgress { done: total, total }).is_break() {
            return None;
        }
        Some(groups.into_groups(fingerprints))
    }

    fn join_bucket(&self, members: &[usize], groups: &mut Groups) {
        for (position, first) in members.iter().enumerate() {
            for second in &members[position + 1..] {
                if groups.root(*first) == groups.root(*second) {
                    continue;
                }
                let (a, b) = (&self.fingerprints[*first], &self.fingerprints[*second]);
                if resembles(a, b, self.max_distance, DUPLICATE_PROPORTION_GAP) {
                    groups.join(*first, *second);
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
        let mut by_root: HashMap<usize, Vec<&Fingerprint>> = HashMap::new();
        for (index, fingerprint) in fingerprints.iter().enumerate() {
            let root = self.root(index);
            by_root.entry(root).or_default().push(fingerprint);
        }
        let mut groups: Vec<Vec<AssetId>> = by_root
            .into_values()
            .filter(|group| group.len() > 1)
            .map(|mut group| {
                group.sort_by(|a, b| b.size_rank.cmp(&a.size_rank).then(a.asset.cmp(&b.asset)));
                group
                    .into_iter()
                    .map(|fingerprint| fingerprint.asset)
                    .collect()
            })
            .collect();
        groups.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
        groups
    }
}
