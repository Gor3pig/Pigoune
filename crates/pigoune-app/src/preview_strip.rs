use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use crate::asset_object::PigouneAssetObject;
use crate::thumbnails::{self, ThumbnailCache};

const STRIP_THUMBNAIL_PIXELS: i32 = 56;

type ChosenCallback = Box<dyn Fn(u32)>;

mod imp {
    use std::cell::{OnceCell, RefCell};
    use std::rc::Rc;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::ChosenCallback;
    use crate::thumbnails::ThumbnailCache;

    #[derive(Default)]
    pub struct PigounePreviewStrip {
        pub list_view: OnceCell<gtk::ListView>,
        pub selection: OnceCell<gtk::SingleSelection>,
        pub thumbnails: RefCell<Option<Rc<ThumbnailCache>>>,
        pub on_chosen: RefCell<Option<ChosenCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigounePreviewStrip {
        const NAME: &'static str = "PigounePreviewStrip";
        type Type = super::PigounePreviewStrip;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for PigounePreviewStrip {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for PigounePreviewStrip {}
    impl BinImpl for PigounePreviewStrip {}
}

glib::wrapper! {
    pub struct PigounePreviewStrip(ObjectSubclass<imp::PigounePreviewStrip>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigounePreviewStrip {
    pub fn connect_chosen(&self, callback: impl Fn(u32) + 'static) {
        self.imp().on_chosen.replace(Some(Box::new(callback)));
    }

    pub fn show_items(&self, items: &[PigouneAssetObject], thumbnails: Rc<ThumbnailCache>) {
        let imp = self.imp();
        imp.thumbnails.replace(Some(thumbnails));
        let store = gio::ListStore::new::<PigouneAssetObject>();
        store.extend_from_slice(items);
        if let Some(selection) = imp.selection.get() {
            selection.set_model(Some(&store));
        }
    }

    pub fn count(&self) -> u32 {
        self.imp()
            .selection
            .get()
            .map_or(0, gtk::prelude::ListModelExt::n_items)
    }

    pub fn point_out(&self, position: u32) {
        let imp = self.imp();
        let (Some(selection), Some(list_view)) = (imp.selection.get(), imp.list_view.get()) else {
            return;
        };
        selection.set_selected(position);
        list_view.scroll_to(position, gtk::ListScrollFlags::NONE, None);
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = strip)]
            self,
            move || strip.center_on(position)
        ));
    }

    fn center_on(&self, position: u32) {
        let count = self.count();
        let Some(list_view) = self.imp().list_view.get() else {
            return;
        };
        let Some(adjustment) = list_view.hadjustment() else {
            return;
        };
        if count == 0 {
            return;
        }
        let item_width = adjustment.upper() / f64::from(count);
        let centered = (f64::from(position) + 0.5) * item_width - adjustment.page_size() / 2.0;
        let furthest = (adjustment.upper() - adjustment.page_size()).max(0.0);
        adjustment.set_value(centered.clamp(0.0, furthest));
    }

    fn build(&self) {
        let imp = self.imp();
        let selection = gtk::SingleSelection::builder()
            .autoselect(false)
            .can_unselect(false)
            .build();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                let picture = gtk::Picture::builder()
                    .can_shrink(true)
                    .content_fit(gtk::ContentFit::Contain)
                    .width_request(STRIP_THUMBNAIL_PIXELS)
                    .height_request(STRIP_THUMBNAIL_PIXELS)
                    .build();
                let frame = gtk::Box::builder()
                    .css_classes(["preview-strip-item"])
                    .build();
                frame.append(&picture);
                item.set_child(Some(&frame));
            }
        });
        factory.connect_bind(glib::clone!(
            #[weak(rename_to = strip)]
            self,
            move |_, item| strip.show_thumbnail(item)
        ));
        let list_view = gtk::ListView::builder()
            .model(&selection)
            .factory(&factory)
            .orientation(gtk::Orientation::Horizontal)
            .single_click_activate(true)
            .can_focus(false)
            .css_classes(["preview-strip-list"])
            .build();
        list_view.connect_activate(glib::clone!(
            #[weak(rename_to = strip)]
            self,
            move |_, position| {
                if let Some(on_chosen) = strip.imp().on_chosen.borrow().as_ref() {
                    on_chosen(position);
                }
            }
        ));
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&list_view)
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .build();
        self.set_child(Some(&scrolled));
        self.add_css_class("preview-strip");
        self.update_property(&[gtk::accessible::Property::Label(&gettext(
            "Thumbnail Strip",
        ))]);
        let _ = imp.list_view.set(list_view);
        let _ = imp.selection.set(selection);
    }

    fn show_thumbnail(&self, item: &glib::Object) {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(asset) = item.item().and_downcast::<PigouneAssetObject>() else {
            return;
        };
        let Some(picture) = item
            .child()
            .and_then(|frame| frame.first_child())
            .and_downcast::<gtk::Picture>()
        else {
            return;
        };
        let cache = self.imp().thumbnails.borrow().clone();
        let Some(cache) = cache else {
            return;
        };
        item.set_accessible_label(&asset.display_name());
        if let Some(texture) = cache.remembered(asset.id()) {
            picture.set_paintable(Some(&texture));
            return;
        }
        picture.set_paintable(None::<&gdk::Paintable>);
        let item = item.clone();
        glib::spawn_future_local(async move {
            let texture =
                thumbnails::thumbnail(asset.file(), asset.thumbnail_file(), &picture).await;
            let Some(texture) = texture else {
                return;
            };
            cache.remember(asset.id(), texture.clone());
            let still_shown = item
                .item()
                .and_downcast::<PigouneAssetObject>()
                .is_some_and(|shown| shown.id() == asset.id());
            if still_shown {
                picture.set_paintable(Some(&texture));
            }
        });
    }
}
