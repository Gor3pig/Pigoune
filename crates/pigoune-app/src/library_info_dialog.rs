use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{
    Asset, AssetId, ColorShare, FormatShare, LibraryOverview, LibraryRecords, StorageUse,
    oldest_compatible_version,
};

use crate::asset_colors;
use crate::asset_facts;
use crate::color_chips;
use crate::config::VERSION;
use crate::health_page::HealthActions;
use crate::host_path;
use crate::thumbnails;
use crate::toasts;

use crate::chart_slices::{self, Measure, Slice};

const SMALL_FIGURES: i32 = 3;
const RESOURCES_COLOR: &str = "#3584e4";
const THUMBNAILS_COLOR: &str = "#9141ac";
const DATABASE_COLOR: &str = "#e66100";
const TRASH_COLOR: &str = "#c01c28";
const OTHER_FILES_COLOR: &str = "#9a9996";
const FREE_SPACE_COLOR: &str = "rgba(154, 153, 150, 0.25)";
const COUNT_TOGGLE: &str = "count";
const RECORD_THUMBNAIL_SIDE: i32 = 40;

type ShowCallback = Box<dyn Fn(AssetId)>;
type ClearCallback = Box<dyn Fn() -> Result<(), String>>;

pub struct LibraryReport {
    pub name: String,
    pub root: PathBuf,
    pub overview: LibraryOverview,
    pub shares: Vec<FormatShare>,
    pub storage: StorageUse,
    pub records: LibraryRecords,
    pub colors: Vec<ColorShare>,
}

mod imp {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;
    use pigoune_core::{AssetId, FormatShare, StorageUse};

    use crate::health_page::PigouneHealthPage;
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
        pub place_label: TemplateChild<gtk::Label>,
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
        pub counts_box: TemplateChild<adw::WrapBox>,
        #[template_child]
        pub colors_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub colors_bar: TemplateChild<PigouneStackedBar>,
        #[template_child]
        pub colors_legend: TemplateChild<adw::WrapBox>,
        #[template_child]
        pub records_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub records_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub disk_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub disk_bar: TemplateChild<PigouneStackedBar>,
        #[template_child]
        pub disk_legend: TemplateChild<adw::WrapBox>,
        #[template_child]
        pub location_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub disk_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub thumbnails_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub thumbnails_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub created_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub format_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub health_page: TemplateChild<PigouneHealthPage>,
        pub root: RefCell<PathBuf>,
        pub shares: RefCell<Vec<FormatShare>>,
        pub storage: RefCell<Option<StorageUse>>,
        pub record_ids: RefCell<Vec<AssetId>>,
        pub on_show: RefCell<Option<super::ShowCallback>>,
        pub clear_thumbnails: RefCell<Option<super::ClearCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneLibraryInfoDialog {
        const NAME: &'static str = "PigouneLibraryInfoDialog";
        type Type = super::PigouneLibraryInfoDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            PigouneRingChart::ensure_type();
            PigouneStackedBar::ensure_type();
            PigouneHealthPage::ensure_type();
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
    pub fn new(
        report: LibraryReport,
        on_show: impl Fn(AssetId) + 'static,
        clear_thumbnails: impl Fn() -> Result<(), String> + 'static,
        health: HealthActions,
    ) -> Self {
        let LibraryReport {
            name,
            root,
            overview,
            shares,
            storage,
            records,
            colors,
        } = report;
        let (name, root, overview) = (name.as_str(), root.as_path(), &overview);
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.on_show.replace(Some(Box::new(on_show)));
        imp.clear_thumbnails
            .replace(Some(Box::new(clear_thumbnails)));
        imp.health_page.connect_actions(health);
        dialog.connect_closed(|dialog| dialog.imp().health_page.cancel());
        imp.root.replace(root.to_path_buf());
        imp.records_box.set_visible(overview.resources > 1);
        dialog.show_records(&records);
        imp.chart_box.set_visible(overview.resources > 0);
        imp.shares.replace(shares);
        imp.storage.replace(Some(storage));
        dialog.show_measure(Measure::Weight);
        dialog.show_disk_space(root, &storage);
        dialog.show_thumbnail_bytes(storage.thumbnails);
        dialog.show_colors(&colors);
        imp.name_label.set_label(name);
        let place = host_path::shown_path(root).to_string_lossy().to_string();
        imp.place_label.set_label(&place);
        imp.place_label.set_tooltip_text(Some(&place));
        dialog.show_figures(overview);
        imp.location_row.set_subtitle(&place);
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
                if let Some(texture) = thumbnails::render(&file, pixels).await {
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

    fn show_figures(&self, overview: &LibraryOverview) {
        let imp = self.imp();
        let amount = |count: usize| u32::try_from(count).unwrap_or(u32::MAX);
        let main = figure_tile(
            overview.resources,
            &ngettext("Resource", "Resources", amount(overview.resources)),
        );
        main.add_css_class("main-figure");
        imp.figures_grid.attach(&main, 0, 0, SMALL_FIGURES, 1);
        let small = [
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
        ];
        for (column, (count, caption)) in (0..).zip(small) {
            imp.figures_grid
                .attach(&figure_tile(count, &caption), column, 1, 1, 1);
        }
        let counts = [
            ngettext(
                "{count} animated",
                "{count} animated",
                amount(overview.animated),
            )
            .replace("{count}", &overview.animated.to_string()),
            gettext("{count} SVG").replace("{count}", &overview.vectors.to_string()),
            ngettext(
                "{count} in the trash",
                "{count} in the trash",
                amount(overview.in_trash),
            )
            .replace("{count}", &overview.in_trash.to_string()),
        ];
        for text in counts {
            let pill = gtk::Label::builder()
                .label(text)
                .css_classes(["tag-pill", "numeric"])
                .build();
            imp.counts_box.append(&pill);
        }
    }

    fn show_colors(&self, colors: &[ColorShare]) {
        let imp = self.imp();
        imp.colors_box.set_visible(!colors.is_empty());
        let total: usize = colors.iter().map(|share| share.count).sum();
        imp.colors_bar.set_parts(
            colors
                .iter()
                .map(|share| {
                    (
                        fraction(chart_slices::ratio(share.count as u64, total as u64)),
                        rgba(asset_colors::color_hex(share.color)),
                    )
                })
                .collect(),
        );
        let description = colors
            .iter()
            .map(|share| format!("{} {}", asset_colors::color_name(share.color), share.count))
            .collect::<Vec<_>>()
            .join(", ");
        imp.colors_bar
            .update_property(&[gtk::accessible::Property::Label(&description)]);
        for share in colors {
            imp.colors_legend
                .append(&color_chips::pill(share.color, Some(share.count)));
        }
    }

    fn show_disk_space(&self, root: &Path, storage: &StorageUse) {
        let imp = self.imp();
        let library_parts = [
            (
                storage.resources,
                RESOURCES_COLOR,
                gettext("Resources {size}"),
            ),
            (
                storage.thumbnails,
                THUMBNAILS_COLOR,
                gettext("Thumbnails {size}"),
            ),
            (storage.database, DATABASE_COLOR, gettext("Database {size}")),
            (storage.trash, TRASH_COLOR, gettext("Trash {size}")),
        ];
        let disk = disk_space(root);
        let whole = disk.map_or(storage.total(), |(size, _)| size);
        let mut parts: Vec<(u64, &str)> = library_parts
            .iter()
            .map(|(bytes, color, _)| (*bytes, *color))
            .collect();
        for (bytes, color, text) in &library_parts {
            imp.disk_legend.append(&legend_item(
                color,
                &text.replace("{size}", &glib::format_size(*bytes)),
            ));
        }
        if let Some((size, free)) = disk {
            let others = size.saturating_sub(free).saturating_sub(storage.total());
            parts.push((others, OTHER_FILES_COLOR));
            parts.push((free, FREE_SPACE_COLOR));
            imp.disk_legend.append(&legend_item(
                OTHER_FILES_COLOR,
                &gettext("Other files {size}").replace("{size}", &glib::format_size(others)),
            ));
            let free_text = gtk::Label::builder()
                .label(disk_plain_text(free, size))
                .css_classes(["caption", "dim-label"])
                .build();
            imp.disk_legend.append(&free_text);
            imp.disk_bar
                .update_property(&[gtk::accessible::Property::Label(&disk_plain_text(
                    free, size,
                ))]);
        }
        imp.disk_bar.set_parts(
            parts
                .into_iter()
                .map(|(bytes, color)| (fraction(chart_slices::ratio(bytes, whole)), rgba(color)))
                .collect(),
        );
    }

    fn root(&self) -> PathBuf {
        self.imp().root.borrow().clone()
    }

    fn show_thumbnail_bytes(&self, bytes: u64) {
        let imp = self.imp();
        imp.thumbnails_row.set_subtitle(&glib::format_size(bytes));
        imp.thumbnails_button.set_sensitive(bytes > 0);
    }

    #[template_callback]
    fn on_clear_thumbnails_clicked(&self) {
        let imp = self.imp();
        let Some(storage) = imp.storage.borrow().as_ref().copied() else {
            return;
        };
        let outcome = imp
            .clear_thumbnails
            .borrow()
            .as_ref()
            .map_or(Ok(()), |clear| clear());
        let message = match outcome {
            Ok(()) => {
                let cleared = StorageUse {
                    thumbnails: 0,
                    ..storage
                };
                imp.storage.replace(Some(cleared));
                self.show_thumbnail_bytes(0);
                imp.disk_legend.remove_all();
                self.show_disk_space(&self.root(), &cleared);
                freed_text(storage.thumbnails)
            }
            Err(reason) => failure_text(&reason),
        };
        toasts::announce(&imp.toast_overlay, &adw::Toast::new(&message));
    }

    #[template_callback]
    fn on_copy_location_clicked(&self) {
        self.clipboard()
            .set_text(&host_path::shown_path(&self.root()).to_string_lossy());
        toasts::announce(
            &self.imp().toast_overlay,
            &adw::Toast::new(&gettext("Location copied")),
        );
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

fn freed_text(bytes: u64) -> String {
    gettext("{size} freed, thumbnails are made again when needed")
        .replace("{size}", &glib::format_size(bytes))
}

fn failure_text(reason: &str) -> String {
    gettext("Unable to clear the thumbnails: {reason}").replace("{reason}", reason)
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

fn legend_item(color: &str, text: &str) -> gtk::Widget {
    let item = gtk::Box::builder().spacing(6).build();
    item.append(&dot(color));
    item.append(
        &gtk::Label::builder()
            .label(text)
            .css_classes(["caption"])
            .build(),
    );
    item.upcast()
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
