use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{
    Asset, AssetId, FormatShare, LibraryOverview, LibraryRecords, StorageUse,
    oldest_compatible_version,
};

use crate::asset_facts;
use crate::config::VERSION;
use crate::host_path;
use crate::thumbnails;

use crate::chart_slices::{self, Measure, Slice};

const FIGURES_PER_ROW: i32 = 4;
const LIBRARY_COLOR: &str = "#3584e4";
const OTHER_FILES_COLOR: &str = "#9a9996";
const FREE_SPACE_COLOR: &str = "rgba(154, 153, 150, 0.25)";
const COUNT_TOGGLE: &str = "count";
const RECORD_THUMBNAIL_SIDE: i32 = 40;

type ShowCallback = Box<dyn Fn(AssetId)>;

pub struct LibraryReport {
    pub name: String,
    pub root: PathBuf,
    pub overview: LibraryOverview,
    pub shares: Vec<FormatShare>,
    pub storage: StorageUse,
    pub records: LibraryRecords,
}

mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;
    use pigoune_core::{AssetId, FormatShare, StorageUse};

    use crate::ring_chart::PigouneRingChart;
    use crate::stacked_bar::PigouneStackedBar;

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
        pub chart_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub ring_chart: TemplateChild<PigouneRingChart>,
        #[template_child]
        pub total_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub total_caption: TemplateChild<gtk::Label>,
        #[template_child]
        pub legend_grid: TemplateChild<gtk::Grid>,
        #[template_child]
        pub measure_toggles: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub storage_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub records_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub records_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub disk_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub disk_bar: TemplateChild<PigouneStackedBar>,
        #[template_child]
        pub disk_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub location_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub disk_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub created_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub format_row: TemplateChild<adw::ActionRow>,
        pub root: RefCell<PathBuf>,
        pub shares: RefCell<Vec<FormatShare>>,
        pub storage: RefCell<Option<StorageUse>>,
        pub record_ids: RefCell<Vec<AssetId>>,
        pub on_show: RefCell<Option<super::ShowCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneLibraryInfoDialog {
        const NAME: &'static str = "PigouneLibraryInfoDialog";
        type Type = super::PigouneLibraryInfoDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            PigouneRingChart::ensure_type();
            PigouneStackedBar::ensure_type();
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
    pub fn new(report: LibraryReport, on_show: impl Fn(AssetId) + 'static) -> Self {
        let LibraryReport {
            name,
            root,
            overview,
            shares,
            storage,
            records,
        } = report;
        let (name, root, overview) = (name.as_str(), root.as_path(), &overview);
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.on_show.replace(Some(Box::new(on_show)));
        imp.root.replace(root.to_path_buf());
        imp.records_box.set_visible(overview.resources > 1);
        dialog.show_records(&records);
        imp.chart_box.set_visible(overview.resources > 0);
        imp.shares.replace(shares);
        imp.storage.replace(Some(storage));
        imp.storage_label.set_label(&storage_text(&storage));
        dialog.show_measure(Measure::Weight);
        dialog.show_disk_space(root, storage.total());
        imp.name_label.set_label(name);
        let figures = figures(overview);
        let count = i32::try_from(figures.len()).unwrap_or(0);
        for (position, (amount, caption)) in (0..).zip(figures) {
            let (column, row) = centered_cell(position, count);
            imp.figures_grid
                .attach(&figure_tile(amount, &caption), column, row, 2, 1);
        }
        imp.location_row
            .set_subtitle(&host_path::shown_path(root).to_string_lossy());
        imp.disk_row.set_subtitle(&disk_text(root));
        imp.created_row.set_visible(overview.created_at.is_some());
        if let Some(created_at) = overview.created_at {
            imp.created_row.set_subtitle(&date_text(created_at));
        }
        imp.format_row
            .set_subtitle(&compatibility_text(overview.format_version));
        dialog
    }

    fn show_records(&self, records: &LibraryRecords) {
        let entries = [
            (RecordKind::Heaviest, &records.heaviest),
            (RecordKind::Largest, &records.largest),
            (RecordKind::Newest, &records.newest),
            (RecordKind::Oldest, &records.oldest),
        ];
        let imp = self.imp();
        for (kind, asset) in entries {
            let Some(asset) = asset else {
                continue;
            };
            imp.records_list.append(&self.record_row(kind, asset));
            imp.record_ids.borrow_mut().push(asset.id);
        }
    }

    fn record_row(&self, kind: RecordKind, asset: &Asset) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(kind.title())
            .subtitle(kind.detail(asset))
            .activatable(true)
            .build();
        let picture = gtk::Image::builder()
            .pixel_size(RECORD_THUMBNAIL_SIDE)
            .valign(gtk::Align::Center)
            .build();
        row.add_prefix(&picture);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let file = self.root().join(&asset.stored_path);
        glib::spawn_future_local(glib::clone!(
            #[weak]
            picture,
            async move {
                let pixels =
                    u32::try_from(RECORD_THUMBNAIL_SIDE * picture.scale_factor()).unwrap_or(1);
                if let Some(texture) = thumbnails::render(&file, pixels, &picture).await {
                    picture.set_paintable(Some(&texture));
                }
            }
        ));
        row
    }

    #[template_callback]
    fn on_record_activated(&self, row: &gtk::ListBoxRow) {
        let imp = self.imp();
        let Some(id) = usize::try_from(row.index())
            .ok()
            .and_then(|index| imp.record_ids.borrow().get(index).copied())
        else {
            return;
        };
        self.close();
        if let Some(on_show) = imp.on_show.borrow().as_ref() {
            on_show(id);
        }
    }

    #[template_callback]
    fn on_measure_changed(&self) {
        let measure = if self.imp().measure_toggles.active_name().as_deref() == Some(COUNT_TOGGLE) {
            Measure::Count
        } else {
            Measure::Weight
        };
        self.show_measure(measure);
    }

    fn show_measure(&self, measure: Measure) {
        let imp = self.imp();
        let slices = chart_slices::slices(&imp.shares.borrow(), measure);
        imp.ring_chart.set_parts(
            slices
                .iter()
                .map(|slice| (fraction(slice.fraction), rgba(slice.color)))
                .collect(),
        );
        imp.ring_chart
            .update_property(&[gtk::accessible::Property::Label(&chart_description(
                &slices,
            ))]);
        let resources = imp
            .shares
            .borrow()
            .iter()
            .map(|share| share.count)
            .sum::<usize>();
        let bytes = imp.storage.borrow().map_or(0, |storage| storage.resources);
        match measure {
            Measure::Weight => {
                imp.total_label.set_label(&glib::format_size(bytes));
                imp.total_caption.set_label(&gettext("in total"));
            }
            Measure::Count => {
                imp.total_label.set_label(&resources.to_string());
                imp.total_caption.set_label(&ngettext(
                    "resource",
                    "resources",
                    u32::try_from(resources).unwrap_or(u32::MAX),
                ));
            }
        }
        self.fill_legend(&slices);
    }

    fn fill_legend(&self, slices: &[Slice]) {
        let grid = &self.imp().legend_grid;
        while let Some(child) = grid.first_child() {
            grid.remove(&child);
        }
        for (row, slice) in (0..).zip(slices) {
            let cells = [
                dot(slice.color),
                legend_cell(&slice.label, gtk::Align::Start, &[]),
                legend_cell(
                    &glib::format_size(slice.bytes),
                    gtk::Align::End,
                    &["numeric"],
                ),
                legend_cell(
                    &slice.count.to_string(),
                    gtk::Align::End,
                    &["numeric", "dim-label"],
                ),
                legend_cell(
                    &percent_text(slice.fraction),
                    gtk::Align::End,
                    &["numeric", "dim-label"],
                ),
            ];
            for (column, cell) in (0..).zip(cells) {
                grid.attach(&cell, column, row, 1, 1);
            }
        }
    }

    fn show_disk_space(&self, root: &Path, library_bytes: u64) {
        let imp = self.imp();
        let Some((size, free)) = disk_space(root) else {
            imp.disk_box.set_visible(false);
            return;
        };
        let others = size.saturating_sub(free).saturating_sub(library_bytes);
        imp.disk_bar.set_parts(
            [
                (library_bytes, LIBRARY_COLOR),
                (others, OTHER_FILES_COLOR),
                (free, FREE_SPACE_COLOR),
            ]
            .into_iter()
            .map(|(bytes, color)| (fraction(chart_slices::ratio(bytes, size)), rgba(color)))
            .collect(),
        );
        let text = disk_text_markup(library_bytes, others, free, size);
        imp.disk_label.set_markup(&text);
        imp.disk_bar
            .update_property(&[gtk::accessible::Property::Label(&disk_plain_text(
                free, size,
            ))]);
    }

    fn root(&self) -> PathBuf {
        self.imp().root.borrow().clone()
    }

    #[template_callback]
    fn on_copy_location_clicked(&self) {
        self.clipboard()
            .set_text(&host_path::shown_path(&self.root()).to_string_lossy());
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

fn centered_cell(position: i32, count: i32) -> (i32, i32) {
    let row = position / FIGURES_PER_ROW;
    let in_row = (count - row * FIGURES_PER_ROW).min(FIGURES_PER_ROW);
    let offset = FIGURES_PER_ROW - in_row;
    (offset + 2 * (position % FIGURES_PER_ROW), row)
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

#[derive(Clone, Copy)]
enum RecordKind {
    Heaviest,
    Largest,
    Newest,
    Oldest,
}

impl RecordKind {
    fn title(self) -> String {
        match self {
            Self::Heaviest => gettext("Heaviest"),
            Self::Largest => gettext("Largest"),
            Self::Newest => gettext("Newest"),
            Self::Oldest => gettext("Oldest"),
        }
    }

    fn detail(self, asset: &Asset) -> String {
        let detail = match self {
            Self::Heaviest => glib::format_size(asset.byte_size).to_string(),
            Self::Largest => asset.dimensions.map_or_else(String::new, |size| {
                format!("{} × {}", size.width(), size.height())
            }),
            Self::Newest | Self::Oldest => gettext("added on {date}").replace(
                "{date}",
                &asset_facts::added_at_text(asset.added_at_unix_ms),
            ),
        };
        format!("{} · {detail}", asset.display_name)
    }
}

fn storage_text(storage: &StorageUse) -> String {
    [
        gettext("Resources {size}").replace("{size}", &glib::format_size(storage.resources)),
        gettext("Trash {size}").replace("{size}", &glib::format_size(storage.trash)),
        gettext("Thumbnails {size}").replace("{size}", &glib::format_size(storage.thumbnails)),
        gettext("Database {size}").replace("{size}", &glib::format_size(storage.database)),
    ]
    .join(" · ")
}

fn chart_description(slices: &[Slice]) -> String {
    slices
        .iter()
        .map(|slice| format!("{} {}", slice.label, percent_text(slice.fraction)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn percent_text(fraction: f64) -> String {
    gettext("{percent} %").replace("{percent}", &format!("{:.0}", fraction * 100.0))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a share between zero and one is drawn with float precision"
)]
fn fraction(value: f64) -> f32 {
    value as f32
}

fn rgba(color: &str) -> gtk::gdk::RGBA {
    gtk::gdk::RGBA::parse(color).unwrap_or(gtk::gdk::RGBA::BLACK)
}

fn dot(color: &str) -> gtk::Widget {
    let label = gtk::Label::new(None);
    label.set_markup(&format!("<span foreground=\"{color}\">●</span>"));
    label.upcast()
}

fn legend_cell(text: &str, align: gtk::Align, classes: &[&str]) -> gtk::Widget {
    let label = gtk::Label::builder().label(text).halign(align).build();
    for class in classes {
        label.add_css_class(class);
    }
    label.upcast()
}

fn disk_space(root: &Path) -> Option<(u64, u64)> {
    let info = gio::File::for_path(root)
        .query_filesystem_info("filesystem::size,filesystem::free", gio::Cancellable::NONE)
        .ok()?;
    let size = info.attribute_uint64("filesystem::size");
    let free = info.attribute_uint64("filesystem::free");
    (size > 0).then_some((size, free.min(size)))
}

fn disk_text_markup(library: u64, others: u64, free: u64, size: u64) -> String {
    let legend = |color: &str, text: String| {
        format!(
            "<span foreground=\"{color}\">●</span> {}",
            glib::markup_escape_text(&text)
        )
    };
    [
        legend(
            LIBRARY_COLOR,
            gettext("This library {size}").replace("{size}", &glib::format_size(library)),
        ),
        legend(
            OTHER_FILES_COLOR,
            gettext("Other files {size}").replace("{size}", &glib::format_size(others)),
        ),
        glib::markup_escape_text(&disk_plain_text(free, size)).to_string(),
    ]
    .join("    ")
}

fn disk_plain_text(free: u64, size: u64) -> String {
    gettext("{free} free of {size}")
        .replace("{free}", &glib::format_size(free))
        .replace("{size}", &glib::format_size(size))
}

fn compatibility_text(format_version: u32) -> String {
    let version = oldest_compatible_version(format_version).unwrap_or(VERSION);
    gettext("Pigoune {version} or later").replace("{version}", version)
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
        .map_or_else(|| gettext("Unknown date"), day_of)
}

fn day_of(unix_seconds: i64) -> String {
    glib::DateTime::from_unix_local(unix_seconds)
        .and_then(|date| date.format(&gettext("%B %-d, %Y")))
        .map_or_else(|_| gettext("Unknown date"), |text| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::centered_cell;

    #[test]
    fn a_shorter_last_row_of_figures_is_centered() {
        let cells: Vec<(i32, i32)> = (0..7).map(|position| centered_cell(position, 7)).collect();
        assert_eq!(
            cells,
            [(0, 0), (2, 0), (4, 0), (6, 0), (1, 1), (3, 1), (5, 1)]
        );
    }
}
