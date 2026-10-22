use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use pigoune_core::annotations::{
    AnnotationDocument, AnnotationObject, BLUR, Geometry, Options, Rect, Shape,
};
use pigoune_core::{
    AssetCommand, AssetId, CacheFile, FlatFiles, ImportOutcome, Library, cache_file,
};
use rusqlite::Connection;
use tempfile::TempDir;

type Copy = (String, Vec<u8>);

#[derive(Debug, PartialEq, Eq)]
struct Check {
    flats: Vec<(AssetId, PathBuf)>,
    not_ready: usize,
    hidden_mask_names: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    HiddenMasks(Vec<String>),
    NotReady,
}

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    check: Check,
    refusal: Option<Refusal>,
    exposed: Option<Result<Vec<Copy>, String>>,
}

fn reference_check(library: &Library, ids: &[AssetId]) -> Check {
    let mut flats = FlatFiles::new();
    let mut not_ready = 0;
    for id in ids {
        match library.annotation_render(*id) {
            Ok(Some(render)) if cache_file(&render.file) == CacheFile::Plain => {
                flats.insert(*id, render.file);
            }
            Ok(Some(_)) | Err(_) => not_ready += 1,
            Ok(None) => {}
        }
    }
    let hidden_mask_names = ids
        .iter()
        .filter_map(|id| {
            let document = library.annotation_document(*id).ok().flatten()?;
            let hidden = document.exposed_masks();
            let asset = library.asset(*id).ok().flatten()?;
            (!hidden.is_empty()).then_some(asset.display_name)
        })
        .collect();
    let mut flats: Vec<_> = flats.into_iter().collect();
    flats.sort();
    Check {
        flats,
        not_ready,
        hidden_mask_names,
    }
}

fn observe(library: &Library, ids: &[AssetId], check: Check) -> Observation {
    let refusal = if check.hidden_mask_names.is_empty() {
        (check.not_ready > 0).then_some(Refusal::NotReady)
    } else {
        Some(Refusal::HiddenMasks(check.hidden_mask_names.clone()))
    };
    let exposed = refusal.is_none().then(|| {
        let flats: FlatFiles = check.flats.iter().cloned().collect();
        library
            .export_copies_checking(ids, &flats, &|_| None)
            .map(|paths| {
                paths
                    .iter()
                    .map(|path| {
                        (
                            path.file_name()
                                .expect("name")
                                .to_string_lossy()
                                .into_owned(),
                            fs::read(path).expect("copy"),
                        )
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    });
    Observation {
        check,
        refusal,
        exposed,
    }
}

fn ghost(number: usize) -> AssetId {
    AssetId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("valid id")
}

fn blur(hidden: bool, x: f32) -> AnnotationDocument {
    let shape = Shape::new(
        "blur",
        Geometry::Box(Rect {
            x,
            y: 2.0,
            width: 30.0,
            height: 40.0,
        }),
        Options::defaults(BLUR.options),
    );
    let mut document = AnnotationDocument::new(None, vec![AnnotationObject::new(1, shape)]);
    document.objects[0].state.hidden = hidden;
    document
}

struct Lab {
    workspace: TempDir,
    library: Library,
    originals: Vec<(AssetId, Vec<u8>)>,
}

impl Lab {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace");
        let library = Library::create(workspace.path(), "Essai").expect("library");
        Self {
            workspace,
            library,
            originals: Vec::new(),
        }
    }

    fn photo(&mut self, number: usize) -> AssetId {
        let mut bytes =
            fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/red-dot.png"))
                .expect("sample");
        bytes.extend_from_slice(format!("#{number}").as_bytes());
        let sources = self.workspace.path().join("sources");
        fs::create_dir_all(&sources).expect("sources");
        let path = sources.join(format!("photo-{number}.png"));
        fs::write(&path, &bytes).expect("source");
        let ImportOutcome::Imported(id) = self.library.import_file(&path, None).expect("import")
        else {
            panic!("not imported");
        };
        self.originals.push((id, bytes));
        id
    }

    fn original(&self, id: AssetId) -> Vec<u8> {
        self.originals
            .iter()
            .find(|(known, _)| *known == id)
            .expect("known")
            .1
            .clone()
    }

    fn annotate(&mut self, id: AssetId, document: &AnnotationDocument) {
        self.library
            .save_annotations(id, document)
            .expect("annotated");
    }

    fn render_path(&self, id: AssetId) -> PathBuf {
        self.library
            .annotation_render(id)
            .expect("render")
            .expect("annotated")
            .file
    }

    fn write_render(&self, id: AssetId, bytes: &[u8]) -> PathBuf {
        let path = self.render_path(id);
        fs::create_dir_all(path.parent().expect("folder")).expect("folder");
        let _ = fs::remove_file(&path);
        fs::write(&path, bytes).expect("render");
        path
    }

    fn stored(&self, id: AssetId) -> PathBuf {
        let asset = self.library.asset(id).expect("q").expect("asset");
        self.library
            .original_of(&asset)
            .expect("original")
            .path()
            .to_path_buf()
    }

    fn observe(&self, ids: &[AssetId]) -> Observation {
        let reference = reference_check(&self.library, ids);
        let observation = observe(&self.library, ids, reference);
        self.compare_with_the_optimized_check(ids, &observation);
        observation
    }

    fn compare_with_the_optimized_check(&self, ids: &[AssetId], reference: &Observation) {
        let optimized = self.library.check_drag_out(ids);
        let mut flats: Vec<_> = optimized.flats.into_iter().collect();
        flats.sort();
        let optimized = observe(
            &self.library,
            ids,
            Check {
                flats,
                not_ready: optimized.not_ready,
                hidden_mask_names: optimized.hidden_mask_names,
            },
        );
        assert_eq!(&optimized, reference);
    }

    fn nothing_exposes_an_original_among_the_exports(&self, ids: &[AssetId]) {
        let export = self.library.root().join("cache").join("export");
        let mut seen = Vec::new();
        collect(&export, &mut seen);
        for id in ids {
            let original = self.original(*id);
            assert!(!seen.contains(&original), "an original was exposed");
        }
    }
}

fn collect(folder: &Path, found: &mut Vec<Vec<u8>>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = entry.file_type().expect("kind");
        if kind.is_dir() {
            collect(&path, found);
        } else if kind.is_file() {
            found.push(fs::read(&path).expect("file"));
        }
    }
}

fn names_and_contents(observation: &Observation) -> Vec<Copy> {
    observation
        .exposed
        .as_ref()
        .expect("not refused")
        .as_ref()
        .expect("exported")
        .clone()
}

#[test]
fn an_ordinary_unannotated_item_is_exposed_as_its_original() {
    let mut lab = Lab::new();
    let photo = lab.photo(1);

    let observation = lab.observe(&[photo]);

    assert_eq!(observation.check.not_ready, 0);
    assert!(observation.check.flats.is_empty());
    assert!(observation.check.hidden_mask_names.is_empty());
    assert_eq!(observation.refusal, None);
    assert_eq!(
        names_and_contents(&observation),
        vec![("photo-1.png".to_owned(), lab.original(photo))]
    );
}

#[test]
fn an_annotated_item_with_a_flat_render_is_exposed_as_the_render_never_the_original() {
    let mut lab = Lab::new();
    let photo = lab.photo(1);
    lab.annotate(photo, &blur(false, 1.0));
    let render = lab.write_render(photo, b"THE FLAT RENDER");

    let observation = lab.observe(&[photo]);

    assert_eq!(observation.check.flats, vec![(photo, render)]);
    assert_eq!(observation.refusal, None);
    assert_eq!(
        names_and_contents(&observation),
        vec![("photo-1.png".to_owned(), b"THE FLAT RENDER".to_vec())]
    );
    lab.nothing_exposes_an_original_among_the_exports(&[photo]);
}

#[test]
fn a_missing_stale_or_unsafe_render_refuses_and_never_falls_back_to_the_original() {
    let mut lab = Lab::new();
    let photo = lab.photo(1);
    lab.annotate(photo, &blur(false, 1.0));

    let missing = lab.observe(&[photo]);
    assert_eq!(missing.check.not_ready, 1);
    assert!(missing.check.flats.is_empty());
    assert_eq!(missing.refusal, Some(Refusal::NotReady));
    assert_eq!(missing.exposed, None);

    lab.write_render(photo, b"OLD RENDER");
    lab.annotate(photo, &blur(false, 9.0));
    let stale = lab.observe(&[photo]);
    assert_eq!(stale.check.not_ready, 1);
    assert_eq!(stale.refusal, Some(Refusal::NotReady));

    let path = lab.render_path(photo);
    fs::create_dir_all(path.parent().expect("folder")).expect("folder");
    fs::create_dir(&path).expect("folder as render");
    let invalid = lab.observe(&[photo]);
    assert_eq!(invalid.check.not_ready, 1);
    assert_eq!(invalid.refusal, Some(Refusal::NotReady));
    fs::remove_dir(&path).expect("removed");

    let outside = lab.workspace.path().join("outside.png");
    fs::write(&outside, b"OUTSIDE").expect("outside");
    symlink(&outside, &path).expect("link");
    let linked = lab.observe(&[photo]);
    assert_eq!(linked.check.not_ready, 1);
    assert!(linked.check.flats.is_empty());
    assert_eq!(linked.refusal, Some(Refusal::NotReady));

    let forced = lab
        .library
        .export_copies_checking(&[photo], &FlatFiles::new(), &|_| None);
    assert!(forced.is_err());
    lab.nothing_exposes_an_original_among_the_exports(&[photo]);
}

#[test]
fn a_sensitive_hidden_mask_is_named_and_refuses_the_whole_selection() {
    let mut lab = Lab::new();
    let plain = lab.photo(1);
    let masked = lab.photo(2);
    lab.annotate(masked, &blur(true, 1.0));
    lab.write_render(masked, b"RENDER");

    let observation = lab.observe(&[plain, masked]);

    assert_eq!(observation.check.hidden_mask_names, vec!["photo-2"]);
    assert_eq!(
        observation.refusal,
        Some(Refusal::HiddenMasks(vec!["photo-2".to_owned()]))
    );
    assert_eq!(observation.exposed, None);
}

#[test]
fn a_visible_mask_does_not_warn() {
    let mut lab = Lab::new();
    let photo = lab.photo(1);
    lab.annotate(photo, &blur(false, 1.0));
    lab.write_render(photo, b"RENDER");

    let observation = lab.observe(&[photo]);

    assert!(observation.check.hidden_mask_names.is_empty());
    assert_eq!(observation.refusal, None);
}

#[test]
fn a_selection_mixing_annotated_and_plain_items_keeps_its_order_and_its_count() {
    let mut lab = Lab::new();
    let ids: Vec<AssetId> = (1..=7).map(|number| lab.photo(number)).collect();
    for &annotated in &[ids[1], ids[4], ids[5]] {
        lab.annotate(annotated, &blur(false, 1.0));
        lab.write_render(annotated, format!("RENDER {annotated}").as_bytes());
    }
    let selection = [ids[5], ids[0], ids[4], ids[2], ids[1], ids[6], ids[3]];

    let observation = lab.observe(&selection);

    assert_eq!(observation.refusal, None);
    assert_eq!(observation.check.flats.len(), 3);
    let exposed = names_and_contents(&observation);
    assert_eq!(exposed.len(), 7);
    let expected: Vec<Vec<u8>> = selection
        .iter()
        .map(|id| {
            if [ids[1], ids[4], ids[5]].contains(id) {
                format!("RENDER {id}").into_bytes()
            } else {
                lab.original(*id)
            }
        })
        .collect();
    assert_eq!(
        exposed
            .iter()
            .map(|(_, bytes)| bytes.clone())
            .collect::<Vec<_>>(),
        expected
    );
    let names: Vec<&str> = exposed.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "photo-6.png",
            "photo-1.png",
            "photo-5.png",
            "photo-3.png",
            "photo-2.png",
            "photo-7.png",
            "photo-4.png"
        ]
    );
}

#[test]
fn a_deleted_record_is_skipped_and_a_trashed_item_is_still_exposed() {
    let mut lab = Lab::new();
    let kept = lab.photo(1);
    let trashed = lab.photo(2);
    lab.library
        .apply_asset_command(&AssetCommand::SetTrashed {
            assets: vec![trashed],
            trashed: true,
        })
        .expect("trashed");
    let ghost = ghost(0);

    let observation = lab.observe(&[kept, ghost, trashed]);

    assert_eq!(observation.refusal, None);
    assert_eq!(
        names_and_contents(&observation),
        vec![
            ("photo-1.png".to_owned(), lab.original(kept)),
            ("photo-2.png".to_owned(), lab.original(trashed)),
        ]
    );
}

#[test]
fn a_missing_original_file_or_an_invalid_stored_path_fails_the_whole_operation() {
    let mut lab = Lab::new();
    let fine = lab.photo(1);
    let lost = lab.photo(2);
    fs::remove_file(lab.stored(lost)).expect("removed");

    let missing = lab.observe(&[fine, lost]);
    assert_eq!(missing.refusal, None);
    assert!(missing.exposed.expect("ran").is_err());

    let forged = lab.photo(3);
    let database = Connection::open(lab.library.root().join("library.db")).expect("database");
    database
        .execute(
            "UPDATE assets SET stored_path = '../../escape.png' WHERE id = ?1",
            [forged.to_string()],
        )
        .expect("forged");
    drop(database);
    let invalid = lab.observe(&[fine, forged]);
    assert!(invalid.exposed.expect("ran").is_err());
}

#[test]
fn a_repeated_id_is_counted_and_exposed_as_many_times_as_it_is_given() {
    let mut lab = Lab::new();
    let photo = lab.photo(1);
    lab.annotate(photo, &blur(false, 1.0));

    let twice = lab.observe(&[photo, photo]);

    assert_eq!(twice.check.not_ready, 2);
    assert_eq!(twice.refusal, Some(Refusal::NotReady));
}

#[test]
fn a_selection_larger_than_any_query_limit_gives_the_same_decisions() {
    let mut lab = Lab::new();
    let real: Vec<AssetId> = (1..=6).map(|number| lab.photo(number)).collect();
    lab.annotate(real[2], &blur(false, 1.0));
    lab.write_render(real[2], b"RENDER");
    lab.annotate(real[4], &blur(true, 1.0));
    lab.write_render(real[4], b"MASKED");
    let mut selection = Vec::new();
    for (index, id) in real.iter().enumerate() {
        selection.push(*id);
        selection.extend((0..8_000).map(|offset| ghost(index * 8_000 + offset)));
        if index == 2 {
            selection.extend(std::iter::repeat_n(real[2], 3));
        }
    }

    let observation = lab.observe(&selection);

    assert_eq!(observation.check.hidden_mask_names, vec!["photo-5"]);
    assert_eq!(
        observation.refusal,
        Some(Refusal::HiddenMasks(vec!["photo-5".to_owned()]))
    );
    assert_eq!(observation.check.not_ready, 0);
    assert_eq!(observation.check.flats.len(), 2);
    let distinct: HashSet<_> = selection.iter().collect();
    assert!(distinct.len() > 40_000);
}

#[test]
fn a_selection_of_many_real_items_across_the_query_chunks_keeps_order_and_count() {
    let mut lab = Lab::new();
    let ids: Vec<AssetId> = (1..=1_100).map(|number| lab.photo(number)).collect();
    for index in (0..ids.len()).step_by(7) {
        lab.annotate(ids[index], &blur(false, 1.0));
        lab.write_render(ids[index], format!("RENDER {index}").as_bytes());
    }
    let selection: Vec<AssetId> = ids.iter().rev().copied().collect();

    let observation = lab.observe(&selection);

    assert_eq!(observation.refusal, None);
    assert_eq!(observation.check.flats.len(), 158);
    let exposed = names_and_contents(&observation);
    assert_eq!(exposed.len(), 1_100);
    for (position, (name, bytes)) in exposed.iter().enumerate() {
        let index = 1_099 - position;
        assert_eq!(name, &format!("photo-{}.png", index + 1));
        if index % 7 == 0 {
            assert_eq!(bytes, format!("RENDER {index}").as_bytes());
        } else {
            assert_eq!(bytes, &lab.original(ids[index]));
        }
    }
}

#[test]
fn a_corrupt_annotation_document_gives_no_warning_but_still_needs_its_render() {
    let mut lab = Lab::new();
    let photo = lab.photo(1);
    lab.annotate(photo, &blur(true, 1.0));
    lab.write_render(photo, b"RENDER");
    let database = Connection::open(lab.library.root().join("library.db")).expect("database");
    database
        .execute(
            "UPDATE asset_annotations SET document = '{' WHERE asset_id = ?1",
            [photo.to_string()],
        )
        .expect("corrupted");
    drop(database);

    let observation = lab.observe(&[photo]);

    assert!(observation.check.hidden_mask_names.is_empty());
    assert_eq!(observation.check.not_ready, 1);
    assert_eq!(observation.refusal, Some(Refusal::NotReady));
}
