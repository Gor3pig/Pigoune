use std::path::Path;

use pigoune_core::{
    AssetCommand, AssetId, ConfirmedRemoval, DATABASE_FILE_NAME, DuplicateError, IgnoreKind,
    IgnoredEntry, Library, RemovalError,
};
use rusqlite::{Connection, params};
use tempfile::TempDir;

fn id(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

fn n(asset: AssetId) -> usize {
    asset.to_string()[24..].parse().expect("number")
}

fn detail() -> Vec<u8> {
    let mut bytes = Vec::new();
    for cell in 0..256 {
        let value: u8 = if cell % 2 == 0 { 230 } else { 20 };
        bytes.extend_from_slice(&[value, value, value, 255]);
    }
    bytes
}

fn add(root: &Path, number: usize, size: u32, hash: u64) {
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("db");
    connection
        .execute(
            "INSERT INTO assets (id, display_name, original_file_name, stored_path, format, width, height,
                                 byte_size, content_hash, is_animated, embedded_sizes, added_at_unix_ms,
                                 visual_hash, colors)
             VALUES (?1, ?2, ?3, ?4, 'png', ?5, ?5, 100, ?6, 0, '', ?7, ?8, NULL)",
            params![
                id(number).to_string(),
                format!("asset {number}"),
                format!("a{number}.png"),
                format!("files/zz/{number}/a{number}.png"),
                size,
                format!("h{number}"),
                i64::try_from(number).expect("small"),
                hash.to_be_bytes().to_vec(),
            ],
        )
        .expect("asset");
    let details = Connection::open(root.join("cache").join("visual-details.db")).expect("details");
    details
        .execute(
            "INSERT INTO asset_details (asset_id, detail) VALUES (?1, ?2)",
            params![id(number).to_string(), detail()],
        )
        .expect("detail");
}

fn library_of(count: usize) -> (TempDir, Library) {
    let workspace = tempfile::tempdir().expect("tmp");
    let library = Library::create(workspace.path(), "Essai").expect("created");
    let root = library.root().to_path_buf();
    drop(library);
    for number in 1..=count {
        add(
            &root,
            number,
            u32::try_from(5000 - number).expect("small"),
            9,
        );
    }
    (workspace, Library::open(&root).expect("opens"))
}

fn library_of_clusters(clusters: &[(usize, u64)]) -> (TempDir, Library) {
    let workspace = tempfile::tempdir().expect("tmp");
    let library = Library::create(workspace.path(), "Essai").expect("created");
    let root = library.root().to_path_buf();
    drop(library);
    let mut number = 1;
    for (count, hash) in clusters {
        for _ in 0..*count {
            add(
                &root,
                number,
                u32::try_from(5000 - number).expect("small"),
                *hash,
            );
            number += 1;
        }
    }
    (workspace, Library::open(&root).expect("opens"))
}

const OTHER_DRAWING: u64 = 0xff00_ff00_ff00_ff00;

fn ids(list: &[usize]) -> Vec<AssetId> {
    list.iter().map(|number| id(*number)).collect()
}

fn sorted(groups: &[Vec<AssetId>]) -> Vec<Vec<usize>> {
    let mut shown: Vec<Vec<usize>> = groups
        .iter()
        .map(|group| {
            let mut members: Vec<usize> = group.iter().map(|asset| n(*asset)).collect();
            members.sort_unstable();
            members
        })
        .collect();
    shown.sort();
    shown
}

fn results(library: &Library) -> Vec<Vec<usize>> {
    sorted(&library.duplicate_groups().expect("groups"))
}

fn entries(library: &Library) -> Vec<IgnoredEntry> {
    library.ignored_duplicate_entries().expect("entries")
}

fn shown(entry: &IgnoredEntry) -> (IgnoreKind, Vec<usize>, Vec<usize>) {
    let mut assets: Vec<usize> = entry.assets.iter().map(|asset| n(*asset)).collect();
    let mut against: Vec<usize> = entry.against.iter().map(|asset| n(*asset)).collect();
    assets.sort_unstable();
    against.sort_unstable();
    (entry.kind, assets, against)
}

fn trash(library: &mut Library, list: &[usize], trashed: bool) {
    library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: ids(list),
            trashed,
        })
        .expect("trashed");
}

#[test]
fn ignoring_a_group_of_two_lists_exactly_that_group() {
    let (_w, mut library) = library_of(2);

    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");

    assert!(results(&library).is_empty());
    let listed = entries(&library);
    assert_eq!(listed.len(), 1);
    assert_eq!(shown(&listed[0]), (IgnoreKind::Group, vec![1, 2], vec![]));
}

#[test]
fn ignoring_one_asset_lists_only_that_asset_and_what_it_is_ignored_against() {
    let (_w, mut library) = library_of(3);

    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");

    assert_eq!(results(&library), [vec![1, 2]]);
    let listed = entries(&library);
    assert_eq!(listed.len(), 1);
    assert_eq!(shown(&listed[0]), (IgnoreKind::Member, vec![3], vec![1, 2]));
    assert_eq!(library.ignored_duplicate_count().expect("count"), 1);
}

#[test]
fn ignoring_the_reference_of_a_group_works_like_any_other_asset() {
    let (_w, mut library) = library_of(3);

    library
        .ignore_duplicate_member(id(1), &ids(&[2, 3]))
        .expect("ignored");

    assert_eq!(results(&library), [vec![2, 3]]);
    assert_eq!(
        shown(&entries(&library)[0]),
        (IgnoreKind::Member, vec![1], vec![2, 3])
    );
}

#[test]
fn ignoring_the_smaller_asset_of_a_pair_hides_the_pair() {
    let (_w, mut library) = library_of(2);

    library
        .ignore_duplicate_member(id(2), &ids(&[1]))
        .expect("ignored");

    assert!(results(&library).is_empty());
    assert_eq!(
        shown(&entries(&library)[0]),
        (IgnoreKind::Member, vec![2], vec![1])
    );
}

#[test]
fn two_assets_ignored_one_after_the_other_are_two_entries() {
    let (_w, mut library) = library_of(4);
    library
        .ignore_duplicate_member(id(4), &ids(&[1, 2, 3]))
        .expect("ignored");

    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");

    let listed = entries(&library);
    assert_eq!(listed.len(), 2);
    let mut subjects: Vec<Vec<usize>> = listed.iter().map(|entry| shown(entry).1).collect();
    subjects.sort();
    assert_eq!(subjects, [vec![3], vec![4]]);
    assert_eq!(results(&library), [vec![1, 2]]);
}

#[test]
fn an_asset_then_the_remaining_group_are_two_entries() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");

    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");

    assert_eq!(entries(&library).len(), 2);
    assert!(results(&library).is_empty());
}

#[test]
fn what_is_already_ignored_cannot_be_ignored_again_as_one_group() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_group(&ids(&[1, 2, 3]))
        .expect("ignored");

    let outcome = library.ignore_duplicate_member(id(3), &ids(&[1, 2]));

    assert!(matches!(outcome, Err(DuplicateError::NotAlike(_))));
    assert_eq!(entries(&library).len(), 1);
}

#[test]
fn ignoring_a_group_keeps_it_apart_from_every_asset_that_still_looks_like_it() {
    let (_w, mut library) = library_of(3);

    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");

    assert!(results(&library).is_empty());
    let listed = entries(&library);
    assert_eq!(listed.len(), 1);
    assert_eq!(shown(&listed[0]), (IgnoreKind::Group, vec![1, 2], vec![3]));
    assert!(library.is_ignored_duplicate(id(1), id(3)).expect("checked"));
    assert!(library.is_ignored_duplicate(id(3), id(2)).expect("checked"));
}

#[test]
fn showing_one_action_again_keeps_the_other_actions() {
    let (_w, mut library) = library_of_clusters(&[(2, 9), (3, OTHER_DRAWING)]);
    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");
    library
        .ignore_duplicate_member(id(3), &ids(&[4, 5]))
        .expect("ignored");
    let listed = entries(&library);
    assert_eq!(listed.len(), 2);
    let group = listed
        .iter()
        .find(|entry| entry.kind == IgnoreKind::Group)
        .expect("a group entry");

    library
        .unignore_duplicate_action(group.action)
        .expect("shown again");

    let left = entries(&library);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].kind, IgnoreKind::Member);
    assert_eq!(results(&library), [vec![1, 2], vec![4, 5]]);
}

#[test]
fn showing_everything_again_can_be_undone_exactly() {
    let (_w, mut library) = library_of_clusters(&[(2, 9), (2, OTHER_DRAWING)]);
    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");
    library
        .ignore_duplicate_group(&ids(&[3, 4]))
        .expect("ignored");
    let before = entries(&library);
    let before_results = results(&library);

    library.unignore_all_duplicates().expect("all");
    assert!(entries(&library).is_empty());
    assert_ne!(results(&library), before_results);
    library.undo().expect("undone");

    assert_eq!(entries(&library), before);
    assert_eq!(results(&library), before_results);
}

#[test]
fn ignoring_goes_through_the_history() {
    let (_w, mut library) = library_of(3);
    let start = (entries(&library), results(&library));
    let outcome = library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");
    let ignored = (entries(&library), results(&library));

    assert!(
        library
            .undo_change(outcome.stamp)
            .expect("undone")
            .is_some()
    );
    assert_eq!((entries(&library), results(&library)), start);
    assert!(library.redo().expect("redone").is_some());
    assert_eq!((entries(&library), results(&library)), ignored);
    library.undo().expect("undone");
    library
        .apply_asset_command(&AssetCommand::SetFavorite {
            assets: ids(&[1]),
            favorite: true,
        })
        .expect("favorite");
    assert!(!library.can_redo());
}

#[test]
fn trashing_an_asset_of_an_ignored_group_hides_it_until_restored() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_group(&ids(&[1, 2, 3]))
        .expect("ignored");

    trash(&mut library, &[3], true);
    assert_eq!(shown(&entries(&library)[0]).1, [1, 2]);

    trash(&mut library, &[3], false);
    assert_eq!(shown(&entries(&library)[0]).1, [1, 2, 3]);
}

#[test]
fn trashing_the_ignored_asset_hides_its_entry_until_restored() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");

    trash(&mut library, &[3], true);
    assert!(entries(&library).is_empty());

    trash(&mut library, &[3], false);
    assert_eq!(entries(&library).len(), 1);
}

#[test]
fn trashing_all_the_others_hides_the_entry_of_an_ignored_asset() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");

    trash(&mut library, &[1, 2], true);

    assert!(entries(&library).is_empty());
}

#[test]
fn deleting_assets_for_good_keeps_what_is_left_of_an_action() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");
    trash(&mut library, &[1], true);

    library.empty_trash().expect("emptied");

    let listed = entries(&library);
    assert_eq!(listed.len(), 1);
    assert_eq!(shown(&listed[0]), (IgnoreKind::Member, vec![3], vec![2]));
}

#[test]
fn deleting_the_ignored_asset_for_good_forgets_the_action() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");
    trash(&mut library, &[3], true);

    library.empty_trash().expect("emptied");

    let root = library.root().to_path_buf();
    drop(library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("db");
    let rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM ignored_duplicate_actions",
            [],
            |row| row.get(0),
        )
        .expect("counted");
    assert_eq!(rows, 0);
}

#[test]
fn a_group_left_with_one_asset_for_good_is_forgotten() {
    let (_w, mut library) = library_of(2);
    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");
    trash(&mut library, &[2], true);

    library.empty_trash().expect("emptied");

    let root = library.root().to_path_buf();
    drop(library);
    let connection = Connection::open(root.join(DATABASE_FILE_NAME)).expect("db");
    let rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM ignored_duplicate_actions",
            [],
            |row| row.get(0),
        )
        .expect("counted");
    assert_eq!(rows, 0);
}

#[test]
fn a_new_look_alike_is_offered_with_one_of_the_ignored_assets_only() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_group(&ids(&[1, 2, 3]))
        .expect("ignored");
    let root = library.root().to_path_buf();
    drop(library);
    add(&root, 4, 1000, 9);
    let library = Library::open(&root).expect("opens");

    let groups = results(&library);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].len(), 2);
    assert!(groups[0].contains(&4));
}

#[test]
fn the_kind_between_two_ignored_assets_is_nothing() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");

    assert!(
        library
            .duplicate_kind_between(id(1), id(3))
            .expect("kind")
            .is_none()
    );
    assert!(
        library
            .duplicate_kind_between(id(2), id(3))
            .expect("kind")
            .is_none()
    );
    assert!(
        library
            .duplicate_kind_between(id(1), id(2))
            .expect("kind")
            .is_some()
    );
    assert!(library.is_ignored_duplicate(id(3), id(1)).expect("checked"));
    assert!(!library.is_ignored_duplicate(id(1), id(2)).expect("checked"));
}

#[test]
fn a_confirmed_removal_is_refused_against_an_ignored_pair() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");
    let kept_against_ignored = ConfirmedRemoval::new(id(2), vec![id(3)]).expect("built");

    let outcome = library.trash_confirmed_duplicates(&kept_against_ignored);

    assert!(matches!(outcome, Err(RemovalError::NotAlike(_))));
    let others = ConfirmedRemoval::new(id(1), vec![id(2)]).expect("built");
    assert!(library.trash_confirmed_duplicates(&others).is_ok());
}

#[test]
fn ignored_duplicates_survive_closing_the_library() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_member(id(3), &ids(&[1, 2]))
        .expect("ignored");
    let before = entries(&library);
    let root = library.root().to_path_buf();
    drop(library);

    let library = Library::open(&root).expect("reopens");

    assert_eq!(entries(&library), before);
    assert!(!library.can_undo());
}

#[test]
fn assets_that_do_not_look_alike_cannot_be_ignored_together() {
    let workspace = tempfile::tempdir().expect("tmp");
    let library = Library::create(workspace.path(), "Essai").expect("created");
    let root = library.root().to_path_buf();
    drop(library);
    add(&root, 1, 1000, 9);
    add(&root, 2, 900, 0xff00_ff00_ff00_ff00);
    let mut library = Library::open(&root).expect("opens");

    let outcome = library.ignore_duplicate_group(&ids(&[1, 2]));

    assert!(matches!(outcome, Err(DuplicateError::NotAlike(_))));
    assert!(entries(&library).is_empty());
}

#[test]
fn unknown_trashed_or_single_assets_are_refused() {
    let (_w, mut library) = library_of(3);
    trash(&mut library, &[3], true);

    assert!(matches!(
        library.ignore_duplicate_group(&ids(&[1, 99])),
        Err(DuplicateError::NotFound(_))
    ));
    assert!(matches!(
        library.ignore_duplicate_group(&ids(&[1, 3])),
        Err(DuplicateError::NotAvailable(_))
    ));
    assert!(matches!(
        library.ignore_duplicate_group(&ids(&[1])),
        Err(DuplicateError::NothingChanged)
    ));
    assert!(entries(&library).is_empty());
}

#[test]
fn ignoring_a_group_of_a_larger_cluster_lists_only_that_group() {
    let (_w, mut library) = library_of(100);
    let first = library.duplicate_groups().expect("groups");
    assert_eq!(first.iter().map(Vec::len).collect::<Vec<_>>(), [40, 40, 20]);

    library.ignore_duplicate_group(&first[0]).expect("ignored");

    let listed = entries(&library);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].assets.len(), 40);
    assert_eq!(listed[0].against.len(), 60);
    library.undo().expect("undone");
    assert_eq!(library.duplicate_groups().expect("groups").len(), 3);
}

#[test]
fn the_other_groups_of_the_cluster_are_still_offered_without_the_ignored_group() {
    let (_w, mut library) = library_of(100);
    let first = library.duplicate_groups().expect("groups");

    library.ignore_duplicate_group(&first[0]).expect("ignored");

    let remaining = library.duplicate_groups().expect("groups");
    assert_eq!(remaining.iter().map(Vec::len).collect::<Vec<_>>(), [40, 20]);
    let ignored: Vec<AssetId> = first[0].clone();
    assert!(
        remaining
            .iter()
            .flatten()
            .all(|asset| !ignored.contains(asset))
    );
}

#[test]
fn a_cluster_of_two_groups_keeps_the_second_one_after_ignoring_the_first() {
    let (_w, mut library) = library_of_clusters(&[(3, 9), (2, OTHER_DRAWING)]);

    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");

    assert!(library.is_ignored_duplicate(id(1), id(3)).expect("checked"));
    assert!(!library.is_ignored_duplicate(id(4), id(5)).expect("checked"));
    assert_eq!(results(&library), [vec![4, 5]]);
}

#[test]
fn undoing_and_redoing_the_ignoring_of_a_group_keeps_who_it_was_kept_apart_from() {
    let (_w, mut library) = library_of(4);
    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");
    let before = entries(&library);

    library.undo().expect("undone");
    assert!(entries(&library).is_empty());
    library.redo().expect("redone");

    assert_eq!(entries(&library), before);
    assert_eq!(
        shown(&before[0]),
        (IgnoreKind::Group, vec![1, 2], vec![3, 4])
    );
}

#[test]
fn trashing_an_asset_kept_apart_hides_it_from_the_entry_until_restored() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");

    trash(&mut library, &[3], true);
    assert_eq!(
        shown(&entries(&library)[0]),
        (IgnoreKind::Group, vec![1, 2], vec![])
    );
    trash(&mut library, &[3], false);

    assert_eq!(
        shown(&entries(&library)[0]),
        (IgnoreKind::Group, vec![1, 2], vec![3])
    );
}

#[test]
fn a_dense_cluster_stays_quick_to_ignore_and_to_show_again() {
    let (_w, mut library) = library_of(400);
    let first = library.duplicate_groups().expect("groups");
    let start = std::time::Instant::now();

    library.ignore_duplicate_group(&first[0]).expect("ignored");
    library.unignore_all_duplicates().expect("shown again");

    assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
}

#[test]
fn no_group_ever_holds_an_ignored_pair() {
    let mut state = 7_u64;
    let (workspace, library) = library_of(1);
    let root = library.root().to_path_buf();
    drop(library);
    for number in 2..=600 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let hash = (state >> 40) % 40;
        add(
            &root,
            number,
            u32::try_from(5000 - number).expect("small"),
            hash | 0x0f00,
        );
    }
    let mut library = Library::open(&root).expect("opens");
    let _ = &workspace;
    for round in 0..5 {
        let groups = library.duplicate_groups().expect("groups");
        for group in &groups {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            match (state >> 33) % 3 {
                0 => {
                    let _ = library.ignore_duplicate_group(group);
                }
                1 if group.len() > 2 => {
                    let _ = library.ignore_duplicate_member(group[0], &group[1..]);
                }
                _ => {}
            }
        }
        for group in library.duplicate_groups().expect("groups") {
            for (position, first) in group.iter().enumerate() {
                for second in &group[position + 1..] {
                    assert!(
                        !library
                            .is_ignored_duplicate(*first, *second)
                            .expect("checked"),
                        "round {round}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_count_of_ignored_entries_always_matches_the_list() {
    let (_w, mut library) = library_of_clusters(&[(4, 9), (3, OTHER_DRAWING)]);
    let counted = |library: &Library| library.ignored_duplicate_count().expect("count");
    assert_eq!(counted(&library), 0);

    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");
    library
        .ignore_duplicate_member(id(5), &ids(&[6, 7]))
        .expect("ignored");
    assert_eq!(counted(&library), entries(&library).len());
    assert_eq!(counted(&library), 2);

    trash(&mut library, &[1], true);
    assert_eq!(counted(&library), entries(&library).len());
    assert_eq!(counted(&library), 2);

    trash(&mut library, &[5], true);
    assert_eq!(counted(&library), entries(&library).len());
    assert_eq!(counted(&library), 1);

    trash(&mut library, &[1, 5], false);
    assert_eq!(counted(&library), entries(&library).len());
    assert_eq!(counted(&library), 2);
}

#[test]
fn the_count_of_ignored_assets_counts_each_asset_once() {
    let (_w, mut library) = library_of_clusters(&[(4, 9), (3, OTHER_DRAWING)]);
    let counted = |library: &Library| library.ignored_asset_count().expect("count");
    let listed = |library: &Library| {
        let mut all: Vec<usize> = entries(library)
            .iter()
            .flat_map(|entry| entry.assets.iter().map(|asset| n(*asset)))
            .collect();
        all.sort_unstable();
        all.dedup();
        all.len()
    };
    assert_eq!(counted(&library), 0);

    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");
    assert_eq!(counted(&library), 2);
    library
        .ignore_duplicate_member(id(3), &ids(&[4]))
        .expect("ignored");
    library
        .ignore_duplicate_member(id(5), &ids(&[6, 7]))
        .expect("ignored");
    assert_eq!(counted(&library), 4);
    assert_eq!(counted(&library), listed(&library));

    trash(&mut library, &[1], true);
    assert_eq!(counted(&library), listed(&library));
    assert_eq!(counted(&library), 3);
    trash(&mut library, &[5], true);
    assert_eq!(counted(&library), listed(&library));
    assert_eq!(counted(&library), 2);
}

#[test]
fn a_group_left_with_one_asset_stays_while_it_still_keeps_others_apart() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");

    trash(&mut library, &[2], true);

    let listed = entries(&library);
    assert_eq!(listed.len(), 1);
    assert_eq!(shown(&listed[0]), (IgnoreKind::Group, vec![1], vec![3]));
    assert_eq!(library.ignored_duplicate_count().expect("count"), 1);
    assert_eq!(library.ignored_asset_count().expect("count"), 1);
    assert!(library.is_ignored_duplicate(id(1), id(3)).expect("checked"));
}

#[test]
fn deleting_for_good_one_asset_of_a_group_keeps_what_it_kept_apart() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_group(&ids(&[1, 2]))
        .expect("ignored");
    trash(&mut library, &[2], true);

    library.empty_trash().expect("emptied");

    let listed = entries(&library);
    assert_eq!(listed.len(), 1);
    assert_eq!(shown(&listed[0]), (IgnoreKind::Group, vec![1], vec![3]));
    assert!(library.is_ignored_duplicate(id(1), id(3)).expect("checked"));
    assert!(results(&library).is_empty());

    trash(&mut library, &[1], true);
    library.empty_trash().expect("emptied");

    assert!(entries(&library).is_empty());
}

#[test]
fn deleting_for_good_the_others_of_a_single_asset_group_forgets_it() {
    let (_w, mut library) = library_of(3);
    library
        .ignore_duplicate_group(&ids(&[1, 2, 3]))
        .expect("ignored");
    trash(&mut library, &[2, 3], true);

    library.empty_trash().expect("emptied");

    assert!(entries(&library).is_empty());
}
