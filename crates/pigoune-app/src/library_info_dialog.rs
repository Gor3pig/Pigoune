use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{LibraryOverview, oldest_compatible_version};

const FIGURES_PER_ROW: i32 = 4;

mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use adw::subclass::prelude::*;
    use gtk::glib;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/library-info-dialog.ui")]
    pub struct PigouneLibraryInfoDialog {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub figures_grid: TemplateChild<gtk::Grid>,
        #[template_child]
        pub location_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub disk_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub created_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub format_row: TemplateChild<adw::ActionRow>,
        pub root: RefCell<PathBuf>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneLibraryInfoDialog {
        const NAME: &'static str = "PigouneLibraryInfoDialog";
        type Type = super::PigouneLibraryInfoDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneLibraryInfoDialog {}
    impl WidgetImpl for PigouneLibraryInfoDialog {}
    impl AdwDialogImpl for PigouneLibraryInfoDialog {}
}

glib::wrapper! {
    pub struct PigouneLibraryInfoDialog(ObjectSubclass<imp::PigouneLibraryInfoDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PigouneLibraryInfoDialog {
    pub fn new(name: &str, root: &Path, overview: &LibraryOverview) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.root.replace(root.to_path_buf());
        imp.name_label.set_label(name);
        for (position, (count, caption)) in (0..).zip(figures(overview)) {
            imp.figures_grid.attach(
                &figure_tile(count, &caption),
                position % FIGURES_PER_ROW,
                position / FIGURES_PER_ROW,
                1,
                1,
            );
        }
        imp.location_row.set_subtitle(&root.to_string_lossy());
        imp.disk_row.set_subtitle(&disk_text(root));
        imp.created_row.set_visible(overview.created_at.is_some());
        if let Some(created_at) = overview.created_at {
            imp.created_row.set_subtitle(&date_text(created_at));
        }
        imp.format_row
            .set_subtitle(&compatibility_text(overview.format_version));
        dialog
    }

    fn root(&self) -> PathBuf {
        self.imp().root.borrow().clone()
    }

    #[template_callback]
    fn on_copy_location_clicked(&self) {
        self.clipboard().set_text(&self.root().to_string_lossy());
        self.imp()
            .toast_overlay
            .add_toast(adw::Toast::new(&gettext("Location copied")));
    }

    #[template_callback]
    fn on_open_folder_clicked(&self) {
        let folder = gio::File::for_path(self.root());
        let window = WidgetExt::root(self).and_downcast::<gtk::Window>();
        gtk::FileLauncher::new(Some(&folder)).launch(
            window.as_ref(),
            gio::Cancellable::NONE,
            |_| {},
        );
    }
}

fn figures(overview: &LibraryOverview) -> Vec<(usize, String)> {
    let amount = |count: usize| u32::try_from(count).unwrap_or(u32::MAX);
    vec![
        (
            overview.resources,
            ngettext("Resource", "Resources", amount(overview.resources)),
        ),
        (
            overview.collections,
            ngettext("Collection", "Collections", amount(overview.collections)),
        ),
        (
            overview.tags,
            ngettext("Tag", "Tags", amount(overview.tags)),
        ),
        (
            overview.favorites,
            ngettext("Favorite", "Favorites", amount(overview.favorites)),
        ),
        (
            overview.animated,
            ngettext("Animated", "Animated", amount(overview.animated)),
        ),
        (overview.vectors, gettext("SVG")),
        (
            overview.in_trash,
            ngettext("In the Trash", "In the Trash", amount(overview.in_trash)),
        ),
    ]
}

fn figure_tile(count: usize, caption: &str) -> gtk::Widget {
    let number = gtk::Label::builder()
        .label(count.to_string())
        .css_classes(["title-1", "numeric"])
        .build();
    let label = gtk::Label::builder()
        .label(caption)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["caption", "dim-label"])
        .build();
    let tile = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .css_classes(["card", "figure-tile"])
        .build();
    tile.append(&number);
    tile.append(&label);
    tile.update_property(&[gtk::accessible::Property::Label(&format!(
        "{count} {caption}"
    ))]);
    tile.upcast()
}

fn compatibility_text(format_version: u32) -> String {
    oldest_compatible_version(format_version).map_or_else(
        || gettext("Library format {number}").replace("{number}", &format_version.to_string()),
        |version| gettext("Pigoune {version} or later").replace("{version}", version),
    )
}

fn disk_text(root: &Path) -> String {
    let removable = gio::File::for_path(root)
        .find_enclosing_mount(gio::Cancellable::NONE)
        .is_ok_and(|mount| mount.can_eject());
    if removable {
        gettext("Removable disk")
    } else {
        gettext("Internal disk")
    }
}

fn date_text(moment: std::time::SystemTime) -> String {
    moment
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .and_then(|seconds| glib::DateTime::from_unix_local(seconds).ok())
        .and_then(|date| date.format(&gettext("%B %-d, %Y")).ok())
        .map_or_else(|| gettext("Unknown date"), |text| text.to_string())
}
