use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use pigoune_core::CollectionLook;

use crate::collection_looks::{self, IconGroup, LookChoice};

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
        pub icon_search: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub no_icon_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub icon_scroller: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub icon_content: TemplateChild<gtk::Box>,
        #[template_child]
        pub icon_name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub error_label: TemplateChild<gtk::Label>,
        pub look: RefCell<CollectionLook>,
        pub search_query: RefCell<String>,
        pub icon_groups: RefCell<Vec<(gtk::Box, gtk::FlowBox)>>,
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
        dialog.add_search_shortcut();
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
        let mut buttons: Vec<(&'static str, gtk::ToggleButton)> = Vec::new();
        let mut groups = Vec::new();
        let mut previous: Option<gtk::Widget> = None;
        for IconGroup { title, choices } in collection_looks::icon_groups() {
            let flow_box = icon_flow_box();
            for LookChoice { key, label } in choices {
                let button = self.icon_button(key, &label);
                if let Some((_, first)) = buttons.first() {
                    button.set_group(Some(first));
                }
                flow_box.append(&button);
                buttons.push((key, button));
            }
            flow_box.set_filter_func(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                #[upgrade_or]
                true,
                move |child| dialog.child_matches_search(child)
            ));
            let section = icon_section(&title, &flow_box);
            imp.icon_content
                .insert_child_after(&section, previous.as_ref());
            previous = Some(section.clone().upcast());
            groups.push((section, flow_box));
        }
        imp.icon_buttons.replace(buttons);
        imp.icon_groups.replace(groups);
        self.scroll_to_chosen_icon_when_shown();
    }

    fn add_search_shortcut(&self) {
        let controller = gtk::ShortcutController::new();
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<Control>f"),
            Some(gtk::CallbackAction::new(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_, _| {
                    dialog.imp().icon_search.grab_focus();
                    glib::Propagation::Stop
                }
            ))),
        ));
        self.add_controller(controller);
    }

    fn child_matches_search(&self, child: &gtk::FlowBoxChild) -> bool {
        child
            .child()
            .and_downcast::<gtk::ToggleButton>()
            .is_none_or(|button| self.button_matches_search(&button))
    }

    fn button_matches_search(&self, button: &gtk::ToggleButton) -> bool {
        let imp = self.imp();
        let query = imp.search_query.borrow();
        imp.icon_buttons
            .borrow()
            .iter()
            .find(|(_, candidate)| candidate == button)
            .is_some_and(|(key, candidate)| {
                collection_looks::matches_search(
                    &candidate.tooltip_text().unwrap_or_default(),
                    key,
                    &query,
                )
            })
    }

    fn flow_box_has_match(&self, flow_box: &gtk::FlowBox) -> bool {
        let mut child = flow_box.first_child();
        while let Some(widget) = child {
            if let Some(flow_child) = widget.downcast_ref::<gtk::FlowBoxChild>()
                && self.child_matches_search(flow_child)
            {
                return true;
            }
            child = widget.next_sibling();
        }
        false
    }

    fn show_search_results(&self) {
        let imp = self.imp();
        let mut any_match = false;
        for (section, flow_box) in imp.icon_groups.borrow().iter() {
            let has_match = self.flow_box_has_match(flow_box);
            section.set_visible(has_match);
            any_match |= has_match;
        }
        imp.no_icon_label.set_visible(!any_match);
    }

    fn scroll_to_chosen_icon_when_shown(&self) {
        self.imp().icon_scroller.connect_map(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| {
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    dialog,
                    move || dialog.scroll_to_chosen_icon()
                ));
            }
        ));
    }

    fn scroll_to_chosen_icon(&self) {
        let imp = self.imp();
        let chosen = collection_looks::known_icon_key(&imp.look.borrow());
        let buttons = imp.icon_buttons.borrow();
        let Some((_, button)) = buttons.iter().find(|(key, _)| *key == chosen) else {
            return;
        };
        let Some(bounds) = button.compute_bounds(&*imp.icon_content) else {
            return;
        };
        let adjustment = imp.icon_scroller.vadjustment();
        let centered =
            f64::from(bounds.y()) - (adjustment.page_size() - f64::from(bounds.height())) / 2.0;
        adjustment.set_value(centered);
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
        self.imp().look.borrow_mut().icon = Some(key.to_owned());
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
        let mut chosen_name = collection_looks::default_icon_label();
        for (key, button) in imp.icon_buttons.borrow().iter() {
            button.set_active(*key == chosen_icon);
            if let Some(image) = button.child() {
                set_tint(&image, color_class);
            }
            if *key == chosen_icon {
                chosen_name = button.tooltip_text().unwrap_or_default().to_string();
            }
        }
        imp.icon_name_label.set_label(&chosen_name);
    }

    #[template_callback]
    fn on_icon_search_changed(&self) {
        let imp = self.imp();
        imp.search_query.replace(imp.icon_search.text().to_string());
        for (_, flow_box) in imp.icon_groups.borrow().iter() {
            flow_box.invalidate_filter();
        }
        self.show_search_results();
        if imp.search_query.borrow().is_empty() {
            glib::idle_add_local_once(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                move || dialog.scroll_to_chosen_icon()
            ));
        } else {
            imp.icon_scroller.vadjustment().set_value(0.0);
        }
    }

    #[template_callback]
    fn on_icon_search_activated(&self) {
        let first_match = self
            .imp()
            .icon_buttons
            .borrow()
            .iter()
            .map(|(_, button)| button.clone())
            .find(|button| self.button_matches_search(button));
        if let Some(button) = first_match {
            button.set_active(true);
        }
    }

    #[template_callback]
    fn on_icon_search_stopped(&self) {
        self.imp().icon_search.set_text("");
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

fn icon_flow_box() -> gtk::FlowBox {
    gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(6)
        .max_children_per_line(8)
        .column_spacing(4)
        .row_spacing(4)
        .build()
}

fn icon_section(title: &str, flow_box: &gtk::FlowBox) -> gtk::Box {
    let heading = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .margin_start(4)
        .css_classes(["caption-heading", "dim-label"])
        .build();
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    section.append(&heading);
    section.append(flow_box);
    section
}

pub fn set_tint(widget: &gtk::Widget, color_class: Option<&str>) {
    for class in collection_looks::COLOR_CLASSES {
        widget.remove_css_class(class);
    }
    if let Some(class) = color_class {
        widget.add_css_class(class);
    }
}
