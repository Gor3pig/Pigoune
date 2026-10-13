use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use pigoune_core::{
    AssetCommand, AssetId, DATABASE_FILE_NAME, ImportOutcome, Library, SimilarSearch, VisualHash,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const SIDE_BY_SIDE: u32 = 6;

fn asset_id(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

struct Seed {
    hash: Option<u64>,
    width: u32,
    height: u32,
}

fn seed(hash: u64) -> Seed {
    Seed {
        hash: Some(hash),
        width: 100,
        height: 100,
    }
}

fn library_with(seeds: &[Seed]) -> (TempDir, Library) {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let library = Library::create(workspace.path(), "Essai").expect("library is created");
    let root = library.root().to_path_buf();
    drop(library);
    insert(&root, seeds);
    (workspace, Library::open(&root).expect("library opens"))
}

fn insert(root: &Path, seeds: &[Seed]) {
    let mut connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("database opens");
    let transaction = connection.transaction().expect("transaction starts");
    for (index, seed) in seeds.iter().enumerate() {
        let number = index + 1;
        let blob: Option<Vec<u8>> = seed.hash.map(|bits| bits.to_be_bytes().to_vec());
        transaction
            .execute(
                "INSERT INTO assets (id, display_name, original_file_name, stored_path, format,
                                     width, height, byte_size, content_hash, is_animated,
                                     embedded_sizes, added_at_unix_ms, visual_hash)
                 VALUES (?1, ?2, ?3, ?4, 'png', ?5, ?6, 100, ?7, 0, '', ?8, ?9)",
                params![
                    asset_id(number).to_string(),
                    format!("asset {number}"),
                    format!("asset-{number}.png"),
                    format!("files/zz/{number}/asset-{number}.png"),
                    seed.width,
                    seed.height,
                    format!("hash-{number}"),
                    i64::try_from(number).expect("small"),
                    blob,
                ],
            )
            .expect("asset is inserted");
    }
    transaction.commit().expect("transaction commits");
}

fn flipped(bits: u64, positions: &[u32]) -> u64 {
    positions
        .iter()
        .fold(bits, |bits, position| bits ^ (1 << position))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn found(search: SimilarSearch) -> Vec<(AssetId, u32)> {
    match search {
        SimilarSearch::Found(similar) => similar
            .into_iter()
            .map(|similar| (similar.asset, similar.distance))
            .collect(),
        other => panic!("unexpected search {other:?}"),
    }
}

#[test]
fn a_new_resource_waits_for_its_hash_until_one_is_recorded() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
    let ImportOutcome::Imported(id) = library
        .import_file(&fixture("red-dot.png"), None)
        .expect("import succeeds")
    else {
        panic!("the file is new");
    };
    assert_eq!(
        library.assets_awaiting_visual_hash(10).expect("listed"),
        [id]
    );

    library
        .record_visual_hashes_batch(&[(id, Some(VisualHash::from_bits(7)))])
        .expect("recorded");

    assert!(
        library
            .assets_awaiting_visual_hash(10)
            .expect("listed")
            .is_empty()
    );
}

#[test]
fn a_resource_without_a_usable_drawing_is_not_analysed_again() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
    let ImportOutcome::Imported(id) = library
        .import_file(&fixture("red-dot.png"), None)
        .expect("import succeeds")
    else {
        panic!("the file is new");
    };

    library
        .record_visual_hashes_batch(&[(id, None)])
        .expect("recorded");

    assert!(
        library
            .assets_awaiting_visual_hash(10)
            .expect("listed")
            .is_empty()
    );
    assert_eq!(
        library.similar_assets(id, 10).expect("searched"),
        SimilarSearch::NoFingerprint
    );
}

#[test]
fn similar_resources_come_closest_first_and_leave_out_the_far_and_the_trashed() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, mut library) = library_with(&[
        seed(base),
        seed(flipped(base, &[1, 2, 3, 4, 5, 6, 7, 8])),
        seed(flipped(base, &[1])),
        seed(flipped(
            base,
            &[0, 10, 20, 30, 40, 50, 60, 61, 62, 63, 5, 6],
        )),
        seed(flipped(base, &[9, 19])),
    ]);
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![asset_id(5)],
            trashed: true,
        })
        .expect("trashed");

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert_eq!(similar, [(asset_id(3), 1), (asset_id(2), 8)]);
}

#[test]
fn a_resource_not_yet_analysed_is_reported_as_pending() {
    let (_workspace, library) = library_with(&[
        Seed {
            hash: None,
            width: 100,
            height: 100,
        },
        seed(1),
    ]);

    assert_eq!(
        library.similar_assets(asset_id(1), 10).expect("searched"),
        SimilarSearch::Pending
    );
}

#[test]
fn the_resources_still_to_analyse_are_counted_without_the_trashed_ones() {
    let missing = Seed {
        hash: None,
        width: 100,
        height: 100,
    };
    let (_workspace, mut library) = library_with(&[
        seed(1),
        Seed {
            hash: None,
            ..missing
        },
        Seed {
            hash: None,
            ..missing
        },
    ]);
    assert_eq!(library.count_awaiting_visual_hash().expect("counted"), 2);

    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![asset_id(3)],
            trashed: true,
        })
        .expect("trashed");

    assert_eq!(library.count_awaiting_visual_hash().expect("counted"), 1);
}

#[test]
fn groups_gather_resources_that_are_close_to_each_other_in_a_chain() {
    let base = 0xf0f0_f0f0_f0f0_f0f0_u64;
    let near = flipped(base, &[0, 1, 2, 3]);
    let nearer_still = flipped(near, &[10, 11, 12, 13]);
    let (_workspace, library) = library_with(&[
        seed(base),
        seed(near),
        seed(nearer_still),
        seed(!base),
        seed(0x1234_5678_9abc_def0),
    ]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert_eq!(groups, [vec![asset_id(1), asset_id(2), asset_id(3)]]);
}

#[test]
fn proportions_that_differ_too_much_keep_resources_apart() {
    let (_workspace, library) = library_with(&[
        Seed {
            hash: Some(99),
            width: 100,
            height: 100,
        },
        Seed {
            hash: Some(99),
            width: 300,
            height: 100,
        },
        Seed {
            hash: Some(99),
            width: 110,
            height: 100,
        },
    ]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert_eq!(groups, [vec![asset_id(1), asset_id(3)]]);
}

#[test]
fn trashed_resources_never_appear_in_groups() {
    let (_workspace, mut library) = library_with(&[seed(5), seed(5), seed(5)]);
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![asset_id(2)],
            trashed: true,
        })
        .expect("trashed");

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert_eq!(groups, [vec![asset_id(1), asset_id(3)]]);
}

fn pseudo_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state ^ (*state >> 29)
}

fn root(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

fn brute_force_groups(hashes: &[u64], max_distance: u32) -> Vec<Vec<AssetId>> {
    let mut parent: Vec<usize> = (0..hashes.len()).collect();
    for first in 0..hashes.len() {
        for second in first + 1..hashes.len() {
            if (hashes[first] ^ hashes[second]).count_ones() <= max_distance {
                let (a, b) = (root(&mut parent, first), root(&mut parent, second));
                parent[a.max(b)] = a.min(b);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<AssetId>> = BTreeMap::new();
    for index in 0..hashes.len() {
        let top = root(&mut parent, index);
        groups.entry(top).or_default().push(asset_id(index + 1));
    }
    let mut groups: Vec<Vec<AssetId>> = groups.into_values().filter(|g| g.len() > 1).collect();
    groups.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
    groups
}

fn clustered_hashes(count: usize, clusters: usize) -> Vec<u64> {
    let mut state = 42_u64;
    let centers: Vec<u64> = (0..clusters).map(|_| pseudo_random(&mut state)).collect();
    (0..count)
        .map(|_| {
            let center =
                centers[usize::try_from(pseudo_random(&mut state)).expect("fits") % clusters];
            let noise = pseudo_random(&mut state);
            let changed = (noise % 9) as usize;
            let mut hash = center;
            for step in 0..changed {
                hash ^= 1 << ((pseudo_random(&mut state) >> (step % 7)) % 64);
            }
            hash
        })
        .collect()
}

#[test]
fn banded_grouping_finds_exactly_what_comparing_every_pair_finds() {
    let hashes = clustered_hashes(1500, 120);
    let seeds: Vec<Seed> = hashes.iter().map(|hash| seed(*hash)).collect();
    let (_workspace, library) = library_with(&seeds);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert!(groups.len() > 20);
    assert_eq!(groups, brute_force_groups(&hashes, SIDE_BY_SIDE));
}

#[test]
fn grouping_forty_thousand_resources_stays_quick() {
    let hashes = clustered_hashes(40_000, 4_000);
    let seeds: Vec<Seed> = hashes.iter().map(|hash| seed(*hash)).collect();
    let (_workspace, library) = library_with(&seeds);

    let start = Instant::now();
    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert!(!groups.is_empty());
    assert!(start.elapsed().as_secs() < 60, "{:?}", start.elapsed());
}
