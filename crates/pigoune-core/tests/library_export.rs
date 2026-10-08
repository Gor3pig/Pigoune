use std::fs;
use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetId, CollectionCommand, CollectionId, ImportOutcome, Library,
};

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn import(library: &mut Library, name: &str, collection: Option<CollectionId>) -> AssetId {
    match library
        .import_file(&sample(name), collection)
        .expect("import succeeds")
    {
        ImportOutcome::Imported(id) => id,
        other => panic!("the resource is new, got {other:?}"),
    }
}

fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .expect("folder is readable")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

struct Setup {
    _workspace: tempfile::TempDir,
    destination: tempfile::TempDir,
    library: Library,
}

impl Setup {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library");
        Self {
            _workspace: workspace,
            destination: tempfile::tempdir().expect("destination"),
            library,
        }
    }

    fn collection(&mut self, name: &str, parent: Option<CollectionId>) -> CollectionId {
        self.library
            .create_collection(name, parent)
            .expect("collection is created")
    }

    fn plan(&self) -> pigoune_core::LibraryExportPlan {
        self.library
            .library_export_plan("Non classés")
            .expect("plan")
    }

    fn export(&self) -> pigoune_core::LibraryExportReport {
        self.plan()
            .run(self.destination.path())
            .expect("export succeeds")
    }
}

#[test]
fn the_library_becomes_a_folder_with_its_collections_as_sub_folders() {
    let mut setup = Setup::new();
    let logos = setup.collection("Logos", None);
    let clients = setup.collection("Clients", Some(logos));
    let former = setup.collection("Former", Some(clients));
    let icons = setup.collection("Icons", None);
    import(&mut setup.library, "red-dot.png", Some(logos));
    import(&mut setup.library, "blue-photo.jpg", Some(clients));
    import(&mut setup.library, "dark-circle.svg", Some(former));
    import(&mut setup.library, "green-square.webp", Some(icons));

    let report = setup.export();

    assert_eq!(report.exported, 4);
    assert!(report.failures.is_empty());
    assert_eq!(report.folder, setup.destination.path().join("Essai"));
    assert_eq!(names_in(&report.folder), ["Icons", "Logos"]);
    assert_eq!(
        names_in(&report.folder.join("Logos")),
        ["Clients", "red-dot.png"]
    );
    assert_eq!(
        names_in(&report.folder.join("Logos/Clients")),
        ["Former", "blue-photo.jpg"]
    );
    assert_eq!(
        names_in(&report.folder.join("Logos/Clients/Former")),
        ["dark-circle.svg"]
    );
}

#[test]
fn unclassified_resources_go_into_a_folder_with_the_given_name() {
    let mut setup = Setup::new();
    let logos = setup.collection("Logos", None);
    import(&mut setup.library, "red-dot.png", Some(logos));
    import(&mut setup.library, "blue-photo.jpg", None);

    let report = setup.export();

    assert_eq!(names_in(&report.folder), ["Logos", "Non classés"]);
    assert_eq!(
        names_in(&report.folder.join("Non classés")),
        ["blue-photo.jpg"]
    );
}

#[test]
fn there_is_no_unclassified_folder_when_everything_is_classified() {
    let mut setup = Setup::new();
    let logos = setup.collection("Logos", None);
    import(&mut setup.library, "red-dot.png", Some(logos));

    let report = setup.export();

    assert_eq!(names_in(&report.folder), ["Logos"]);
}

#[test]
fn a_collection_with_the_same_name_keeps_it_and_the_unclassified_folder_is_numbered() {
    let mut setup = Setup::new();
    let own = setup.collection("Non classés", None);
    import(&mut setup.library, "red-dot.png", Some(own));
    import(&mut setup.library, "blue-photo.jpg", None);

    let report = setup.export();

    assert_eq!(names_in(&report.folder), ["Non classés", "Non classés (2)"]);
    assert_eq!(
        names_in(&report.folder.join("Non classés")),
        ["red-dot.png"]
    );
    assert_eq!(
        names_in(&report.folder.join("Non classés (2)")),
        ["blue-photo.jpg"]
    );
}

#[test]
fn a_resource_in_two_collections_is_copied_into_each() {
    let mut setup = Setup::new();
    let first = setup.collection("First", None);
    let second = setup.collection("Second", None);
    let dot = import(&mut setup.library, "red-dot.png", Some(first));
    setup
        .library
        .apply_collection_command(&CollectionCommand::AddAssets {
            collection: second,
            assets: vec![dot],
        })
        .expect("resource is added");

    let report = setup.export();

    assert_eq!(report.exported, 2);
    assert_eq!(names_in(&report.folder.join("First")), ["red-dot.png"]);
    assert_eq!(names_in(&report.folder.join("Second")), ["red-dot.png"]);
}

#[test]
fn the_exported_files_are_identical_to_the_stored_ones() {
    let mut setup = Setup::new();
    import(&mut setup.library, "blue-photo.jpg", None);

    let report = setup.export();

    assert_eq!(
        fs::read(report.folder.join("Non classés/blue-photo.jpg")).expect("read"),
        fs::read(sample("blue-photo.jpg")).expect("read")
    );
}

#[test]
fn empty_collections_still_get_their_folder() {
    let mut setup = Setup::new();
    let pack = setup.collection("Pack", None);
    setup.collection("Later", Some(pack));
    setup.collection("Empty", None);
    import(&mut setup.library, "red-dot.png", Some(pack));

    let report = setup.export();

    assert_eq!(names_in(&report.folder), ["Empty", "Pack"]);
    assert!(names_in(&report.folder.join("Empty")).is_empty());
    assert!(names_in(&report.folder.join("Pack/Later")).is_empty());
}

#[test]
fn a_library_without_resources_has_a_total_of_zero() {
    let mut setup = Setup::new();
    setup.collection("Empty", None);

    assert_eq!(setup.plan().total(), 0);
}

#[test]
fn resources_in_the_trash_are_not_exported() {
    let mut setup = Setup::new();
    let pack = setup.collection("Pack", None);
    let dot = import(&mut setup.library, "red-dot.png", Some(pack));
    import(&mut setup.library, "blue-photo.jpg", Some(pack));
    import(&mut setup.library, "dark-circle.svg", None);
    let loose = setup
        .library
        .visible_assets_in(pigoune_core::AssetView::Unclassified)
        .expect("assets")[0]
        .id;
    setup
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![dot, loose],
            trashed: true,
        })
        .expect("resources are trashed");

    let report = setup.export();

    assert_eq!(names_in(&report.folder), ["Pack"]);
    assert_eq!(names_in(&report.folder.join("Pack")), ["blue-photo.jpg"]);
}

#[test]
fn an_existing_folder_is_never_overwritten() {
    let mut setup = Setup::new();
    import(&mut setup.library, "red-dot.png", None);
    fs::create_dir(setup.destination.path().join("Essai")).expect("existing folder");
    fs::write(setup.destination.path().join("Essai/mine.txt"), b"mine").expect("file");

    let report = setup.export();

    assert_eq!(report.folder, setup.destination.path().join("Essai (2)"));
    assert_eq!(
        names_in(&setup.destination.path().join("Essai")),
        ["mine.txt"]
    );
}

#[test]
fn forbidden_characters_in_collection_names_are_replaced() {
    let mut setup = Setup::new();
    let pack = setup.collection("Logos / Icons", None);
    import(&mut setup.library, "red-dot.png", Some(pack));

    let report = setup.export();

    assert_eq!(names_in(&report.folder), ["Logos - Icons"]);
}

#[test]
fn names_that_clash_once_cleaned_are_numbered() {
    let mut setup = Setup::new();
    let first = setup.collection("A/B", None);
    let second = setup.collection("A-B", None);
    import(&mut setup.library, "red-dot.png", Some(first));
    import(&mut setup.library, "blue-photo.jpg", Some(second));

    let report = setup.export();

    assert_eq!(names_in(&report.folder), ["A-B", "A-B (2)"]);
}

#[test]
fn a_missing_stored_file_is_reported_and_the_others_are_exported() {
    let mut setup = Setup::new();
    let pack = setup.collection("Pack", None);
    let dot = import(&mut setup.library, "red-dot.png", Some(pack));
    import(&mut setup.library, "blue-photo.jpg", Some(pack));
    let asset = setup.library.asset(dot).expect("query").expect("asset");
    fs::remove_file(setup.library.file_of(&asset)).expect("file removed");

    let report = setup.export();

    assert_eq!(report.exported, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].name, asset.display_name);
    assert_eq!(names_in(&report.folder.join("Pack")), ["blue-photo.jpg"]);
}

#[test]
fn a_missing_destination_is_refused() {
    let mut setup = Setup::new();
    import(&mut setup.library, "red-dot.png", None);

    let result = setup.plan().run(&setup.destination.path().join("nowhere"));

    assert!(result.is_err());
}

#[test]
fn the_outline_shows_the_folders_with_their_file_counts() {
    let mut setup = Setup::new();
    let logos = setup.collection("Logos", None);
    let clients = setup.collection("Clients", Some(logos));
    let icons = setup.collection("Icons", None);
    import(&mut setup.library, "red-dot.png", Some(logos));
    import(&mut setup.library, "blue-photo.jpg", Some(clients));
    import(&mut setup.library, "dark-circle.svg", Some(clients));
    import(&mut setup.library, "green-square.webp", Some(icons));
    import(&mut setup.library, "spinner.gif", None);

    let outline = setup
        .library
        .export_outline("Non classés")
        .expect("outline");

    let lines: Vec<(usize, &str, usize)> = outline
        .folders
        .iter()
        .map(|folder| (folder.depth, folder.name.as_str(), folder.files))
        .collect();
    assert_eq!(outline.root, "Essai");
    assert_eq!(
        lines,
        [
            (1, "Logos", 3),
            (2, "Clients", 2),
            (1, "Icons", 1),
            (1, "Non classés", 1),
        ]
    );
    assert_eq!(outline.more, 0);
}

#[test]
fn the_outline_keeps_a_line_for_the_unclassified_folder_and_counts_the_others() {
    let mut setup = Setup::new();
    for index in 0..8 {
        setup.collection(&format!("Pack {index}"), None);
    }
    import(&mut setup.library, "red-dot.png", None);

    let outline = setup
        .library
        .export_outline("Non classés")
        .expect("outline");

    assert_eq!(outline.folders.len(), 5);
    assert_eq!(outline.folders.last().expect("last").name, "Non classés");
    assert_eq!(outline.more, 4);
}

#[test]
fn the_outline_of_an_empty_library_has_only_its_name() {
    let setup = Setup::new();

    let outline = setup
        .library
        .export_outline("Non classés")
        .expect("outline");

    assert!(outline.folders.is_empty());
    assert_eq!(outline.more, 0);
}
