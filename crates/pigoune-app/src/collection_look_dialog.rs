use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use pigoune_core::CollectionLook;

use crate::collection_looks::{self, DEFAULT_ICON_KEY, LookChoice};

type SubmitCallback = Box<dyn Fn(&CollectionLook) -> Result<(), String>>;

const CHECKMARK_ICON: &str = "object-select-symbolic";

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use pigoune_core::CollectionLook;

    use super::SubmitCallback;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/collection-look-dialog.ui")]
    pub struct PigouneCollectionLookDialog {
        #[template_child]
        pub preview_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub preview_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub color_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub color_box: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub icon_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub icon_box: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub icon_name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub error_label: TemplateChild<gtk::Label>,
        pub look: RefCell<CollectionLook>,
        pub color_buttons: RefCell<Vec<(Option<&'static str>, gtk::ToggleButton)>>,
        pub icon_buttons: RefCell<Vec<(&'static str, gtk::ToggleButton)>>,
        pub on_submit: RefCell<Option<SubmitCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneCollectionLookDialog {
        const NAME: &'static str = "PigouneCollectionLookDialog";
        type Type = super::PigouneCollectionLookDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneCollectionLookDialog {}
    impl WidgetImpl for PigouneCollectionLookDialog {}
    impl AdwDialogImpl for PigouneCollectionLookDialog {}
}

glib::wrapper! {
    pub struct PigouneCollectionLookDialog(ObjectSubclass<imp::PigouneCollectionLookDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PigouneCollectionLookDialog {
    pub fn new(
        name: &str,
        look: &CollectionLook,
        on_submit: impl Fn(&CollectionLook) -> Result<(), String> + 'static,
    ) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        dialog.set_title(&gettext("Customize “{name}”").replace("{name}", name));
        imp.preview_row.set_title(&glib::markup_escape_text(name));
        imp.look.replace(look.clone());
        imp.on_submit.replace(Some(Box::new(on_submit)));
        dialog.build_color_buttons();
        dialog.build_icon_buttons();
        dialog.show_look();
        dialog
    }

    fn build_color_buttons(&self) {
        let imp = self.imp();
        let mut buttons = vec![(
            None,
            self.color_button(None, &collection_looks::default_color_label()),
        )];
        for LookChoice { key, label } in collection_looks::color_choices() {
            buttons.push((Some(key), self.color_button(Some(key), &label)));
        }
        let first = buttons[0].1.clone();
        for (_, button) in &buttons {
            if button != &first {
                button.set_group(Some(&first));
            }
            imp.color_box.append(button);
        }
        imp.color_buttons.replace(buttons);
    }

    fn color_button(&self, key: Option<&'static str>, label: &str) -> gtk::ToggleButton {
        let checkmark = gtk::Image::from_icon_name(CHECKMARK_ICON);
        let button = gtk::ToggleButton::builder()
            .child(&checkmark)
            .tooltip_text(label)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes(["collection-swatch", swatch_class(key)])
            .build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        button.update_relation(&[gtk::accessible::Relation::DescribedBy(&[self
            .imp()
            .color_title
            .upcast_ref()])]);
        button.connect_toggled(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |button| {
                if button.is_active() {
                    dialog.choose_color(key);
                }
            }
        ));
        button
    }

    fn build_icon_buttons(&self) {
        let imp = self.imp();
        let mut buttons = Vec::new();
        for LookChoice { key, label } in collection_looks::icon_choices() {
            buttons.push((key, self.icon_button(key, &label)));
        }
        let first = buttons[0].1.clone();
        for (_, button) in &buttons {
            if button != &first {
                button.set_group(Some(&first));
            }
            imp.icon_box.append(button);
        }
        imp.icon_buttons.replace(buttons);
    }

    fn icon_button(&self, key: &'static str, label: &str) -> gtk::ToggleButton {
        let image = gtk::Image::from_icon_name(collection_looks::icon_name_of_key(key));
        let button = gtk::ToggleButton::builder()
            .child(&image)
            .tooltip_text(label)
            .css_classes(["flat", "collection-icon-choice"])
            .build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        button.update_relation(&[gtk::accessible::Relation::DescribedBy(&[self
            .imp()
            .icon_title
            .upcast_ref()])]);
        button.connect_toggled(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |button| {
                if button.is_active() {
                    dialog.choose_icon(key);
                }
            }
        ));
        button
    }

    fn choose_color(&self, key: Option<&'static str>) {
        self.imp().look.borrow_mut().color = key.map(str::to_owned);
        self.show_look();
    }

    fn choose_icon(&self, key: &'static str) {
        self.imp().look.borrow_mut().icon = (key != DEFAULT_ICON_KEY).then(|| key.to_owned());
        self.show_look();
    }

    fn show_look(&self) {
        let imp = self.imp();
        let look = imp.look.borrow().clone();
        let color_class = collection_looks::color_class(&look);
        set_tint(imp.preview_icon.upcast_ref(), color_class);
        imp.preview_icon
            .set_icon_name(Some(collection_looks::icon_name(&look)));
        let chosen_color = collection_looks::known_color_key(&look);
        for (key, button) in imp.color_buttons.borrow().iter() {
            button.set_active(*key == chosen_color);
        }
        let chosen_icon = collection_looks::known_icon_key(&look);
        for (key, button) in imp.icon_buttons.borrow().iter() {
            button.set_active(*key == chosen_icon);
            if let Some(image) = button.child() {
                set_tint(&image, color_class);
            }
            if *key == chosen_icon {
                imp.icon_name_label
                    .set_label(&button.tooltip_text().unwrap_or_default());
            }
        }
    }

    #[template_callback]
    fn on_default_clicked(&self) {
        self.imp().look.replace(CollectionLook::default());
        self.show_look();
    }

    #[template_callback]
    fn on_cancel_clicked(&self) {
        self.close();
    }

    #[template_callback]
    fn on_save_clicked(&self) {
        let imp = self.imp();
        let look = imp.look.borrow().clone();
        let outcome = imp
            .on_submit
            .borrow()
            .as_ref()
            .map_or(Ok(()), |submit| submit(&look));
        match outcome {
            Ok(()) => {
                self.close();
            }
            Err(message) => {
                imp.error_label.set_label(&message);
                imp.error_label.set_visible(true);
            }
        }
    }
}

fn swatch_class(key: Option<&str>) -> &'static str {
    match key {
        Some("blue") => "swatch-blue",
        Some("teal") => "swatch-teal",
        Some("green") => "swatch-green",
        Some("yellow") => "swatch-yellow",
        Some("orange") => "swatch-orange",
        Some("red") => "swatch-red",
        Some("pink") => "swatch-pink",
        Some("purple") => "swatch-purple",
        Some("slate") => "swatch-slate",
        _ => "swatch-default",
    }
}

pub fn set_tint(widget: &gtk::Widget, color_class: Option<&str>) {
    for class in collection_looks::COLOR_CLASSES {
        widget.remove_css_class(class);
    }
    if let Some(class) = color_class {
        widget.add_css_class(class);
    }
}
