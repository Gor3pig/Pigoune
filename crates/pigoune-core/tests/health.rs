use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use pigoune_core::{AssetId, HealthProgress, ImportOutcome, Library};

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn import(library: &mut Library, name: &str) -> AssetId {
    match library
        .import_file(&sample(name), None)
        .expect("import succeeds")
    {
        ImportOutcome::Imported(id) => id,
        other => panic!("the resource is new, got {other:?}"),
    }
}

fn stored_file(library: &Library, id: AssetId) -> PathBuf {
    let asset = library.asset(id).expect("query").expect("asset exists");
    library.root().join(asset.stored_path)
}

fn check(library: &Library) -> pigoune_core::HealthReport {
    library
        .health_plan()
        .expect("plan")
        .run(|_| ControlFlow::Continue(()))
        .expect("not cancelled")
}

#[test]
fn an_untouched_library_is_in_order() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    import(&mut library, "red-dot.png");
    import(&mut library, "blue-photo.jpg");

    let report = check(&library);

    assert_eq!(report.checked, 2);
    assert_eq!(report.problems(), 0);
}

#[test]
fn an_empty_library_is_in_order() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let library = Library::create(workspace.path(), "Essai").expect("library");

    let report = check(&library);

    assert_eq!(report.checked, 0);
    assert_eq!(report.problems(), 0);
}

#[test]
fn a_deleted_file_is_reported_as_missing_with_its_collection() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let collection = library
        .create_collection("Marque", None)
        .expect("collection");
    let id = import(&mut library, "red-dot.png");
    library
        .apply_collection_command(&pigoune_core::CollectionCommand::AddAssets {
            collection,
            assets: vec![id],
        })
        .expect("added");
    fs::remove_file(stored_file(&library, id)).expect("file removed");

    let report = check(&library);

    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].id, id);
    assert_eq!(report.missing[0].collection.as_deref(), Some("Marque"));
    assert!(report.damaged.is_empty());
}

#[test]
fn a_modified_file_is_reported_as_damaged() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, "red-dot.png");
    let file = stored_file(&library, id);
    let mut bytes = fs::read(&file).expect("read");
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    fs::write(&file, bytes).expect("rewritten with the same size");

    let report = check(&library);

    assert_eq!(report.damaged.len(), 1);
    assert_eq!(report.damaged[0].id, id);
    assert!(report.missing.is_empty());
}

#[test]
fn a_truncated_file_is_reported_as_damaged() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, "blue-photo.jpg");
    fs::write(stored_file(&library, id), b"x").expect("truncated");

    assert_eq!(check(&library).damaged.len(), 1);
}

#[test]
fn a_file_without_a_record_is_reported_as_unrecorded() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, "red-dot.png");
    let folder = stored_file(&library, id)
        .parent()
        .expect("folder")
        .to_path_buf();
    fs::write(folder.join("stray.png"), b"stray").expect("stray written");

    let report = check(&library);

    assert_eq!(report.unrecorded.len(), 1);
    assert!(report.unrecorded[0].ends_with("stray.png"));
    assert!(report.unrecorded[0].is_relative());
    assert_eq!(report.checked, 1);
}

#[test]
fn files_of_trashed_resources_are_checked_too() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, "red-dot.png");
    library
        .apply_asset_command(&pigoune_core::AssetCommand::SetTrashed {
            assets: vec![id],
            trashed: true,
        })
        .expect("trashed");
    fs::remove_file(stored_file(&library, id)).expect("file removed");

    let report = check(&library);

    assert_eq!(report.checked, 1);
    assert_eq!(report.missing.len(), 1);
}

#[test]
fn an_unfinished_import_is_not_an_unrecorded_file() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    import(&mut library, "red-dot.png");
    let partial = library
        .root()
        .join(pigoune_core::FILES_DIR_NAME)
        .join(".0190aaaa.partial");
    fs::create_dir_all(&partial).expect("partial folder");
    fs::write(partial.join("half.png"), b"half").expect("partial file");

    assert_eq!(check(&library).problems(), 0);
}

#[test]
fn the_check_reports_its_progress_and_can_be_cancelled() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    import(&mut library, "red-dot.png");
    import(&mut library, "blue-photo.jpg");
    import(&mut library, "dark-circle.svg");
    let plan = library.health_plan().expect("plan");
    assert_eq!(plan.total(), 3);

    let mut seen: Vec<HealthProgress> = Vec::new();
    let finished = plan.run(|progress| {
        seen.push(progress);
        ControlFlow::Continue(())
    });
    assert!(finished.is_some());
    assert_eq!(seen.first().map(|progress| progress.done), Some(0));
    assert_eq!(seen.last().map(|progress| progress.done), Some(3));
    assert!(seen.iter().all(|progress| progress.total == 3));

    let cancelled = plan.run(|progress| {
        if progress.done == 1 {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    assert!(cancelled.is_none());
}

#[test]
fn the_check_changes_nothing() {
    let workspace = tempfile::tempdir().expect("temporary directory");
    let mut library = Library::create(workspace.path(), "Essai").expect("library");
    let id = import(&mut library, "red-dot.png");
    fs::remove_file(stored_file(&library, id)).expect("file removed");
    let before = library.visible_assets().expect("assets").len();

    let _ = check(&library);

    assert_eq!(library.visible_assets().expect("assets").len(), before);
}
