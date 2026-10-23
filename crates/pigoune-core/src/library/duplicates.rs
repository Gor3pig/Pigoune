use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

use rusqlite::OptionalExtension;

use super::ignored_duplicates::IgnoredSets;
use super::{AssetId, Library, LibraryError};
use crate::media::{DominantColor, VisualDetail, VisualHash, dominant_from_text};

pub const DUPLICATE_DISTANCE: u32 = 4;
pub const IDENTICAL_DETAIL_GAP: f64 = 7.0;
pub const PROBABLE_DETAIL_GAP: f64 = 14.0;
pub const MOST_IN_A_GROUP: usize = 40;
pub const SIMILAR_DISTANCE: u32 = 10;
pub const ALMOST_IDENTICAL_DISTANCE: u32 = 2;
pub const VERY_CLOSE_DISTANCE: u32 = 6;
const HASH_BITS: u32 = 64;
const DUPLICATE_PROPORTION_GAP: f64 = 1.25;
const SIMILAR_PROPORTION_GAP: f64 = 1.5;
const TRUSTED_SHAPE_DISTANCE: u32 = 4;
const STEPS_PER_REPORT: usize = 64;

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

pub(super) struct Fingerprint {
    asset: AssetId,
    hash: VisualHash,
    proportion: Option<f64>,
    colors: Option<Vec<DominantColor>>,
    detail: Option<VisualDetail>,
    size_rank: (u64, u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateKind {
    VisuallyIdentical,
    Probable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupMember {
    pub asset: AssetId,
    pub kind: DuplicateKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub members: Vec<GroupMember>,
}

impl DuplicateGroup {
    #[must_use]
    pub fn assets(&self) -> Vec<AssetId> {
        self.members.iter().map(|member| member.asset).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateProgress {
    pub done: u64,
    pub total: u64,
}

pub struct DuplicatePlan {
    fingerprints: Vec<Fingerprint>,
    ignored: IgnoredSets,
}

pub(super) enum Stored {
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
        let target = match self.stored_fingerprint(id, false)? {
            Stored::Missing | Stored::Unusable => return Ok(SimilarSearch::NoFingerprint),
            Stored::Pending => return Ok(SimilarSearch::Pending),
            Stored::Ready(target) => target,
        };
        let mut ranked: Vec<(SimilarAsset, f64)> = self
            .fingerprints(false)?
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

    pub fn duplicate_plan(&self) -> Result<DuplicatePlan, LibraryError> {
        Ok(DuplicatePlan {
            fingerprints: self.fingerprints_outside_skipped(true)?,
            ignored: self.ignored_sets()?,
        })
    }

    pub fn duplicate_candidates_awaiting_detail(&self) -> Result<Vec<AssetId>, LibraryError> {
        let awaiting = self.assets_without_detail()?;
        if awaiting.is_empty() {
            return Ok(Vec::new());
        }
        let fingerprints = self.fingerprints_outside_skipped(false)?;
        let bands = DUPLICATE_DISTANCE + 1;
        let buckets = band_buckets(&fingerprints, bands);
        let found = fingerprints
            .iter()
            .filter(|fingerprint| awaiting.contains(&fingerprint.asset))
            .filter(|fingerprint| {
                (0..bands).any(|band| {
                    let value = band_of(fingerprint.hash.bits(), band, bands);
                    buckets[band as usize][&value].iter().any(|index| {
                        let other = &fingerprints[*index];
                        other.asset != fingerprint.asset
                            && fingerprint.hash.distance(other.hash) <= DUPLICATE_DISTANCE
                            && similar_proportions(
                                fingerprint.proportion,
                                other.proportion,
                                DUPLICATE_PROPORTION_GAP,
                            )
                    })
                })
            })
            .map(|fingerprint| fingerprint.asset)
            .collect();
        Ok(found)
    }

    fn assets_without_detail(&self) -> Result<HashSet<AssetId>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT a.id FROM assets a
             LEFT JOIN details.asset_details d ON d.asset_id = a.id
             WHERE a.trashed_at_unix_ms IS NULL AND length(a.visual_hash) = ?1
               AND d.asset_id IS NULL",
        )?;
        let ids = statement
            .query_map([VisualHash::BYTE_LENGTH], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    pub fn duplicate_groups(&self) -> Result<Vec<Vec<AssetId>>, LibraryError> {
        let plan = self.duplicate_plan()?;
        let groups = plan.run(|_| ControlFlow::Continue(())).unwrap_or_default();
        Ok(groups.iter().map(DuplicateGroup::assets).collect())
    }

    pub fn duplicate_kind_between(
        &self,
        first: AssetId,
        second: AssetId,
    ) -> Result<Option<DuplicateKind>, LibraryError> {
        let (Stored::Ready(first), Stored::Ready(second)) = (
            self.stored_fingerprint(first, true)?,
            self.stored_fingerprint(second, true)?,
        ) else {
            return Ok(None);
        };
        if !same_shape(&first, &second) || self.is_ignored_duplicate(first.asset, second.asset)? {
            return Ok(None);
        }
        Ok(detail_kind(&first, &second))
    }

    fn fingerprints_outside_skipped(
        &self,
        with_detail: bool,
    ) -> Result<Vec<Fingerprint>, LibraryError> {
        let left_out = self.assets_in_skipped_collections()?;
        let mut fingerprints = self.fingerprints(with_detail)?;
        if !left_out.is_empty() {
            fingerprints.retain(|fingerprint| !left_out.contains(&fingerprint.asset));
        }
        Ok(fingerprints)
    }

    fn fingerprints(&self, with_detail: bool) -> Result<Vec<Fingerprint>, LibraryError> {
        let (detail_column, detail_join) = detail_sql(with_detail);
        let mut statement = self.connection.prepare(&format!(
            "SELECT a.id, a.visual_hash, a.width, a.height, a.colors, a.byte_size, {detail_column}
             FROM assets a {detail_join}
             WHERE a.trashed_at_unix_ms IS NULL AND length(a.visual_hash) = ?1
             ORDER BY a.id"
        ))?;
        let rows = statement.query_map([VisualHash::BYTE_LENGTH], fingerprint_from_row)?;
        let mut fingerprints = Vec::new();
        for row in rows {
            if let Some(fingerprint) = row? {
                fingerprints.push(fingerprint);
            }
        }
        Ok(fingerprints)
    }

    pub(super) fn stored_fingerprint(
        &self,
        id: AssetId,
        with_detail: bool,
    ) -> Result<Stored, LibraryError> {
        let (detail_column, detail_join) = detail_sql(with_detail);
        let row = self
            .connection
            .query_row(
                &format!(
                    "SELECT a.id, a.visual_hash, a.width, a.height, a.colors, a.byte_size,
                            {detail_column}
                     FROM assets a {detail_join} WHERE a.id = ?1"
                ),
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

fn detail_sql(with_detail: bool) -> (&'static str, &'static str) {
    if with_detail {
        (
            "d.detail",
            "LEFT JOIN details.asset_details d ON d.asset_id = a.id",
        )
    } else {
        ("NULL", "")
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
    let detail: Option<Vec<u8>> = row.get(6)?;
    Ok(Some(Fingerprint {
        asset: row.get(0)?,
        hash,
        proportion: width
            .zip(height)
            .filter(|(width, height)| *width > 0 && *height > 0)
            .map(|(width, height)| f64::from(width) / f64::from(height)),
        colors: colors.map(|text| dominant_from_text(&text)),
        detail: detail.and_then(|bytes| VisualDetail::from_bytes(&bytes)),
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

fn same_shape(first: &Fingerprint, second: &Fingerprint) -> bool {
    first.hash.distance(second.hash) <= DUPLICATE_DISTANCE
        && similar_proportions(
            first.proportion,
            second.proportion,
            DUPLICATE_PROPORTION_GAP,
        )
}

fn detail_kind(first: &Fingerprint, second: &Fingerprint) -> Option<DuplicateKind> {
    let gap = first.detail.as_ref()?.gap(second.detail.as_ref()?);
    if gap <= IDENTICAL_DETAIL_GAP {
        Some(DuplicateKind::VisuallyIdentical)
    } else if gap <= PROBABLE_DETAIL_GAP {
        Some(DuplicateKind::Probable)
    } else {
        None
    }
}

pub(super) fn duplicate_kind(first: &Fingerprint, second: &Fingerprint) -> Option<DuplicateKind> {
    if same_shape(first, second) {
        detail_kind(first, second)
    } else {
        None
    }
}

impl DuplicatePlan {
    fn kind_between(&self, first: usize, second: usize) -> Option<DuplicateKind> {
        let (first, second) = (&self.fingerprints[first], &self.fingerprints[second]);
        if !same_shape(first, second) {
            return None;
        }
        if !self.ignored.is_empty() && self.ignored.contains(first.asset, second.asset) {
            return None;
        }
        detail_kind(first, second)
    }

    pub(super) fn cluster_of(&self, seeds: &[AssetId]) -> Result<Vec<AssetId>, AssetId> {
        let positions: HashMap<AssetId, usize> = self
            .fingerprints
            .iter()
            .enumerate()
            .map(|(position, fingerprint)| (fingerprint.asset, position))
            .collect();
        let start = *positions.get(&seeds[0]).ok_or(seeds[0])?;
        let bands = DUPLICATE_DISTANCE + 1;
        let buckets = band_buckets(&self.fingerprints, bands);
        let mut reached = vec![false; self.fingerprints.len()];
        reached[start] = true;
        let mut queue = vec![start];
        while let Some(current) = queue.pop() {
            for band in 0..bands {
                let value = band_of(self.fingerprints[current].hash.bits(), band, bands);
                for candidate in &buckets[band as usize][&value] {
                    if !reached[*candidate] && self.kind_between(current, *candidate).is_some() {
                        reached[*candidate] = true;
                        queue.push(*candidate);
                    }
                }
            }
        }
        for seed in seeds {
            if !positions
                .get(seed)
                .is_some_and(|position| reached[*position])
            {
                return Err(*seed);
            }
        }
        let mut cluster: Vec<AssetId> = reached
            .iter()
            .enumerate()
            .filter(|(_, reached)| **reached)
            .map(|(position, _)| self.fingerprints[position].asset)
            .collect();
        cluster.sort_unstable();
        Ok(cluster)
    }

    #[must_use]
    pub fn run(
        &self,
        mut progress: impl FnMut(DuplicateProgress) -> ControlFlow<()>,
    ) -> Option<Vec<DuplicateGroup>> {
        let fingerprints = &self.fingerprints;
        let total = u64::try_from(fingerprints.len()).unwrap_or(u64::MAX);
        let bands = DUPLICATE_DISTANCE + 1;
        let buckets = band_buckets(fingerprints, bands);
        let mut order: Vec<usize> = (0..fingerprints.len()).collect();
        order.sort_by(|a, b| {
            fingerprints[*b]
                .size_rank
                .cmp(&fingerprints[*a].size_rank)
                .then(fingerprints[*a].asset.cmp(&fingerprints[*b].asset))
        });
        let mut taken = vec![false; fingerprints.len()];
        let mut seen = vec![0_u32; fingerprints.len()];
        let mut groups = Vec::new();
        for (step, reference) in order.iter().copied().enumerate() {
            if step % STEPS_PER_REPORT == 0 {
                let done = u64::try_from(step).unwrap_or(0);
                if progress(DuplicateProgress { done, total }).is_break() {
                    return None;
                }
            }
            if taken[reference] {
                continue;
            }
            taken[reference] = true;
            let stamp = u32::try_from(step + 1).unwrap_or(u32::MAX);
            let mut candidates: Vec<(usize, DuplicateKind)> = Vec::new();
            for band in 0..bands {
                let value = band_of(fingerprints[reference].hash.bits(), band, bands);
                for candidate in &buckets[band as usize][&value] {
                    if taken[*candidate] || seen[*candidate] == stamp {
                        continue;
                    }
                    seen[*candidate] = stamp;
                    if let Some(kind) = self.kind_between(reference, *candidate) {
                        candidates.push((*candidate, kind));
                    }
                }
            }
            candidates.sort_by(|(a, _), (b, _)| {
                fingerprints[*b]
                    .size_rank
                    .cmp(&fingerprints[*a].size_rank)
                    .then(fingerprints[*a].asset.cmp(&fingerprints[*b].asset))
            });
            let mut accepted: Vec<(usize, DuplicateKind)> = Vec::new();
            for (candidate, kind) in candidates {
                if accepted.len() + 1 >= MOST_IN_A_GROUP {
                    break;
                }
                let agrees_with_all = accepted
                    .iter()
                    .all(|(member, _)| self.kind_between(*member, candidate).is_some());
                if agrees_with_all {
                    accepted.push((candidate, kind));
                }
            }
            if accepted.is_empty() {
                continue;
            }
            let mut members = vec![GroupMember {
                asset: fingerprints[reference].asset,
                kind: DuplicateKind::VisuallyIdentical,
            }];
            for (candidate, kind) in accepted {
                taken[candidate] = true;
                members.push(GroupMember {
                    asset: fingerprints[candidate].asset,
                    kind,
                });
            }
            groups.push(DuplicateGroup { members });
        }
        if progress(DuplicateProgress { done: total, total }).is_break() {
            return None;
        }
        groups.sort_by(|a, b| {
            b.members
                .len()
                .cmp(&a.members.len())
                .then(a.members[0].asset.cmp(&b.members[0].asset))
        });
        Some(groups)
    }
}

fn band_buckets(fingerprints: &[Fingerprint], bands: u32) -> Vec<HashMap<u64, Vec<usize>>> {
    let mut buckets: Vec<HashMap<u64, Vec<usize>>> = vec![HashMap::new(); bands as usize];
    for (index, fingerprint) in fingerprints.iter().enumerate() {
        for band in 0..bands {
            buckets[band as usize]
                .entry(band_of(fingerprint.hash.bits(), band, bands))
                .or_default()
                .push(index);
        }
    }
    buckets
}

fn band_of(bits: u64, band: u32, bands: u32) -> u64 {
    let mut value = 0_u64;
    for position in (band..HASH_BITS).step_by(bands as usize) {
        value = (value << 1) | ((bits >> position) & 1);
    }
    value
}
