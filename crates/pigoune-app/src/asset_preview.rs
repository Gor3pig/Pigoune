use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};

use crate::asset_object::PigouneAssetObject;
use crate::thumbnails::{self, ThumbnailCache};

const BACKGROUNDS: [&str; 5] = ["transparent", "white", "grey", "black", "checkerboard"];
const SMALLEST_RENDER_PIXELS: u32 = 256;

type ClosedCallback = Box<dyn Fn()>;

mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gdk, glib};

    use super::ClosedCallback;
    use crate::thumbnails::ThumbnailCache;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/asset-preview.ui")]
    #[properties(wrapper_type = super::PigouneAssetPreview)]
    pub struct PigouneAssetPreview {
        #[template_child]
        pub preview_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub surface: TemplateChild<gtk::Box>,
        #[template_child]
        pub picture: TemplateChild<gtk::Picture>,
        #[property(get, set = Self::set_background)]
        pub background: RefCell<String>,
        pub selection: RefCell<Option<gtk::SingleSelection>>,
        pub selection_handler: RefCell<Option<glib::SignalHandlerId>>,
        pub thumbnails: RefCell<Option<Rc<ThumbnailCache>>>,
        pub loading: RefCell<Option<glib::JoinHandle<()>>>,
        pub on_closed: RefCell<Option<ClosedCallback>>,
    }

    impl PigouneAssetPreview {
        fn set_background(&self, background: String) {
            for known in super::BACKGROUNDS {
                self.surface.remove_css_class(known);
            }
            self.surface.add_css_class(&background);
            self.background.replace(background);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneAssetPreview {
        const NAME: &'static str = "PigouneAssetPreview";
        type Type = super::PigouneAssetPreview;
        type ParentType = adw::Bin;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
            class.add_binding(gdk::Key::Escape, gdk::ModifierType::empty(), |preview| {
                preview.close();
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::space, gdk::ModifierType::empty(), |preview| {
                preview.close();
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::Left, gdk::ModifierType::empty(), |preview| {
                preview.step(-1);
                glib::Propagation::Stop
            });
            class.add_binding(gdk::Key::Right, gdk::ModifierType::empty(), |preview| {
                preview.step(1);
                glib::Propagation::Stop
            });
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PigouneAssetPreview {}
    impl WidgetImpl for PigouneAssetPreview {}
    impl BinImpl for PigouneAssetPreview {}
}

glib::wrapper! {
    pub struct PigouneAssetPreview(ObjectSubclass<imp::PigouneAssetPreview>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl PigouneAssetPreview {
    pub fn connect_closed(&self, callback: impl Fn() + 'static) {
        self.imp().on_closed.replace(Some(Box::new(callback)));
    }

    pub fn open(&self, selection: &gtk::SingleSelection, thumbnails: Rc<ThumbnailCache>) {
        let imp = self.imp();
        self.forget_selection();
        imp.thumbnails.replace(Some(thumbnails));
        let handler = selection.connect_selected_item_notify(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            move |selection| preview.show_selected(selection)
        ));
        imp.selection.replace(Some(selection.clone()));
        imp.selection_handler.replace(Some(handler));
        self.show_selected(selection);
        self.grab_focus();
    }

    pub fn close(&self) {
        self.forget_selection();
        if let Some(on_closed) = self.imp().on_closed.borrow().as_ref() {
            on_closed();
        }
    }

    #[template_callback]
    fn on_back_clicked(&self) {
        self.close();
    }

    fn forget_selection(&self) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        if let (Some(selection), Some(handler)) =
            (imp.selection.take(), imp.selection_handler.take())
        {
            selection.disconnect(handler);
        }
        imp.picture.set_paintable(None::<&gdk::Paintable>);
    }

    fn step(&self, offset: i32) {
        let Some(selection) = self.imp().selection.borrow().clone() else {
            return;
        };
        if let Some(position) = neighbour(selection.selected(), offset, selection.n_items()) {
            selection.set_selected(position);
        }
    }

    fn show_selected(&self, selection: &gtk::SingleSelection) {
        let Some(asset) = selection
            .selected_item()
            .and_downcast::<PigouneAssetObject>()
        else {
            self.close();
            return;
        };
        let imp = self.imp();
        imp.preview_title.set_title(asset.display_name());
        imp.preview_title
            .set_subtitle(&position_text(selection.selected(), selection.n_items()));
        let remembered = imp
            .thumbnails
            .borrow()
            .as_ref()
            .and_then(|thumbnails| thumbnails.remembered(asset.id()));
        imp.picture.set_paintable(remembered.as_ref());
        self.render(&asset);
    }

    fn render(&self, asset: &PigouneAssetObject) {
        let imp = self.imp();
        if let Some(loading) = imp.loading.take() {
            loading.abort();
        }
        let pixels = self.render_pixels();
        let file = asset.file().to_path_buf();
        let loading = glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = preview)]
            self,
            async move {
                if let Some(texture) = thumbnails::render(&file, pixels, &preview).await {
                    preview.imp().picture.set_paintable(Some(&texture));
                }
            }
        ));
        imp.loading.replace(Some(loading));
    }

    fn render_pixels(&self) -> u32 {
        let longest_side = self
            .root()
            .map_or(0, |root| root.width().max(root.height()));
        u32::try_from(longest_side * self.scale_factor())
            .unwrap_or(0)
            .max(SMALLEST_RENDER_PIXELS)
    }
}

impl Default for PigouneAssetPreview {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn neighbour(current: u32, offset: i32, count: u32) -> Option<u32> {
    if current == gtk::INVALID_LIST_POSITION {
        return None;
    }
    let target = i64::from(current) + i64::from(offset);
    u32::try_from(target)
        .ok()
        .filter(|position| *position < count)
}

fn position_text(position: u32, count: u32) -> String {
    gettext("{position} of {count}")
        .replace("{position}", &(position + 1).to_string())
        .replace("{count}", &count.to_string())
}

#[cfg(test)]
mod tests {
    use super::neighbour;

    #[test]
    fn navigation_moves_by_one_inside_the_grid() {
        assert_eq!(neighbour(3, 1, 10), Some(4));
        assert_eq!(neighbour(3, -1, 10), Some(2));
    }

    #[test]
    fn navigation_stops_at_both_ends() {
        assert_eq!(neighbour(0, -1, 10), None);
        assert_eq!(neighbour(9, 1, 10), None);
    }

    #[test]
    fn navigation_needs_a_selection() {
        assert_eq!(neighbour(gtk::INVALID_LIST_POSITION, 1, 10), None);
    }
}
