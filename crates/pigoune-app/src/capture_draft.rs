use std::fs;
use std::io::{self, Write};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use gtk::glib;
use pigoune_core::annotations::{AnnotationDocument, hash_of_bytes};

const FOLDER_NAME: &str = "capture-draft";
const ORIGINAL_NAME: &str = "original.png";
const DOCUMENT_NAME: &str = "document.json";
const UNFINISHED_NAME: &str = "document.json.unfinished";
const UNFINISHED_ORIGINAL_NAME: &str = "original.png.unfinished";

fn sealed_name(generation: u32) -> String {
    format!("original.{generation}.png")
}

fn is_sealed_name(name: &str) -> bool {
    name.strip_prefix("original.")
        .and_then(|rest| rest.strip_suffix(".png"))
        .is_some_and(|digits| {
            !digits.is_empty()
                && !digits.starts_with('0')
                && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

#[derive(Debug, Clone)]
pub struct Draft {
    folder: PathBuf,
}

impl Draft {
    pub fn standard() -> Self {
        Self::at(glib::user_cache_dir().join("pigoune").join(FOLDER_NAME))
    }

    pub fn at(folder: PathBuf) -> Self {
        Self { folder }
    }

    pub fn clear(&self) {
        let _ = fs::remove_dir_all(&self.folder);
    }

    pub fn store_original(&self, png: &[u8]) -> io::Result<()> {
        self.write_atomically(UNFINISHED_ORIGINAL_NAME, ORIGINAL_NAME, png)
    }

    pub fn store_document(&self, document: &AnnotationDocument) -> io::Result<()> {
        let json = document.to_json().map_err(io::Error::other)?;
        self.write_atomically(UNFINISHED_NAME, DOCUMENT_NAME, json.as_bytes())
    }

    pub fn store_sealed(&self, png: &[u8], document: &AnnotationDocument) -> io::Result<()> {
        let record = document
            .seal
            .as_ref()
            .filter(|record| record.is_well_formed() && hash_of_bytes(png) == record.base)
            .ok_or_else(|| io::Error::other("the sealed image is not the one the record names"))?;
        let name = sealed_name(record.generation);
        self.write_atomically(UNFINISHED_ORIGINAL_NAME, &name, png)?;
        self.store_document(document)?;
        self.tidy(&name);
        Ok(())
    }

    fn write_atomically(&self, unfinished: &str, name: &str, content: &[u8]) -> io::Result<()> {
        fs::create_dir_all(&self.folder)?;
        let partial = self.folder.join(unfinished);
        let mut file = fs::File::create(&partial)?;
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&partial, self.folder.join(name))?;
        if let Ok(folder) = fs::File::open(&self.folder) {
            let _ = folder.sync_all();
        }
        Ok(())
    }

    fn tidy(&self, keep: &str) {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let is_ours = name == ORIGINAL_NAME || is_sealed_name(name);
            let regular = entry.file_type().is_ok_and(|kind| kind.is_file());
            if is_ours && regular && name != keep {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    pub fn load(&self) -> Option<(PathBuf, AnnotationDocument)> {
        let json = fs::read_to_string(self.folder.join(DOCUMENT_NAME)).ok()?;
        let document = AnnotationDocument::from_json(&json).ok()?;
        let Some(record) = &document.seal else {
            let original = self.folder.join(ORIGINAL_NAME);
            return original.is_file().then_some((original, document));
        };
        let name = sealed_name(record.generation);
        let original = self.folder.join(&name);
        let bytes = fs::read(&original).ok()?;
        if hash_of_bytes(&bytes) != record.base {
            return None;
        }
        self.tidy(&name);
        Some((original, document))
    }

    #[cfg(test)]
    pub fn folder(&self) -> &Path {
        &self.folder
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use pigoune_core::annotations::{
        AnnotationDocument, AnnotationObject, Geometry, Options, Rect, SealRecord, Shape,
        hash_of_bytes,
    };

    use super::Draft;

    fn fresh() -> Draft {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let folder = std::env::temp_dir().join(format!(
            "pigoune-draft-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let draft = Draft::at(folder);
        draft.clear();
        draft
    }

    fn document() -> AnnotationDocument {
        let shape = Shape::new(
            "blur",
            Geometry::Box(Rect {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 40.0,
            }),
            Options::new(),
        );
        AnnotationDocument::new(None, vec![AnnotationObject::new(1, shape)])
    }

    #[test]
    fn nothing_is_loaded_before_anything_is_stored() {
        assert!(fresh().load().is_none());
    }

    #[test]
    fn a_stored_draft_comes_back_with_its_document_and_its_image() {
        let draft = fresh();
        draft.store_original(b"png bytes").unwrap();
        draft.store_document(&document()).unwrap();

        let (original, loaded) = draft.load().expect("a draft");

        assert_eq!(std::fs::read(original).unwrap(), b"png bytes");
        assert_eq!(loaded, document());
        draft.clear();
    }

    #[test]
    fn a_draft_without_its_image_or_its_document_is_not_offered() {
        let draft = fresh();
        draft.store_document(&document()).unwrap();
        assert!(draft.load().is_none());
        draft.store_original(b"png").unwrap();
        std::fs::write(draft.folder().join("document.json"), "not json").unwrap();
        assert!(draft.load().is_none());
        draft.clear();
    }

    fn sealed_document(png: &[u8]) -> AnnotationDocument {
        let mut document = document();
        document.objects[0].state.sealed = true;
        document.seal = Some(SealRecord {
            generation: 1,
            base: hash_of_bytes(png),
        });
        document
    }

    #[test]
    fn a_sealed_draft_comes_back_verified_and_the_net_image_is_gone() {
        let draft = fresh();
        draft.store_original(b"net pixels").unwrap();
        draft.store_document(&document()).unwrap();
        let sealed = b"blurred pixels";
        draft
            .store_sealed(sealed, &sealed_document(sealed))
            .unwrap();

        let (original, loaded) = draft.load().expect("a draft");

        assert_eq!(std::fs::read(original).unwrap(), sealed);
        assert_eq!(loaded, sealed_document(sealed));
        assert!(!draft.folder().join("original.png").exists());
        draft.clear();
    }

    #[test]
    fn a_sealing_that_stops_before_the_document_is_written_leaves_the_previous_draft_whole() {
        let draft = fresh();
        draft.store_original(b"net pixels").unwrap();
        draft.store_document(&document()).unwrap();
        std::fs::write(draft.folder().join("original.1.png"), b"blurred pixels").unwrap();

        let (original, loaded) = draft.load().expect("the previous draft");

        assert_eq!(std::fs::read(original).unwrap(), b"net pixels");
        assert_eq!(loaded, document());
        draft.clear();
    }

    #[test]
    fn a_sealing_that_stops_after_the_document_is_written_finishes_on_the_next_load() {
        let draft = fresh();
        draft.store_original(b"net pixels").unwrap();
        let sealed = b"blurred pixels";
        std::fs::write(draft.folder().join("original.1.png"), sealed).unwrap();
        draft.store_document(&sealed_document(sealed)).unwrap();

        let (original, _) = draft.load().expect("the sealed draft");

        assert_eq!(std::fs::read(original).unwrap(), sealed);
        assert!(!draft.folder().join("original.png").exists());
        draft.clear();
    }

    #[test]
    fn a_sealed_draft_whose_image_changed_or_vanished_is_not_offered() {
        let draft = fresh();
        let sealed = b"blurred pixels";
        draft
            .store_sealed(sealed, &sealed_document(sealed))
            .unwrap();
        std::fs::write(draft.folder().join("original.1.png"), b"other").unwrap();
        assert!(draft.load().is_none());
        std::fs::remove_file(draft.folder().join("original.1.png")).unwrap();
        assert!(draft.load().is_none());
        draft.clear();
    }

    #[test]
    fn a_sealed_draft_never_falls_back_to_a_net_image() {
        let draft = fresh();
        draft.store_original(b"net pixels").unwrap();
        let sealed = b"blurred pixels";
        draft.store_document(&sealed_document(sealed)).unwrap();
        assert!(draft.load().is_none(), "the sealed image is missing");
        draft.clear();
    }

    #[test]
    fn sealing_with_another_image_than_the_recorded_one_is_refused() {
        let draft = fresh();
        draft.store_original(b"net pixels").unwrap();
        draft.store_document(&document()).unwrap();
        assert!(
            draft
                .store_sealed(b"something else", &sealed_document(b"blurred pixels"))
                .is_err()
        );
        assert!(!draft.folder().join("original.1.png").exists());
        assert_eq!(draft.load().unwrap().1, document());
        draft.clear();
    }

    #[test]
    fn clearing_removes_the_whole_draft() {
        let draft = fresh();
        draft.store_original(b"png").unwrap();
        draft.store_document(&document()).unwrap();
        draft.clear();
        assert!(draft.load().is_none());
        assert!(!draft.folder().exists());
    }

    #[test]
    fn a_newer_document_replaces_the_older_one() {
        let draft = fresh();
        draft.store_original(b"png").unwrap();
        draft
            .store_document(&AnnotationDocument::default())
            .unwrap();
        draft.store_document(&document()).unwrap();
        assert_eq!(draft.load().unwrap().1, document());
        draft.clear();
    }
}
