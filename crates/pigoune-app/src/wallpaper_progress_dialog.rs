use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;

use crate::image_conversion::WallpaperStep;

const STEPS: f64 = 3.0;

mod imp {
    use adw::subclass::prelude::*;
    use gtk::glib;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/wallpaper-progress-dialog.ui")]
    pub struct PigouneWallpaperProgressDialog {
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneWallpaperProgressDialog {
        const NAME: &'static str = "PigouneWallpaperProgressDialog";
        type Type = super::PigouneWallpaperProgressDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneWallpaperProgressDialog {}
    impl WidgetImpl for PigouneWallpaperProgressDialog {}
    impl AdwDialogImpl for PigouneWallpaperProgressDialog {}
}

glib::wrapper! {
    pub struct PigouneWallpaperProgressDialog(ObjectSubclass<imp::PigouneWallpaperProgressDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl PigouneWallpaperProgressDialog {
    pub fn new() -> Self {
        let dialog: Self = glib::Object::new();
        dialog.show_step(WallpaperStep::Loading);
        dialog
    }

    pub fn show_step(&self, step: WallpaperStep) {
        let imp = self.imp();
        let (done, text) = match step {
            WallpaperStep::Loading => (0.0, gettext("Loading the image…")),
            WallpaperStep::Framing => (1.0, gettext("Framing the image to the screen…")),
            WallpaperStep::Saving => (2.0, gettext("Saving the wallpaper…")),
        };
        imp.progress_bar.set_fraction(done / STEPS);
        imp.status_label.set_label(&text);
    }
}

impl Default for PigouneWallpaperProgressDialog {
    fn default() -> Self {
        Self::new()
    }
}
