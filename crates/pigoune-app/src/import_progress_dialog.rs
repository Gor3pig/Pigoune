use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;
use pigoune_core::ImportProgress;

type CancelRequestedCallback = Box<dyn Fn()>;

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::CancelRequestedCallback;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/import-progress-dialog.ui")]
    pub struct PigouneImportProgressDialog {
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
        pub on_cancel_requested: RefCell<Option<CancelRequestedCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneImportProgressDialog {
        const NAME: &'static str = "PigouneImportProgressDialog";
        type Type = super::PigouneImportProgressDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneImportProgressDialog {}
    impl WidgetImpl for PigouneImportProgressDialog {}
    impl AdwDialogImpl for PigouneImportProgressDialog {}
}

glib::wrapper! {
    pub struct PigouneImportProgressDialog(ObjectSubclass<imp::PigouneImportProgressDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PigouneImportProgressDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn connect_cancel_requested(&self, callback: impl Fn() + 'static) {
        self.imp()
            .on_cancel_requested
            .replace(Some(Box::new(callback)));
    }

    pub fn show_progress(&self, progress: ImportProgress) {
        let imp = self.imp();
        if progress.total > 0 {
            imp.progress_bar
                .set_fraction(fraction(progress.done, progress.total));
        }
        imp.status_label.set_label(
            &ngettext(
                "{done} of {total} file processed",
                "{done} of {total} files processed",
                u32::try_from(progress.total).unwrap_or(u32::MAX),
            )
            .replace("{done}", &progress.done.to_string())
            .replace("{total}", &progress.total.to_string()),
        );
    }

    #[template_callback]
    fn on_cancel_clicked(&self) {
        let imp = self.imp();
        imp.cancel_button.set_sensitive(false);
        imp.cancel_button.set_label(&gettext("Canceling…"));
        if let Some(on_cancel_requested) = imp.on_cancel_requested.borrow().as_ref() {
            on_cancel_requested();
        }
    }
}

impl Default for PigouneImportProgressDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a progress bar does not need exact counts beyond 2^52 files"
)]
fn fraction(done: usize, total: usize) -> f64 {
    done as f64 / total as f64
}
