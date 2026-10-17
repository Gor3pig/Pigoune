use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, BLUR, DocumentError, Geometry, Options, Rect, Shape,
};
use pigoune_core::{
    AnnotatedError, AnnotatedImage, AssetColor, AssetCommand, AssetId, AssetView, DominantColor,
    ImportOutcome, Library, Rgb,
};
use rusqlite::Connection;
use tempfile::TempDir;

fn plain_document() -> AnnotationDocument {
    AnnotationDocument::default()
}

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

fn document_with_a_blur() -> AnnotationDocument {
    AnnotationDocument::new(None, vec![AnnotationObject::new(1, blur_shape(1.0))])
}

fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

struct Fixture {
    workspace: TempDir,
    sources: TempDir,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library");
        Self {
            workspace,
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

    fn import(&mut self, name: &str, rendered: usize, original: usize) -> AssetId {
        let rendered = self.png(name, rendered);
        let original = self.png("original-source.png", original);
        self.library
            .import_annotated(AnnotatedImage {
                rendered: &rendered,
                original: &original,
                document: &plain_document(),
            })
            .expect("annotated image is imported")
    }

    fn stored_file(&self, id: AssetId) -> PathBuf {
        let asset = self
            .library
            .asset(id)
            .expect("query")
            .expect("asset exists");
        self.library.root().join(asset.stored_path)
    }

    fn asset_count(&self) -> usize {
        self.library
            .visible_assets_in(AssetView::All)
            .expect("listed")
            .len()
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
fn an_annotated_image_keeps_its_rendering_its_original_and_its_document() {
    let mut fixture = Fixture::new();

    let id = fixture.import("Capture 2026-10-15 14-32.png", 1, 2);

    assert_eq!(fixture.asset_count(), 1);
    let asset = fixture.library.asset(id).expect("query").expect("exists");
    assert_eq!(asset.display_name, "Capture 2026-10-15 14-32");
    assert_eq!(
        fs::read(fixture.stored_file(id)).expect("rendering"),
        fs::read(fixture.png("again.png", 1)).expect("source")
    );
    let original = fixture
        .library
        .original_path(id)
        .expect("query")
        .expect("annotated");
    assert_eq!(
        fs::read(original).expect("original"),
        fs::read(fixture.png("again.png", 2)).expect("source")
    );
    let annotations = fixture
        .library
        .annotations(id)
        .expect("query")
        .expect("some");
    assert_eq!(
        annotations.document,
        plain_document().to_json().expect("json")
    );
    assert!(!annotations.has_masks);
    assert_eq!(fixture.files_on_disk().len(), 2);
}

#[test]
fn an_ordinary_resource_has_no_annotations_and_no_original() {
    let mut fixture = Fixture::new();
    let outcome = fixture
        .library
        .import_file(&sample("red-dot.png"), None)
        .expect("imported");
    let ImportOutcome::Imported(id) = outcome else {
        panic!("the resource is new");
    };

    assert_eq!(fixture.library.annotations(id).expect("query"), None);
    assert_eq!(fixture.library.original_path(id).expect("query"), None);
}

#[test]
fn a_rendering_already_in_the_library_is_refused_and_names_the_existing_resource() {
    let mut fixture = Fixture::new();
    let first = fixture.import("a.png", 1, 2);
    let before = fixture.files_on_disk();
    let rendered = fixture.png("b.png", 1);
    let original = fixture.png("c.png", 3);

    let result = fixture.library.import_annotated(AnnotatedImage {
        rendered: &rendered,
        original: &original,
        document: &plain_document(),
    });

    assert!(matches!(result, Err(AnnotatedError::AlreadyPresent(existing)) if existing == first));
    assert_eq!(fixture.asset_count(), 1);
    assert_eq!(fixture.files_on_disk(), before);
}

#[test]
fn a_rendering_equal_to_an_ordinary_import_is_refused_too() {
    let mut fixture = Fixture::new();
    fixture
        .library
        .import_file(&sample("red-dot.png"), None)
        .expect("imported");
    let rendered = sample("red-dot.png");
    let original = fixture.png("o.png", 5);

    let result = fixture.library.import_annotated(AnnotatedImage {
        rendered: &rendered,
        original: &original,
        document: &plain_document(),
    });

    assert!(matches!(result, Err(AnnotatedError::AlreadyPresent(_))));
    assert_eq!(fixture.asset_count(), 1);
}

#[test]
fn anything_but_a_png_is_refused_without_a_trace() {
    let mut fixture = Fixture::new();
    let png = fixture.png("ok.png", 1);
    let photo = sample("blue-photo.jpg");
    let fake = fixture.sources.path().join("fake.png");
    fs::write(&fake, b"not an image").expect("written");
    let absent = fixture.sources.path().join("absent.png");

    for (rendered, original) in [
        (&photo, &png),
        (&png, &photo),
        (&fake, &png),
        (&png, &fake),
        (&png, &absent),
        (&absent, &png),
    ] {
        let result = fixture.library.import_annotated(AnnotatedImage {
            rendered,
            original,
            document: &plain_document(),
        });
        assert!(result.is_err(), "{rendered:?} / {original:?}: {result:?}");
    }

    assert_eq!(fixture.asset_count(), 0);
    assert!(fixture.files_on_disk().is_empty());
}

#[test]
fn a_rendering_cannot_take_the_name_reserved_for_the_original() {
    let mut fixture = Fixture::new();
    let rendered = fixture.png("original.png", 1);
    let original = fixture.png("o.png", 2);

    let result = fixture.library.import_annotated(AnnotatedImage {
        rendered: &rendered,
        original: &original,
        document: &plain_document(),
    });

    assert!(matches!(result, Err(AnnotatedError::ReservedName(_))));
    assert_eq!(fixture.asset_count(), 0);
}

#[test]
fn updating_replaces_the_rendering_and_the_document_but_never_the_original() {
    let mut fixture = Fixture::new();
    let id = fixture.import("cap.png", 1, 2);
    fixture
        .library
        .record_colors(
            id,
            &[DominantColor {
                family: AssetColor::Red,
                average: Rgb::new(1, 2, 3),
            }],
        )
        .expect("colors recorded");
    let original_before =
        fs::read(fixture.library.original_path(id).expect("q").expect("some")).expect("original");
    let new_rendering = fixture.png("new.png", 9);

    fixture
        .library
        .update_annotated(id, &new_rendering, &document_with_a_blur())
        .expect("updated");

    assert_eq!(
        fs::read(fixture.stored_file(id)).expect("rendering"),
        fs::read(&new_rendering).expect("source")
    );
    let asset = fixture.library.asset(id).expect("query").expect("exists");
    assert_eq!(
        asset.byte_size,
        fs::metadata(&new_rendering).expect("meta").len()
    );
    assert!(asset.colors.is_empty());
    assert_eq!(
        fixture.library.assets_awaiting_colors(10).expect("listed"),
        [id]
    );
    let annotations = fixture.library.annotations(id).expect("q").expect("some");
    assert_eq!(
        annotations.document,
        document_with_a_blur().to_json().expect("json")
    );
    assert!(annotations.has_masks);
    assert_eq!(
        fs::read(fixture.library.original_path(id).expect("q").expect("some")).expect("original"),
        original_before
    );
    assert_eq!(fixture.files_on_disk().len(), 2);
    assert_eq!(fixture.problems(), 0);
}

#[test]
fn updating_with_the_same_rendering_only_changes_the_document() {
    let mut fixture = Fixture::new();
    let id = fixture.import("cap.png", 1, 2);
    let same = fixture.png("same.png", 1);

    fixture
        .library
        .update_annotated(id, &same, &document_with_a_blur())
        .expect("updated");

    let annotations = fixture.library.annotations(id).expect("q").expect("some");
    assert_eq!(
        annotations.document,
        document_with_a_blur().to_json().expect("json")
    );
    assert!(annotations.has_masks);
    assert_eq!(fixture.problems(), 0);
}

#[test]
fn updating_into_another_resources_rendering_is_refused_and_changes_nothing() {
    let mut fixture = Fixture::new();
    let first = fixture.import("a.png", 1, 2);
    let second = fixture.import("b.png", 3, 4);
    let collision = fixture.png("c.png", 1);
    let stored_before = fs::read(fixture.stored_file(second)).expect("rendering");

    let result = fixture
        .library
        .update_annotated(second, &collision, &document_with_a_blur());

    assert!(matches!(result, Err(AnnotatedError::AlreadyPresent(other)) if other == first));
    assert_eq!(
        fs::read(fixture.stored_file(second)).expect("rendering"),
        stored_before
    );
    let annotations = fixture
        .library
        .annotations(second)
        .expect("q")
        .expect("some");
    assert_eq!(
        annotations.document,
        plain_document().to_json().expect("json")
    );
    assert_eq!(fixture.files_on_disk().len(), 4);
    assert_eq!(fixture.problems(), 0);
}

#[test]
fn only_an_annotated_resource_can_be_updated() {
    let mut fixture = Fixture::new();
    let ImportOutcome::Imported(plain) = fixture
        .library
        .import_file(&sample("red-dot.png"), None)
        .expect("imported")
    else {
        panic!("new");
    };
    let rendering = fixture.png("r.png", 4);

    assert!(matches!(
        fixture.library.update_annotated(plain, &rendering, &plain_document()),
        Err(AnnotatedError::NotAnnotated(id)) if id == plain
    ));
    let elsewhere = tempfile::tempdir().expect("temporary directory");
    let mut other = Library::create(elsewhere.path(), "Autre").expect("library");
    let ImportOutcome::Imported(unknown) = other
        .import_file(&sample("red-dot.png"), None)
        .expect("imported")
    else {
        panic!("new");
    };
    assert!(matches!(
        fixture.library.update_annotated(unknown, &rendering, &plain_document()),
        Err(AnnotatedError::AssetNotFound(id)) if id == unknown
    ));
}

#[test]
fn trashing_and_restoring_keeps_the_annotations_and_deleting_for_good_removes_everything() {
    let mut fixture = Fixture::new();
    let id = fixture.import("cap.png", 1, 2);

    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![id],
            trashed: true,
        })
        .expect("trashed");
    assert!(fixture.library.annotations(id).expect("q").is_some());
    fixture
        .library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![id],
            trashed: false,
        })
        .expect("restored");
    assert!(fixture.library.annotations(id).expect("q").is_some());
    assert_eq!(fixture.files_on_disk().len(), 2);

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
fn removing_the_record_of_a_missing_rendering_removes_the_original_too() {
    let mut fixture = Fixture::new();
    let id = fixture.import("cap.png", 1, 2);
    fs::remove_file(fixture.stored_file(id)).expect("rendering lost");

    fixture
        .library
        .remove_record_of_missing_file(id)
        .expect("record removed");

    assert!(fixture.files_on_disk().is_empty());
    assert_eq!(fixture.library.annotations(id).expect("q"), None);
}

#[test]
fn the_original_is_a_recorded_file_and_counts_in_the_storage_use() {
    let mut fixture = Fixture::new();
    let id = fixture.import("cap.png", 1, 2);

    let rendering = fs::metadata(fixture.stored_file(id)).expect("meta").len();
    let original = fs::metadata(fixture.library.original_path(id).expect("q").expect("some"))
        .expect("meta")
        .len();

    assert_eq!(fixture.problems(), 0);
    let storage = fixture.library.storage_use().expect("storage");
    assert_eq!(storage.resources, rendering + original);
}

#[test]
fn a_library_reopened_still_has_its_annotations() {
    let mut fixture = Fixture::new();
    let id = fixture.import("cap.png", 1, 2);
    let root = fixture.library.root().to_path_buf();
    drop(fixture.library);

    let library = Library::open(&root).expect("opens");

    assert_eq!(
        library.annotation_document(id).expect("q").expect("some"),
        plain_document()
    );
    drop(fixture.workspace);
}

#[test]
fn an_unknown_tool_survives_a_trip_through_the_library() {
    let mut fixture = Fixture::new();
    let rendered = fixture.png("cap.png", 1);
    let original = fixture.png("o.png", 2);
    let document = AnnotationDocument::from_json(
        r#"{"version":1,"objects":[{"id":1,"kind":"lasso","points":[[1,2]]}],"frame":{"margin":3}}"#,
    )
    .expect("read");

    let id = fixture
        .library
        .import_annotated(AnnotatedImage {
            rendered: &rendered,
            original: &original,
            document: &document,
        })
        .expect("imported");

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
    let id = fixture.import("cap.png", 1, 2);
    fixture
        .database()
        .execute(
            "UPDATE asset_annotations SET document = '{\"version\":9,\"objects\":[]}'",
            [],
        )
        .expect("document replaced");

    let result = fixture.library.annotation_document(id);

    assert!(matches!(
        result,
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
    let plain = fixture.import("a.png", 1, 2);
    let rendered = fixture.png("b.png", 3);
    let original = fixture.png("o.png", 4);
    let blurred = fixture
        .library
        .import_annotated(AnnotatedImage {
            rendered: &rendered,
            original: &original,
            document: &document_with_a_blur(),
        })
        .expect("imported");

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
fn a_document_that_cannot_be_written_is_refused_without_a_trace() {
    let mut fixture = Fixture::new();
    let rendered = fixture.png("cap.png", 1);
    let original = fixture.png("o.png", 2);
    let broken =
        AnnotationDocument::new(None, vec![AnnotationObject::new(1, blur_shape(f32::NAN))]);

    let result = fixture.library.import_annotated(AnnotatedImage {
        rendered: &rendered,
        original: &original,
        document: &broken,
    });

    assert!(matches!(result, Err(AnnotatedError::Document(_))));
    assert_eq!(fixture.asset_count(), 0);
    assert!(fixture.files_on_disk().is_empty());
}
