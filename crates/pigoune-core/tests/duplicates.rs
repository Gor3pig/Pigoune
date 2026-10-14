use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::Instant;

use pigoune_core::{
    AssetCommand, AssetId, AssetView, ConfirmedRemoval, DATABASE_FILE_NAME, DUPLICATE_DISTANCE,
    DuplicateKind, ImportOutcome, Library, MOST_IN_A_GROUP, PROBABLE_DETAIL_GAP, RemovalError,
    SimilarSearch, VisualAnalysis, VisualDetail, VisualHash,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

const SIDE_BY_SIDE: u32 = DUPLICATE_DISTANCE;
const DETAIL_CELLS: usize = 256;
const PLAIN_CELL: u8 = 20;
const LIT_CELL: u8 = 230;

fn asset_id(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

struct Seed {
    hash: Option<u64>,
    width: u32,
    height: u32,
    colors: Option<&'static str>,
    tint: u8,
    detail: bool,
    drawn_as: Option<u64>,
}

fn detail_for(hash: u64, tint: u8) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(DETAIL_CELLS * 4);
    for cell in 0..DETAIL_CELLS {
        let value = if (hash >> (cell % 64)) & 1 == 1 {
            LIT_CELL
        } else {
            PLAIN_CELL
        };
        bytes.extend_from_slice(&[value.saturating_add(tint), value, value, 255]);
    }
    bytes
}

fn analysis_for(hash: u64) -> VisualAnalysis {
    VisualAnalysis {
        hash: VisualHash::from_bits(hash),
        detail: VisualDetail::from_bytes(&detail_for(hash, 0)).expect("a detail"),
    }
}

fn seed(hash: u64) -> Seed {
    Seed {
        hash: Some(hash),
        width: 100,
        height: 100,
        colors: None,
        tint: 0,
        detail: true,
        drawn_as: None,
    }
}

fn looking_like(hash: u64, drawn_as: u64) -> Seed {
    Seed {
        drawn_as: Some(drawn_as),
        ..seed(hash)
    }
}

fn tinted(hash: u64, tint: u8) -> Seed {
    Seed { tint, ..seed(hash) }
}

fn without_detail(hash: u64) -> Seed {
    Seed {
        detail: false,
        ..seed(hash)
    }
}

fn shaped(hash: u64, width: u32, height: u32) -> Seed {
    Seed {
        width,
        height,
        ..seed(hash)
    }
}

fn colored(hash: u64, colors: &'static str) -> Seed {
    Seed {
        colors: Some(colors),
        ..seed(hash)
    }
}

const DARK_AND_LIGHT: &str = "black#080808,white#f0f0f0";
const BROWN: &str = "brown#704828";

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
        let detail: Option<Vec<u8>> = seed
            .hash
            .filter(|_| seed.detail)
            .map(|bits| detail_for(seed.drawn_as.unwrap_or(bits), seed.tint));
        transaction
            .execute(
                "INSERT INTO assets (id, display_name, original_file_name, stored_path, format,
                                     width, height, byte_size, content_hash, is_animated,
                                     embedded_sizes, added_at_unix_ms, visual_hash, colors, visual_detail)
                 VALUES (?1, ?2, ?3, ?4, 'png', ?5, ?6, 100, ?7, 0, '', ?8, ?9, ?10, ?11)",
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
                    seed.colors,
                    detail,
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
        .record_visual_analysis_batch(&[(id, Some(analysis_for(7)))])
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
        .record_visual_analysis_batch(&[(id, None)])
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
            ..seed(0)
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
        ..seed(0)
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
fn a_chain_of_resemblances_never_becomes_one_group() {
    let base = 0xf0f0_f0f0_f0f0_f0f0_u64;
    let near = flipped(base, &[0, 1, 2]);
    let nearer_still = flipped(near, &[10, 11, 12]);
    let (_workspace, library) = library_with(&[
        seed(base),
        seed(near),
        seed(nearer_still),
        seed(!base),
        seed(0x1234_5678_9abc_def0),
    ]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert_eq!(groups, [vec![asset_id(1), asset_id(2)]]);
}
#[test]
fn proportions_that_differ_too_much_keep_resources_apart() {
    let (_workspace, library) = library_with(&[
        shaped(99, 100, 100),
        shaped(99, 300, 100),
        shaped(99, 110, 100),
    ]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert_eq!(groups, [vec![asset_id(3), asset_id(1)]]);
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

fn number_of(id: AssetId) -> usize {
    (1..=70_000)
        .find(|number| asset_id(*number) == id)
        .expect("a seeded asset")
}

fn pseudo_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state ^ (*state >> 29)
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
fn every_pair_inside_a_group_is_verified() {
    let hashes = clustered_hashes(1500, 120);
    let seeds: Vec<Seed> = hashes.iter().map(|hash| seed(*hash)).collect();
    let (_workspace, library) = library_with(&seeds);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert!(groups.len() > 20);
    for group in &groups {
        assert!(group.len() <= MOST_IN_A_GROUP);
        for (position, first) in group.iter().enumerate() {
            for second in &group[position + 1..] {
                let a = hashes[number_of(*first) - 1];
                let b = hashes[number_of(*second) - 1];
                let gap = VisualDetail::from_bytes(&detail_for(a, 0))
                    .expect("detail")
                    .gap(&VisualDetail::from_bytes(&detail_for(b, 0)).expect("detail"));
                assert!((a ^ b).count_ones() <= SIDE_BY_SIDE);
                assert!(gap <= PROBABLE_DETAIL_GAP, "{gap}");
            }
        }
    }
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

#[test]
fn thousands_of_identical_resources_are_split_into_small_groups() {
    let seeds: Vec<Seed> = (0..6_000).map(|_| seed(0xdead_beef_0bad_f00d)).collect();
    let (_workspace, library) = library_with(&seeds);

    let start = Instant::now();
    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert!(groups.iter().all(|group| group.len() <= MOST_IN_A_GROUP));
    assert_eq!(groups.iter().map(Vec::len).sum::<usize>(), 6_000);
    assert!(start.elapsed().as_secs() < 30, "{:?}", start.elapsed());
}
const WALLPAPER: u64 = 0x0030_30e0_e4c4_c484;
const SESSION_THUMBNAIL: u64 = 0x3030_64e4_e4c4_c692;

#[test]
fn a_tall_wallpaper_is_not_offered_as_similar_to_a_square_thumbnail() {
    let (_workspace, library) = library_with(&[
        shaped(SESSION_THUMBNAIL, 320, 320),
        shaped(WALLPAPER, 1440, 2560),
    ]);
    assert_eq!(
        (SESSION_THUMBNAIL ^ WALLPAPER).count_ones(),
        10,
        "the real pair that was wrongly matched"
    );

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert!(similar.is_empty());
}

#[test]
fn similar_proportions_may_differ_a_little_but_not_by_much() {
    let (_workspace, library) = library_with(&[
        seed(0xabcd_ef01_2345_6789),
        shaped(0xabcd_ef01_2345_6789, 100, 140),
        shaped(0xabcd_ef01_2345_6789, 100, 200),
    ]);

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert_eq!(similar, [(asset_id(2), 0)]);
}

#[test]
fn a_loose_resemblance_needs_a_color_in_common() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, library) = library_with(&[
        colored(base, DARK_AND_LIGHT),
        colored(flipped(base, &[1, 2, 3, 4, 5, 6, 7, 8]), BROWN),
        colored(
            flipped(base, &[9, 10, 11, 12, 13, 14, 15, 16]),
            "white#ffffff",
        ),
    ]);

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert_eq!(similar, [(asset_id(3), 8)]);
}

#[test]
fn a_very_close_shape_is_offered_even_in_other_colors() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, library) = library_with(&[
        colored(base, DARK_AND_LIGHT),
        colored(flipped(base, &[1, 2, 3]), BROWN),
    ]);

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert_eq!(similar, [(asset_id(2), 3)]);
}

#[test]
fn colors_not_known_yet_do_not_hold_a_resemblance_back() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, library) = library_with(&[
        colored(base, DARK_AND_LIGHT),
        seed(flipped(base, &[1, 2, 3, 4, 5, 6, 7, 8])),
    ]);

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert_eq!(similar, [(asset_id(2), 8)]);
}

#[test]
fn the_same_shape_in_another_color_is_not_a_duplicate() {
    let base = 0x0f0f_0f0f_0f0f_0f0f_u64;
    let (_workspace, library) =
        library_with(&[seed(base), tinted(base, 120), seed(flipped(base, &[20]))]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert_eq!(groups, [vec![asset_id(1), asset_id(3)]]);
}
#[test]
fn within_the_same_closeness_the_closest_colors_come_first() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, library) = library_with(&[
        colored(base, "blue#3050c0"),
        colored(flipped(base, &[1]), "red#c03030"),
        colored(flipped(base, &[2, 3]), "blue#3458c4"),
    ]);

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert_eq!(similar, [(asset_id(3), 2), (asset_id(2), 1)]);
}

#[test]
fn a_nearer_shape_still_beats_better_colors() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, library) = library_with(&[
        colored(base, "blue#3050c0"),
        colored(flipped(base, &[1]), "red#c03030"),
        colored(flipped(base, &[2, 3, 4, 5, 6]), "blue#3050c0"),
    ]);

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert_eq!(similar, [(asset_id(2), 1), (asset_id(3), 5)]);
}

#[test]
fn unknown_colors_come_after_known_ones_of_the_same_closeness() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, library) = library_with(&[
        colored(base, "blue#3050c0"),
        seed(flipped(base, &[1])),
        colored(flipped(base, &[2, 3]), "red#c03030"),
    ]);

    let similar = found(library.similar_assets(asset_id(1), 10).expect("searched"));

    assert_eq!(similar, [(asset_id(3), 2), (asset_id(2), 1)]);
}

#[test]
fn a_group_lists_its_largest_image_first() {
    let (_workspace, library) = library_with(&[
        shaped(77, 100, 100),
        shaped(77, 200, 200),
        shaped(77, 150, 150),
    ]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert_eq!(groups, [vec![asset_id(2), asset_id(3), asset_id(1)]]);
}

#[test]
fn a_plan_finds_the_same_groups_as_the_direct_search() {
    let hashes = clustered_hashes(1500, 120);
    let seeds: Vec<Seed> = hashes.iter().map(|hash| seed(*hash)).collect();
    let (_workspace, library) = library_with(&seeds);

    let plan = library.duplicate_plan(SIDE_BY_SIDE).expect("planned");
    let found: Vec<Vec<AssetId>> = plan
        .run(|_| ControlFlow::Continue(()))
        .expect("not canceled")
        .into_iter()
        .map(|group| {
            group
                .members
                .into_iter()
                .map(|member| member.asset)
                .collect()
        })
        .collect();

    assert_eq!(
        found,
        library.duplicate_groups(SIDE_BY_SIDE).expect("grouped")
    );
}
#[test]
fn a_plan_reports_a_progress_that_only_grows_and_ends_complete() {
    let hashes = clustered_hashes(800, 60);
    let seeds: Vec<Seed> = hashes.iter().map(|hash| seed(*hash)).collect();
    let (_workspace, library) = library_with(&seeds);
    let plan = library.duplicate_plan(SIDE_BY_SIDE).expect("planned");

    let mut reports = Vec::new();
    plan.run(|progress| {
        reports.push((progress.done, progress.total));
        ControlFlow::Continue(())
    })
    .expect("not canceled");

    assert!(reports.len() > 2);
    assert!(reports.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    assert!(reports.iter().all(|(done, total)| done <= total));
    let (done, total) = *reports.last().expect("reported");
    assert_eq!(done, total);
}

#[test]
fn a_plan_can_be_canceled() {
    let hashes = clustered_hashes(800, 60);
    let seeds: Vec<Seed> = hashes.iter().map(|hash| seed(*hash)).collect();
    let (_workspace, library) = library_with(&seeds);
    let plan = library.duplicate_plan(SIDE_BY_SIDE).expect("planned");

    let found = plan.run(|_| ControlFlow::Break(()));

    assert!(found.is_none());
}

fn trashed_ids(library: &Library) -> Vec<AssetId> {
    let mut ids: Vec<AssetId> = library
        .visible_assets_in(AssetView::Trash)
        .expect("listed")
        .iter()
        .map(|asset| asset.id)
        .collect();
    ids.sort();
    ids
}

fn removal(kept: usize, chosen: &[usize]) -> ConfirmedRemoval {
    ConfirmedRemoval::new(
        asset_id(kept),
        chosen.iter().map(|number| asset_id(*number)).collect(),
    )
    .expect("a valid request")
}

fn flipped_in_chain(base: u64) -> [u64; 4] {
    [
        base,
        flipped(base, &[0, 1, 2, 3, 4, 5]),
        flipped(base, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        !base,
    ]
}

#[test]
fn only_the_assets_the_user_confirmed_are_moved_to_the_trash() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, mut library) = library_with(&[
        seed(base),
        seed(flipped(base, &[1])),
        seed(flipped(base, &[2])),
        seed(!base),
    ]);

    library
        .trash_confirmed_duplicates(&removal(1, &[2]))
        .expect("moved");

    assert_eq!(trashed_ids(&library), [asset_id(2)]);
}

#[test]
fn a_request_cannot_be_built_without_an_explicit_and_sound_selection() {
    let kept = asset_id(1);
    assert!(matches!(
        ConfirmedRemoval::new(kept, Vec::new()),
        Err(RemovalError::NothingChosen)
    ));
    assert!(matches!(
        ConfirmedRemoval::new(kept, vec![asset_id(2), kept]),
        Err(RemovalError::KeptAmongChosen)
    ));
    assert!(matches!(
        ConfirmedRemoval::new(kept, vec![asset_id(2), asset_id(2)]),
        Err(RemovalError::ChosenTwice(_))
    ));
    let too_many: Vec<AssetId> = (2..2 + 600).map(asset_id).collect();
    assert!(matches!(
        ConfirmedRemoval::new(kept, too_many),
        Err(RemovalError::TooManyChosen { .. })
    ));
}

#[test]
fn an_asset_that_no_longer_resembles_the_kept_one_stops_the_whole_operation() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, mut library) =
        library_with(&[seed(base), seed(flipped(base, &[1])), seed(!base)]);

    let outcome = library.trash_confirmed_duplicates(&removal(1, &[2, 3]));

    assert!(matches!(outcome, Err(RemovalError::NotAlike(id)) if id == asset_id(3)));
    assert!(trashed_ids(&library).is_empty());
}

#[test]
fn the_end_of_a_chain_cannot_be_trashed_by_keeping_its_start() {
    let [start, middle, end, other] = flipped_in_chain(0x0f0f_0f0f_0f0f_0f0f);
    let (_workspace, mut library) =
        library_with(&[seed(start), seed(middle), seed(end), seed(other)]);

    let outcome = library.trash_confirmed_duplicates(&removal(1, &[3]));

    assert!(matches!(outcome, Err(RemovalError::NotAlike(_))));
    assert!(trashed_ids(&library).is_empty());
}

#[test]
fn a_selection_gone_stale_is_refused_and_changes_nothing() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, mut library) = library_with(&[
        seed(base),
        seed(flipped(base, &[1])),
        seed(flipped(base, &[2])),
    ]);
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![asset_id(3)],
            trashed: true,
        })
        .expect("trashed");

    let outcome = library.trash_confirmed_duplicates(&removal(1, &[2, 3]));

    assert!(matches!(outcome, Err(RemovalError::NotAvailable(id)) if id == asset_id(3)));
    assert_eq!(trashed_ids(&library), [asset_id(3)]);
}

#[test]
fn unknown_and_trashed_assets_are_never_accepted() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, mut library) = library_with(&[seed(base), seed(flipped(base, &[1]))]);

    let unknown = library.trash_confirmed_duplicates(&removal(1, &[99]));
    assert!(matches!(unknown, Err(RemovalError::NotAvailable(_))));

    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![asset_id(1)],
            trashed: true,
        })
        .expect("trashed");
    let kept_gone = library.trash_confirmed_duplicates(&removal(1, &[2]));
    assert!(matches!(kept_gone, Err(RemovalError::NotAvailable(id)) if id == asset_id(1)));
    assert_eq!(trashed_ids(&library), [asset_id(1)]);
}

#[test]
fn the_move_can_be_undone_completely() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, mut library) = library_with(&[
        seed(base),
        seed(flipped(base, &[1])),
        seed(flipped(base, &[2])),
    ]);

    let stamp = library
        .trash_confirmed_duplicates(&removal(1, &[2, 3]))
        .expect("moved");
    assert_eq!(trashed_ids(&library), [asset_id(2), asset_id(3)]);
    library.undo_change(stamp).expect("undone");

    assert!(trashed_ids(&library).is_empty());
}

#[test]
fn whatever_is_requested_only_confirmed_assets_ever_change() {
    let mut state = 7_u64;
    let hashes = clustered_hashes(60, 8);
    let seeds: Vec<Seed> = hashes.iter().map(|hash| seed(*hash)).collect();
    let (_workspace, mut library) = library_with(&seeds);
    for _ in 0..400 {
        let kept = usize::try_from(pseudo_random(&mut state) % 60).expect("small") + 1;
        let count = usize::try_from(pseudo_random(&mut state) % 5).expect("small") + 1;
        let chosen: Vec<usize> = (0..count)
            .map(|_| usize::try_from(pseudo_random(&mut state) % 62).expect("small") + 1)
            .collect();
        let before = trashed_ids(&library);
        let ids: Vec<AssetId> = chosen.iter().map(|number| asset_id(*number)).collect();
        let outcome = ConfirmedRemoval::new(asset_id(kept), ids.clone())
            .and_then(|request| library.trash_confirmed_duplicates(&request));
        let after = trashed_ids(&library);
        match outcome {
            Ok(stamp) => {
                assert!(
                    after
                        .iter()
                        .all(|id| before.contains(id) || ids.contains(id))
                );
                assert!(!after.contains(&asset_id(kept)));
                library.undo_change(stamp).expect("undone");
            }
            Err(_) => assert_eq!(after, before),
        }
        assert_eq!(trashed_ids(&library), before);
    }
}

#[test]
fn identical_and_probable_duplicates_are_told_apart() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, library) = library_with(&[
        seed(base),
        seed(flipped(base, &[1])),
        seed(flipped(base, &[2, 3, 4])),
    ]);

    let plan = library.duplicate_plan(SIDE_BY_SIDE).expect("planned");
    let groups = plan.run(|_| ControlFlow::Continue(())).expect("done");

    assert_eq!(groups.len(), 1);
    let kinds: Vec<(AssetId, DuplicateKind)> = groups[0]
        .members
        .iter()
        .map(|member| (member.asset, member.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            (asset_id(1), DuplicateKind::VisuallyIdentical),
            (asset_id(2), DuplicateKind::VisuallyIdentical),
            (asset_id(3), DuplicateKind::Probable),
        ]
    );
}

#[test]
fn a_silhouette_in_common_is_not_enough_to_be_a_duplicate() {
    let silhouette = 0x0f00_0000_0000_000f_u64;
    let (_workspace, library) = library_with(&[
        seed(silhouette),
        looking_like(silhouette, !silhouette),
        looking_like(silhouette, 0x00ff_ff00_00ff_ff00),
    ]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert!(groups.is_empty());
}

#[test]
fn assets_without_a_detail_are_not_offered_as_duplicates() {
    let (_workspace, library) = library_with(&[seed(9), without_detail(9)]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert!(groups.is_empty());
}

#[test]
fn a_library_of_icons_sharing_one_silhouette_is_not_swallowed_by_one_group() {
    let silhouette = 0x0f00_0000_0000_000f_u64;
    let mut state = 11_u64;
    let mut hashes: Vec<u64> = (0..400)
        .map(|_| silhouette | ((pseudo_random(&mut state) & 0xffff_ffff) << 8))
        .collect();
    for planted in 0..5 {
        let copy = hashes[planted * 7];
        hashes.push(copy);
    }
    let seeds: Vec<Seed> = hashes.iter().map(|hash| seed(*hash)).collect();
    let (_workspace, library) = library_with(&seeds);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert!(groups.len() >= 5);
    assert!(
        groups.iter().all(|group| group.len() <= 3),
        "{:?}",
        groups.iter().map(Vec::len).collect::<Vec<_>>()
    );
}

#[test]
fn two_resemblances_of_one_asset_that_differ_from_each_other_are_not_grouped_together() {
    let base = 0x0123_4567_89ab_cdef_u64;
    let (_workspace, library) = library_with(&[
        seed(base),
        seed(flipped(base, &[0, 1, 2])),
        seed(flipped(base, &[3, 4, 5])),
    ]);

    let groups = library.duplicate_groups(SIDE_BY_SIDE).expect("grouped");

    assert_eq!(groups, [vec![asset_id(1), asset_id(2)]]);
}
