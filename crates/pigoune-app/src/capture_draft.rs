use std::fs;
use std::io;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use gtk::glib;
use pigoune_core::annotations::AnnotationDocument;

const FOLDER_NAME: &str = "capture-draft";
const ORIGINAL_NAME: &str = "original.png";
const DOCUMENT_NAME: &str = "document.json";
const UNFINISHED_NAME: &str = "document.json.unfinished";

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
        fs::create_dir_all(&self.folder)?;
        fs::write(self.folder.join(ORIGINAL_NAME), png)
    }

    pub fn store_document(&self, document: &AnnotationDocument) -> io::Result<()> {
        let json = document.to_json().map_err(io::Error::other)?;
        fs::create_dir_all(&self.folder)?;
        let unfinished = self.folder.join(UNFINISHED_NAME);
        fs::write(&unfinished, json)?;
        fs::rename(unfinished, self.folder.join(DOCUMENT_NAME))
    }

    pub fn load(&self) -> Option<(PathBuf, AnnotationDocument)> {
        let original = self.folder.join(ORIGINAL_NAME);
        if !original.is_file() {
            return None;
        }
        let json = fs::read_to_string(self.folder.join(DOCUMENT_NAME)).ok()?;
        let document = AnnotationDocument::from_json(&json).ok()?;
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
        AnnotationDocument, AnnotationObject, Geometry, Options, Rect, Shape,
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
