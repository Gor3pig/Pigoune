use std::path::PathBuf;

use gettextrs::gettext;
use gtk::gdk;
use gtk::glib;
use pigoune_core::Library;

use crate::clipboard_content;

pub trait ContentSink {
    fn offer(&self, copies: &[PathBuf]) -> Result<(), glib::BoolError>;
}

impl ContentSink for gdk::Clipboard {
    fn offer(&self, copies: &[PathBuf]) -> Result<(), glib::BoolError> {
        self.set_content(Some(&clipboard_content::provider(copies)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    NotRegistered,
    Refused,
}

pub fn install(
    library: &Library,
    sink: &impl ContentSink,
    copies: &[PathBuf],
) -> Result<(), Failure> {
    let pending = library
        .begin_clipboard_install(copies)
        .map_err(|_| Failure::NotRegistered)?;
    if sink.offer(copies).is_err() {
        pending.abandon();
        return Err(Failure::Refused);
    }
    pending.commit();
    Ok(())
}

pub fn storage_text() -> String {
    gettext("Not enough free space to copy these files. The clipboard was left as it was")
}

pub fn failure_text() -> String {
    gettext("The files could not be copied to the clipboard. The clipboard was left as it was")
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::Path;

    use pigoune_core::{FlatFiles, ImportOutcome};

    use super::*;

    struct Sink {
        offered: Cell<usize>,
        accepts: bool,
    }

    impl Sink {
        fn accepting() -> Self {
            Self {
                offered: Cell::new(0),
                accepts: true,
            }
        }

        fn refusing() -> Self {
            Self {
                offered: Cell::new(0),
                accepts: false,
            }
        }
    }

    impl ContentSink for Sink {
        fn offer(&self, _: &[PathBuf]) -> Result<(), glib::BoolError> {
            self.offered.set(self.offered.get() + 1);
            if self.accepts {
                Ok(())
            } else {
                Err(glib::bool_error!("the clipboard refuses the content"))
            }
        }
    }

    struct Workspace(PathBuf);

    impl Workspace {
        fn new() -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            let path = std::env::temp_dir().join(format!(
                "pigoune-clipboard-install-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("workspace");
            Self(path)
        }
    }

    impl Drop for Workspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn library(workspace: &Workspace) -> (Library, pigoune_core::AssetId) {
        let mut library = Library::create(&workspace.0, "Essai").expect("library");
        let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures/red-dot.png");
        let ImportOutcome::Imported(asset) = library.import_file(&sample, None).expect("import")
        else {
            panic!("the image is new");
        };
        (library, asset)
    }

    fn copy(library: &Library, asset: pigoune_core::AssetId) -> Vec<PathBuf> {
        library
            .clipboard_copies(&[asset], &FlatFiles::new())
            .expect("copy is prepared")
    }

    #[test]
    fn an_accepted_content_keeps_its_folder() {
        let workspace = Workspace::new();
        let (library, asset) = library(&workspace);
        let copies = copy(&library, asset);
        let sink = Sink::accepting();

        let result = install(&library, &sink, &copies);

        assert_eq!(result, Ok(()));
        assert_eq!(sink.offered.get(), 1);
        assert!(copies[0].exists());
    }

    #[test]
    fn a_refused_content_loses_its_new_folder_and_leaves_the_old_content_alone() {
        let workspace = Workspace::new();
        let (library, asset) = library(&workspace);
        let old = copy(&library, asset);
        assert_eq!(install(&library, &Sink::accepting(), &old), Ok(()));
        let new = copy(&library, asset);
        let sink = Sink::refusing();

        let result = install(&library, &sink, &new);

        assert_eq!(result, Err(Failure::Refused));
        assert_eq!(sink.offered.get(), 1);
        assert!(!new[0].exists());
        assert!(old[0].exists());
        assert_eq!(
            fs::read(&old[0]).expect("old content is read"),
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../pigoune-core/tests/fixtures/red-dot.png")
            )
            .expect("sample")
        );
    }

    #[test]
    fn the_clipboard_is_not_touched_when_the_installation_cannot_be_registered() {
        let workspace = Workspace::new();
        let (library, _) = library(&workspace);
        let stranger = workspace.0.join("stranger.png");
        fs::write(&stranger, b"png").expect("file");
        let sink = Sink::accepting();

        let result = install(&library, &sink, std::slice::from_ref(&stranger));

        assert_eq!(result, Err(Failure::NotRegistered));
        assert_eq!(sink.offered.get(), 0);
        assert!(stranger.exists());
    }

    #[test]
    fn the_texts_say_that_the_clipboard_was_left_as_it_was() {
        assert!(storage_text().contains("free space"));
        assert!(storage_text().contains("left as it was"));
        assert!(failure_text().contains("left as it was"));
    }

    #[test]
    fn the_copy_installs_the_content_only_after_the_mask_question_and_the_registration() {
        let window = include_str!("window.rs");
        let start = window
            .find("async fn copy_selected(")
            .expect("the copy is in the window");
        let body: String = window[start..]
            .lines()
            .take(60)
            .collect::<Vec<_>>()
            .join("\n");
        let question = body.find("output_is_allowed").expect("the copy asks");
        let copy = body
            .find("clipboard_copies_checking")
            .expect("the copy is made");
        let install = body
            .find("clipboard_install::install")
            .expect("the content is installed");
        assert!(question < copy && copy < install);
        assert!(!body.contains("set_content"));
    }
}
