use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gio, glib};
use pigoune_core::{
    CollectionId, ImportControl, ImportError, ImportProgress, ImportSummary, Library,
};

use crate::image_check::ImageCheck;
use crate::import_progress_dialog::PigouneImportProgressDialog;

const PROGRESS_DIALOG_DELAY: Duration = Duration::from_millis(500);

pub struct FinishedImport {
    pub library: Library,
    pub result: Result<ImportSummary, ImportError>,
}

pub async fn run(
    parent: &impl IsA<gtk::Widget>,
    library: Library,
    paths: Vec<PathBuf>,
    target: Option<CollectionId>,
) -> Option<FinishedImport> {
    let (progress_sender, progress_receiver) = async_channel::unbounded();
    let cancel_requested = Arc::new(AtomicBool::new(false));

    let worker_cancel_requested = Arc::clone(&cancel_requested);
    let work = gio::spawn_blocking(move || {
        let mut library = library;
        let image_check = ImageCheck::start();
        let result = library.import_paths(
            &paths,
            target,
            |path| image_check.is_intact(path),
            |progress| {
                let _ = progress_sender.send_blocking(progress);
                if worker_cancel_requested.load(Ordering::Relaxed) {
                    ImportControl::Cancel
                } else {
                    ImportControl::Continue
                }
            },
        );
        FinishedImport { library, result }
    });

    let dialog = PigouneImportProgressDialog::new();
    dialog.connect_cancel_requested(move || cancel_requested.store(true, Ordering::Relaxed));
    let pending_dialog = show_dialog_later(parent, &dialog);
    follow_progress(&dialog, progress_receiver);

    let finished = work.await.ok();

    if let Some(source) = pending_dialog.take() {
        source.remove();
    } else {
        dialog.force_close();
    }
    finished
}

fn show_dialog_later(
    parent: &impl IsA<gtk::Widget>,
    dialog: &PigouneImportProgressDialog,
) -> Rc<Cell<Option<glib::SourceId>>> {
    let parent: gtk::Widget = parent.clone().upcast();
    let pending = Rc::new(Cell::new(None));
    let source = glib::timeout_add_local_once(
        PROGRESS_DIALOG_DELAY,
        glib::clone!(
            #[strong]
            pending,
            #[strong]
            dialog,
            #[strong]
            parent,
            move || {
                pending.set(None);
                dialog.present(Some(&parent));
            }
        ),
    );
    pending.set(Some(source));
    pending
}

fn follow_progress(
    dialog: &PigouneImportProgressDialog,
    progress_receiver: async_channel::Receiver<ImportProgress>,
) {
    glib::spawn_future_local(glib::clone!(
        #[weak]
        dialog,
        async move {
            while let Ok(progress) = progress_receiver.recv().await {
                dialog.show_progress(progress);
            }
        }
    ));
}
