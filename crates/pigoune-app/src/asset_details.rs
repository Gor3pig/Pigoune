use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gdk, glib};
use pigoune_core::{AnimationTiming, Asset, CollectionId, Tag, TextField};

use crate::animation;
use crate::asset_facts;
use crate::asset_object::PigouneAssetObject;
use crate::collection_places::SharedCollection;
use crate::tag_editor::{PigouneTagEditor, SharedTag};
use crate::thumbnails::{self, ThumbnailCache};

type RenamedCallback = Box<dyn Fn(&PigouneAssetObject, String)>;
type TextChangedCallback = Box<dyn Fn(&PigouneAssetObject, TextField, String)>;

fn watch_focus_leave(widget: &gtk::Widget, details: &PigouneAssetDetails) {
    let focus = gtk::EventControllerFocus::new();
    focus.connect_leave(glib::clone!(
        #[weak]
        details,
        move |_| details.save_texts()
    ));
    widget.add_controller(focus);
}

fn is_web_link(text: &str) -> bool {
    let text = text.trim();
    text.starts_with("https://") || text.starts_with("http://")
}

const NOTHING_PAGE: &str = "nothing";
const ASSET_PAGE: &str = "asset";
const GROUP_PAGE: &str = "group";
const TRASHED_PAGE: &str = "trashed";
const PREVIEW_PIXELS: u32 = 512;

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use super::{RenamedCallback, TextChangedCallback};
    use crate::asset_object::PigouneAssetObject;
    use crate::collection_places::PigouneCollectionPlaces;
    use crate::tag_summary::PigouneTagSummary;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-details.ui")]
    pub struct PigouneAssetDetails {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub preview: TemplateChild<gtk::Picture>,
        #[template_child]
        pub name_label: TemplateChild<gtk::EditableLabel>,
        #[template_child]
        pub note_view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub source_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub open_source_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub license_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub author_row: TemplateChild<adw::EntryRow>,
        pub showing: RefCell<Option<PigouneAssetObject>>,
        pub on_renamed: RefCell<Option<RenamedCallback>>,
        pub on_text_changed: RefCell<Option<TextChangedCallback>>,
        #[template_child]
        pub favorite_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub tag_summary: TemplateChild<PigouneTagSummary>,
        #[template_child]
        pub group_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub trashed_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub group_favorite_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub group_tag_summary: TemplateChild<PigouneTagSummary>,
        #[template_child]
        pub collection_places: TemplateChild<PigouneCollectionPlaces>,
        #[template_child]
        pub group_collection_places: TemplateChild<PigouneCollectionPlaces>,
        pub favorite_bindings: RefCell<Vec<glib::Binding>>,
        #[template_child]
        pub format_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub dimensions_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub size_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub added_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub original_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub animation_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub summary_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub embedded_row: TemplateChild<adw::ActionRow>,
        pub loading: RefCell<Option<glib::JoinHandle<()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetDetails {
        const NAME: &'static str = "PigouneAssetDetails";
        type Type = super::PigouneAssetDetails;
        type ParentType = adw::Bin;

        fn class_init(class: &mut Self::Class) {
            PigouneTagSummary::ensure_type();
            PigouneCollectionPlaces::ensure_type();
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneAssetDetails {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().save_edits_when_done();
        }
    }
    impl WidgetImpl for PigouneAssetDetails {}
    impl BinImpl for PigouneAssetDetails {}
}

glib::wrapper! {
    pub struct PigouneAssetDetails(ObjectSubclass<imp::PigouneAssetDetails>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl PigouneAssetDetails {
    pub fn connect_renamed(&self, callback: impl Fn(&PigouneAssetObject, String) + 'static) {
        self.imp().on_renamed.replace(Some(Box::new(callback)));
    }

    pub fn connect_text_changed(
        &self,
        callback: impl Fn(&PigouneAssetObject, TextField, String) + 'static,
    ) {
        self.imp().on_text_changed.replace(Some(Box::new(callback)));
    }

    pub fn start_renaming(&self) {
        let name_label = &self.imp().name_label;
        name_label.start_editing();
        name_label.grab_focus();
    }

    #[template_callback]
    fn on_open_source_clicked(&self) {
        let url = self.imp().source_row.text();
        let window = self.root().and_downcast::<gtk::Window>();
        gtk::UriLauncher::new(&url).launch(window.as_ref(), None::<&gtk::gio::Cancellable>, |_| {});
    }

    fn save_edits_when_done(&self) {
        let imp = self.imp();
        imp.name_label.connect_editing_notify(glib::clone!(
            #[weak(rename_to = details)]
            self,
            move |name_label| {
                if !name_label.is_editing() {
                    details.save_name();
                }
            }
        ));
        for row in [&*imp.source_row, &*imp.license_row, &*imp.author_row] {
            row.connect_entry_activated(glib::clone!(
                #[weak(rename_to = details)]
                self,
                move |_| details.save_texts()
            ));
            watch_focus_leave(row.upcast_ref(), self);
        }
        watch_focus_leave(imp.note_view.upcast_ref(), self);
        imp.source_row.connect_changed(glib::clone!(
            #[weak(rename_to = details)]
            self,
            move |row| {
                details
                    .imp()
                    .open_source_button
                    .set_visible(is_web_link(&row.text()));
            }
        ));
    }

    fn save_name(&self) {
        let imp = self.imp();
        let Some(object) = imp.showing.borrow().clone() else {
            return;
        };
        let typed = imp.name_label.text().trim().to_owned();
        if typed.is_empty() {
            imp.name_label.set_text(&object.display_name());
            return;
        }
        if typed != object.display_name()
            && let Some(on_renamed) = imp.on_renamed.borrow().as_ref()
        {
            on_renamed(&object, typed);
        }
    }

    fn save_texts(&self) {
        let imp = self.imp();
        let Some(object) = imp.showing.borrow().clone() else {
            return;
        };
        for (field, typed) in self.typed_texts() {
            if typed.trim() != object.text(field)
                && let Some(on_text_changed) = imp.on_text_changed.borrow().as_ref()
            {
                on_text_changed(&object, field, typed);
            }
        }
    }

    fn typed_texts(&self) -> [(TextField, String); 4] {
        let imp = self.imp();
        let buffer = imp.note_view.buffer();
        let (start, end) = buffer.bounds();
        [
            (
                TextField::Note,
                buffer.text(&start, &end, false).to_string(),
            ),
            (TextField::SourceUrl, imp.source_row.text().to_string()),
            (TextField::License, imp.license_row.text().to_string()),
            (TextField::Author, imp.author_row.text().to_string()),
        ]
    }

    fn show_texts(&self, object: &PigouneAssetObject) {
        let imp = self.imp();
        imp.note_view.buffer().set_text(&object.note());
        imp.source_row.set_text(&object.source_url());
        imp.license_row.set_text(&object.license());
        imp.author_row.set_text(&object.author());
    }

    pub fn tag_editors(&self) -> [PigouneTagEditor; 2] {
        let imp = self.imp();
        [imp.tag_summary.editor(), imp.group_tag_summary.editor()]
    }

    pub fn focus_tag_entry(&self) {
        let imp = self.imp();
        if self.is_showing_group() {
            imp.group_tag_summary.open_editor();
        } else {
            imp.tag_summary.open_editor();
        }
    }

    fn is_showing_group(&self) -> bool {
        self.imp().stack.visible_child_name().as_deref() == Some(GROUP_PAGE)
    }

    pub fn show_tags(&self, current: Vec<SharedTag>, all: Vec<Tag>) {
        let imp = self.imp();
        if self.is_showing_group() {
            imp.group_tag_summary.show_tags(current, all);
        } else {
            imp.tag_summary.show_tags(current, all);
        }
    }

    pub fn connect_collection_opened(&self, callback: impl Fn(CollectionId) + 'static) {
        let imp = self.imp();
        let callback = Rc::new(callback);
        let for_group = Rc::clone(&callback);
        imp.group_collection_places
            .connect_opened(move |collection| for_group(collection));
        imp.collection_places
            .connect_opened(move |collection| callback(collection));
    }

    pub fn show_collections(&self, current: &[SharedCollection], selected: usize) {
        let imp = self.imp();
        if self.is_showing_group() {
            imp.group_collection_places
                .show_collections(current, selected);
        } else {
            imp.collection_places.show_collections(current, selected);
        }
    }

    pub fn show_trashed(&self, selected: &[PigouneAssetObject]) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        self.save_texts();
        imp.showing.replace(None);
        let title = match selected {
            [single] => single.display_name(),
            several => ngettext(
                "{count} resource selected",
                "{count} resources selected",
                u32::try_from(several.len()).unwrap_or(u32::MAX),
            )
            .replace("{count}", &several.len().to_string()),
        };
        imp.trashed_page.set_title(&title);
        imp.stack.set_visible_child_name(TRASHED_PAGE);
    }

    pub fn show_group(&self, selected: &[PigouneAssetObject]) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        self.save_texts();
        imp.showing.replace(None);
        imp.preview.set_paintable(None::<&gdk::Paintable>);
        let count = selected.len();
        imp.group_title.set_label(
            &ngettext(
                "{count} resource selected",
                "{count} resources selected",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
            .replace("{count}", &count.to_string()),
        );
        let all_favorite = selected.iter().all(PigouneAssetObject::favorite);
        imp.group_favorite_button.set_label(&if all_favorite {
            gettext("Remove from Favorites")
        } else {
            gettext("Add to Favorites")
        });
        imp.stack.set_visible_child_name(GROUP_PAGE);
    }

    pub fn show(&self, selected: Option<&PigouneAssetObject>, thumbnails: &Rc<ThumbnailCache>) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        self.save_texts();
        imp.showing.replace(selected.cloned());
        if let Some(asset) = selected {
            self.show_texts(asset);
            self.describe(asset);
            self.show_preview(asset, thumbnails);
            imp.stack.set_visible_child_name(ASSET_PAGE);
        } else {
            imp.preview.set_paintable(None::<&gdk::Paintable>);
            imp.stack.set_visible_child_name(NOTHING_PAGE);
        }
    }

    fn describe(&self, object: &PigouneAssetObject) {
        let imp = self.imp();
        let asset = object.asset();
        imp.name_label.set_text(&object.display_name());
        for binding in imp.favorite_bindings.take() {
            binding.unbind();
        }
        let icon = object
            .bind_property("favorite", &*imp.favorite_button, "icon-name")
            .transform_to(|_, favorite: bool| {
                Some(if favorite {
                    "starred-symbolic"
                } else {
                    "non-starred-symbolic"
                })
            })
            .sync_create()
            .build();
        let tooltip = object
            .bind_property("favorite", &*imp.favorite_button, "tooltip-text")
            .transform_to(|_, favorite: bool| {
                Some(if favorite {
                    gettext("Remove from Favorites")
                } else {
                    gettext("Add to Favorites")
                })
            })
            .sync_create()
            .build();
        imp.favorite_bindings.replace(vec![icon, tooltip]);
        imp.format_row
            .set_subtitle(asset_facts::format_name(asset.format));
        imp.dimensions_row
            .set_subtitle(&asset_facts::dimensions_text(asset.dimensions));
        imp.size_row
            .set_subtitle(&asset_facts::byte_size_text(asset.byte_size));
        imp.added_row
            .set_subtitle(&asset_facts::added_at_text(asset.added_at_unix_ms));
        imp.original_row.set_subtitle(&asset.original_file_name);
        imp.animation_row.set_visible(asset.is_animated);
        self.show_animation(asset, None);
        imp.embedded_row
            .set_visible(!asset.embedded_sizes.is_empty());
        imp.embedded_row
            .set_subtitle(&asset_facts::embedded_sizes_text(&asset.embedded_sizes));
    }

    pub fn show_animation_timing(&self, timing: Option<AnimationTiming>) {
        let showing = self.imp().showing.borrow().clone();
        if let Some(object) = showing {
            self.show_animation(object.asset(), timing);
        }
    }

    fn show_animation(&self, asset: &Asset, timing: Option<AnimationTiming>) {
        let imp = self.imp();
        imp.summary_label
            .set_label(&asset_facts::summary_text(asset, timing));
        imp.animation_row
            .set_subtitle(&asset_facts::animation_text(timing));
    }

    fn show_preview(&self, object: &PigouneAssetObject, thumbnails: &Rc<ThumbnailCache>) {
        let imp = self.imp();
        imp.preview
            .set_paintable(thumbnails.remembered(object.id()).as_ref());
        let file = object.file().to_path_buf();
        if object.asset().is_animated {
            let preview = imp.preview.get();
            let playing = animation::play(file, move |frame| preview.set_paintable(Some(frame)));
            imp.loading.replace(Some(playing));
            return;
        }
        let loading = glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = details)]
            self,
            async move {
                if let Some(texture) = thumbnails::render(&file, PREVIEW_PIXELS, &details).await {
                    details.imp().preview.set_paintable(Some(&texture));
                }
            }
        ));
        imp.loading.replace(Some(loading));
    }
}

#[cfg(test)]
mod tests {
    use super::is_web_link;

    #[test]
    fn only_web_addresses_can_be_opened() {
        assert!(is_web_link("https://github.com/logos"));
        assert!(is_web_link("  http://example.org "));
        assert!(!is_web_link("livre X, page 12"));
        assert!(!is_web_link("ftp://example.org"));
    }
}
