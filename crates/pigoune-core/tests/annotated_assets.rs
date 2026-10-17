use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, BLUR, DocumentError, Geometry, Options, Rect, Shape,
};
use pigoune_core::{
    AnnotatedError, AssetCommand, AssetId, AssetView, ImportOutcome, Library, TagCommand, TextField,
};
use rusqlite::Connection;
use tempfile::TempDir;

fn blur_shape(x: f32) -> Shape {
    Shape::new(
        "blur",
        Geometry::Box(Rect {
            x,
            y: 2.0,
            width: 30.0,
            height: 40.0,
        }),
        Options::defaults(BLUR.options),
    )
}

fn document_with_a_blur(x: f32) -> AnnotationDocument {
    AnnotationDocument::new(None, vec![AnnotationObject::new(1, blur_shape(x))])
}

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

struct Fixture {
    _workspace: TempDir,
    sources: TempDir,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library");
        Self {
            _workspace: workspace,
            sources: tempfile::tempdir().expect("sources"),
            library,
        }
    }

    fn png(&self, name: &str, number: usize) -> PathBuf {
        let mut bytes = fs::read(sample("red-dot.png")).expect("sample is read");
        bytes.extend_from_slice(number.to_string().as_bytes());
        let path = self.sources.path().join(name);
        fs::write(&path, bytes).expect("source written");
        path
    }

    fn import(&mut self, name: &str, number: usize) -> AssetId {
        let path = self.png(name, number);
        let ImportOutcome::Imported(id) = self.library.import_file(&path, None).expect("imported")
        else {
            panic!("a new image");
        };
        id
    }

    fn stored_file(&self, id: AssetId) -> PathBuf {
        let asset = self.library.asset(id).expect("q").expect("asset");
        self.library.root().join(asset.stored_path)
    }

    fn files_on_disk(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();
        collect(&self.library.root().join("files"), &mut found);
        found.sort();
        found
    }

    fn problems(&self) -> usize {
        self.library
            .health_plan()
            .expect("plan")
            .run(|_| ControlFlow::Continue(()))
            .expect("not cancelled")
            .problems()
    }

    fn database(&self) -> Connection {
        Connection::open(self.library.root().join("library.db")).expect("database opens")
    }
}

fn collect(folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            collect(&entry.path(), found);
        } else {
            found.push(entry.path());
        }
    }
}

#[test]
fn annotating_an_existing_resource_attaches_a_document_and_never_touches_the_file() {
    let mut fixture = Fixture::new();
    let id = fixture.import("photo.png", 1);
    let before = fs::read(fixture.stored_file(id)).expect("file");
    let asset_before = fixture.library.asset(id).expect("q").expect("asset");
    assert!(fixture.library.annotations(id).expect("q").is_none());

    fixture
        .library
        .save_annotations(id, &document_with_a_blur(1.0))
        .expect("saved");

    assert_eq!(fs::read(fixture.stored_file(id)).expect("file"), before);
    let asset_after = fixture.library.asset(id).expect("q").expect("asset");
    assert_eq!(asset_after.content_hash, asset_before.content_hash);
    assert_eq!(asset_after.stored_path, asset_before.stored_path);
    assert_eq!(asset_after.byte_size, asset_before.byte_size);
    assert_eq!(fixture.files_on_disk().len(), 1, "no copy, no extra file");
    assert_eq!(fixture.library.annotated_ids().expect("ids").len(), 1);
    assert_eq!(
        fixture
            .library
            .annotation_document(id)
            .expect("q")
            .expect("some"),
        document_with_a_blur(1.0)
    );
    assert_eq!(fixture.problems(), 0);
}

#[test]
fn successive_saves_replace_the_document_of_the_same_resource() {
    let mut fixture = Fixture::new();
    let id = fixture.import("photo.png", 1);
    fixture
        .library
        .save_annotations(id, &document_with_a_blur(1.0))
        .expect("first");
    fixture
        .library
        .save_annotations(id, &document_with_a_blur(9.0))
        .expect("second");

    assert_eq!(fixture.asset_count(), 1);
    assert_eq!(
        fixture
            .library
            .annotation_document(id)
            .expect("q")
            .expect("some"),
        document_with_a_blur(9.0)
    );
    let rows: i64 = fixture
        .database()
        .query_row("SELECT count(*) FROM asset_annotations", [], |row| {
            row.get(0)
        })
        .expect("counted");
    assert_eq!(rows, 1);
}

impl Fixture {
    fn asset_count(&self) -> usize {
        self.library
            .visible_assets_in(AssetView::All)
            .expect("listed")
            .len()
    }
}

#[test]
fn saving_an_empty_document_removes_the_annotations() {
    let mut fixture = Fixture::new();
    let id = fixture.import("photo.png", 1);
    fixture
        .library
        .save_annotations(id, &document_with_a_blur(1.0))
        .expect("saved");
    fixture
        .library
        .save_annotations(id, &AnnotationDocument::default())
        .expect("emptied");
    assert!(fixture.library.annotations(id).expect("q").is_none());
    assert!(fixture.library.annotated_ids().expect("ids").is_empty());
}

#[test]
fn favorites_notes_tags_and_names_are_untouched_by_annotating() {
    let mut fixture = Fixture::new();
    let id = fixture.import("photo.png", 1);
    fixture
        .library
        .apply_asset_command(&AssetCommand::Batch(vec![
            AssetCommand::SetFavorite {
                assets: vec![id],
                favorite: true,
            },
            AssetCommand::SetText {
                asset: id,
                field: TextField::Note,
                value: "holiday".to_owned(),
            },
            AssetCommand::Rename {
                asset: id,
                name: "Holiday".to_owned(),
            },
        ]))
        .expect("metadata");
    fixture
        .library
        .apply_tag_command(&TagCommand::Add {
            assets: vec![id],
            name: "trip".to_owned(),
        })
        .expect("tag");
    let before = fixture.library.asset(id).expect("q").expect("asset");

    fixture
        .library
        .save_annotations(id, &document_with_a_blur(1.0))
        .expect("saved");

    let after = fixture.library.asset(id).expect("q").expect("asset");
    assert_eq!(before, after);
    assert!(after.is_favorite);
    assert_eq!(after.note, "holiday");
    assert_eq!(after.display_name, "Holiday");
    assert_eq!(fixture.library.tags_of(id).expect("tags").len(), 1);
}

#[test]
fn annotating_an_unknown_resource_is_refused() {
    let mut fixture = Fixture::new();
    let elsewhere = tempfile::tempdir().expect("temporary directory");
    let mut other = Library::create(elsewhere.path(), "Autre").expect("library");
    let ImportOutcome::Imported(unknown) = other
        .import_file(&sample("red-dot.png"), None)
        .expect("imported")
    else {
        panic!("new");
    };
    assert!(matches!(
        fixture
            .library
            .save_annotations(unknown, &document_with_a_blur(1.0)),
        Err(AnnotatedError::AssetNotFound(id)) if id == unknown
    ));
}

#[test]
fn a_new_capture_is_imported_with_its_document_in_one_call() {
    let mut fixture = Fixture::new();
    let capture = fixture.png("capture.png", 7);

    let id = fixture
        .library
        .import_with_annotations(&capture, &document_with_a_blur(1.0))
        .expect("imported");

    assert_eq!(
        fs::read(fixture.stored_file(id)).expect("stored"),
        fs::read(&capture).expect("source")
    );
    assert_eq!(
        fixture
            .library
            .annotation_document(id)
            .expect("q")
            .expect("some"),
        document_with_a_blur(1.0)
    );
    assert_eq!(fixture.files_on_disk().len(), 1);
    assert_eq!(fixture.problems(), 0);
}

#[test]
fn an_image_already_in_the_library_is_refused_and_names_the_existing_resource() {
    let mut fixture = Fixture::new();
    let existing = fixture.import("a.png", 1);
    let again = fixture.png("b.png", 1);

    assert!(matches!(
        fixture
            .library
            .import_with_annotations(&again, &document_with_a_blur(1.0)),
        Err(AnnotatedError::AlreadyPresent(id)) if id == existing
    ));
    assert!(fixture.library.annotations(existing).expect("q").is_none());
    assert_eq!(fixture.asset_count(), 1);
}

#[test]
fn a_document_that_cannot_be_written_is_refused_without_a_trace() {
    let mut fixture = Fixture::new();
    let capture = fixture.png("capture.png", 1);
    let broken =
        AnnotationDocument::new(None, vec![AnnotationObject::new(1, blur_shape(f32::NAN))]);

    assert!(matches!(
        fixture.library.import_with_annotations(&capture, &broken),
        Err(AnnotatedError::Document(_))
    ));
    assert_eq!(fixture.asset_count(), 0);
    assert!(fixture.files_on_disk().is_empty());

    let id = fixture.import("photo.png", 2);
    assert!(matches!(
        fixture.library.save_annotations(id, &broken),
        Err(AnnotatedError::Document(_))
    ));
    assert!(fixture.library.annotations(id).expect("q").is_none());
}

#[test]
fn trashing_and_restoring_keeps_the_annotations_and_deleting_for_good_removes_them() {
    let mut fixture = Fixture::new();
    let id = fixture.import("photo.png", 1);
    fixture
        .library
        .save_annotations(id, &document_with_a_blur(1.0))
        .expect("saved");
    for trashed in [true, false] {
        fixture
            .library
            .apply_asset_command(&AssetCommand::SetTrashed {
                assets: vec![id],
                trashed,
            })
            .expect("moved");
        assert!(fixture.library.annotations(id).expect("q").is_some());
    }
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![id],
            trashed: true,
        })
        .expect("trashed");
    assert_eq!(fixture.library.empty_trash().expect("emptied"), 1);

    assert!(fixture.files_on_disk().is_empty());
    let rows: i64 = fixture
        .database()
        .query_row("SELECT count(*) FROM asset_annotations", [], |row| {
            row.get(0)
        })
        .expect("counted");
    assert_eq!(rows, 0);
}

#[test]
fn a_library_reopened_still_has_its_annotations() {
    let mut fixture = Fixture::new();
    let id = fixture.import("photo.png", 1);
    fixture
        .library
        .save_annotations(id, &document_with_a_blur(4.0))
        .expect("saved");
    let root = fixture.library.root().to_path_buf();
    let spare = tempfile::tempdir().expect("spare folder");
    let parked = Library::create(spare.path(), "Parking").expect("parking library");
    drop(std::mem::replace(&mut fixture.library, parked));

    let reopened = Library::open(&root).expect("reopened");

    assert_eq!(
        reopened.annotation_document(id).expect("q").expect("some"),
        document_with_a_blur(4.0)
    );
}

#[test]
fn an_unknown_tool_survives_a_trip_through_the_library() {
    let mut fixture = Fixture::new();
    let id = fixture.import("photo.png", 1);
    let document = AnnotationDocument::from_json(
        r#"{"version":1,"objects":[{"id":1,"kind":"lasso","points":[[1,2]]}],"frame":{"margin":3}}"#,
    )
    .expect("read");

    fixture
        .library
        .save_annotations(id, &document)
        .expect("saved");

    assert_eq!(
        fixture
            .library
            .annotation_document(id)
            .expect("q")
            .expect("some"),
        document
    );
}

#[test]
fn a_document_written_by_a_newer_version_is_reported_but_the_image_stays_usable() {
    let mut fixture = Fixture::new();
    let id = fixture.import("photo.png", 1);
    fixture
        .library
        .save_annotations(id, &document_with_a_blur(1.0))
        .expect("saved");
    fixture
        .database()
        .execute(
            "UPDATE asset_annotations SET document = '{\"version\":9,\"objects\":[]}'",
            [],
        )
        .expect("document replaced");

    assert!(matches!(
        fixture.library.annotation_document(id),
        Err(AnnotatedError::Document(DocumentError::TooNew {
            found: 9,
            ..
        }))
    ));
    assert!(fixture.library.annotations(id).expect("q").is_some());
    assert!(fixture.stored_file(id).is_file());
}

#[test]
fn a_document_with_a_blur_is_flagged_and_a_plain_one_is_not() {
    let mut fixture = Fixture::new();
    let plain = fixture.import("a.png", 1);
    let blurred = fixture.import("b.png", 2);
    let rectangle = Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
        }),
        Options::new(),
    );
    fixture
        .library
        .save_annotations(
            plain,
            &AnnotationDocument::new(None, vec![AnnotationObject::new(1, rectangle)]),
        )
        .expect("saved");
    fixture
        .library
        .save_annotations(blurred, &document_with_a_blur(1.0))
        .expect("saved");

    assert!(
        !fixture
            .library
            .annotations(plain)
            .expect("q")
            .expect("some")
            .has_masks
    );
    assert!(
        fixture
            .library
            .annotations(blurred)
            .expect("q")
            .expect("some")
            .has_masks
    );
}

#[test]
fn the_render_key_follows_the_image_and_the_document_and_nothing_else() {
    let mut fixture = Fixture::new();
    let first = fixture.import("a.png", 1);
    let second = fixture.import("b.png", 2);
    assert_eq!(
        fixture.library.annotation_render_key(first).expect("q"),
        None
    );
    for id in [first, second] {
        fixture
            .library
            .save_annotations(id, &document_with_a_blur(1.0))
            .expect("saved");
    }
    let key = fixture
        .library
        .annotation_render_key(first)
        .expect("q")
        .expect("key");
    assert_ne!(
        fixture
            .library
            .annotation_render_key(second)
            .expect("q")
            .expect("key"),
        key,
        "two images with the same document do not share a rendering"
    );
    fixture
        .library
        .save_annotations(first, &document_with_a_blur(1.0))
        .expect("saved again");
    assert_eq!(
        fixture.library.annotation_render_key(first).expect("q"),
        Some(key.clone()),
        "saving the same document again changes nothing"
    );
    fixture
        .library
        .save_annotations(first, &document_with_a_blur(2.0))
        .expect("changed");
    let changed = fixture
        .library
        .annotation_render_key(first)
        .expect("q")
        .expect("key");
    assert_ne!(changed, key);
    let mut hidden = document_with_a_blur(2.0);
    hidden.objects[0].state.hidden = true;
    fixture
        .library
        .save_annotations(first, &hidden)
        .expect("hidden");
    assert_ne!(
        fixture
            .library
            .annotation_render_key(first)
            .expect("q")
            .expect("key"),
        changed,
        "hiding a layer changes the picture"
    );
}
