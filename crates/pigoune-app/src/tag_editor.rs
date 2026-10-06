use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, glib};
use pigoune_core::{Tag, TagId};

use crate::shortened_label::{name_label, naming};
use crate::tag_input;

type AddedCallback = Box<dyn Fn(Vec<String>)>;
type OpenedCallback = Box<dyn Fn(TagId)>;
type AppliedCallback = Box<dyn Fn(String)>;

pub struct SharedTag {
    pub tag: Tag,
    pub carried_by: usize,
    pub out_of: usize,
}
type RemovedCallback = Box<dyn Fn(TagId)>;

mod imp {
    use std::cell::{OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;
    use pigoune_core::Tag;

    use super::{AddedCallback, AppliedCallback, OpenedCallback, RemovedCallback};

    #[derive(Default)]
    pub struct PigouneTagEditor {
        pub chips: OnceCell<gtk::FlowBox>,
        pub entry: OnceCell<gtk::Entry>,
        pub entry_child: OnceCell<gtk::FlowBoxChild>,
        pub popover: OnceCell<gtk::Popover>,
        pub suggestions: OnceCell<gtk::ListBox>,
        pub current: RefCell<Vec<Tag>>,
        pub all: RefCell<Vec<Tag>>,
        pub on_added: RefCell<Option<AddedCallback>>,
        pub on_removed: RefCell<Option<RemovedCallback>>,
        pub on_opened: RefCell<Option<OpenedCallback>>,
        pub on_applied: RefCell<Option<AppliedCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneTagEditor {
        const NAME: &'static str = "PigouneTagEditor";
        type Type = super::PigouneTagEditor;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PigouneTagEditor {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }

        fn dispose(&self) {
            if let Some(popover) = self.popover.get() {
                popover.unparent();
            }
        }
    }

    impl WidgetImpl for PigouneTagEditor {}
    impl BoxImpl for PigouneTagEditor {}
}

glib::wrapper! {
    pub struct PigouneTagEditor(ObjectSubclass<imp::PigouneTagEditor>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneTagEditor {
    pub fn connect_added(&self, callback: impl Fn(Vec<String>) + 'static) {
        self.imp().on_added.replace(Some(Box::new(callback)));
    }

    pub fn connect_removed(&self, callback: impl Fn(TagId) + 'static) {
        self.imp().on_removed.replace(Some(Box::new(callback)));
    }

    pub fn connect_opened(&self, callback: impl Fn(TagId) + 'static) {
        self.imp().on_opened.replace(Some(Box::new(callback)));
    }

    pub fn connect_applied(&self, callback: impl Fn(String) + 'static) {
        self.imp().on_applied.replace(Some(Box::new(callback)));
    }

    pub fn focus_entry(&self) {
        part(&self.imp().entry).grab_focus();
    }

    pub fn show_tags(&self, current: Vec<SharedTag>, all: Vec<Tag>) {
        let imp = self.imp();
        let chips = part(&imp.chips);
        let entry_child = part(&imp.entry_child);
        while let Some(child) = chips.first_child() {
            if child == entry_child.clone().upcast::<gtk::Widget>() {
                break;
            }
            chips.remove(&child);
        }
        for (position, shared) in current.iter().enumerate() {
            let chip = gtk::FlowBoxChild::builder()
                .child(&self.chip(shared))
                .focusable(false)
                .build();
            chips.insert(&chip, i32::try_from(position).unwrap_or(-1));
        }
        imp.current
            .replace(current.into_iter().map(|shared| shared.tag).collect());
        imp.all.replace(all);
        self.refresh_suggestions();
    }

    fn chip(&self, shared: &SharedTag) -> gtk::Box {
        let tag = &shared.tag;
        let partial = shared.carried_by < shared.out_of;
        let chip = gtk::Box::builder().css_classes(["tag-chip"]).build();
        let content = gtk::Box::builder().spacing(4).build();
        content.append(&name_label(&tag.name));
        let open = gtk::Button::builder()
            .child(&content)
            .css_classes(["flat", "tag-chip-label"])
            .build();
        if partial {
            chip.add_css_class("partial");
            content.append(
                &gtk::Label::builder()
                    .label(format!("{}/{}", shared.carried_by, shared.out_of))
                    .css_classes(["caption", "dim-label", "numeric"])
                    .build(),
            );
            open.set_tooltip_text(Some(&naming(
                &tag.name,
                &gettext("On {count} of {total} resources. Click to add it to all of them.")
                    .replace("{count}", &shared.carried_by.to_string())
                    .replace("{total}", &shared.out_of.to_string()),
            )));
        } else {
            open.set_tooltip_text(Some(
                &gettext("Open the Tag “{name}”").replace("{name}", &tag.name),
            ));
        }
        let id = tag.id;
        let name = tag.name.clone();
        open.connect_clicked(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.activate_tag(id, &name, partial)
        ));
        let remove = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text(gettext("Remove the Tag “{name}”").replace("{name}", &tag.name))
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular", "tag-chip-remove"])
            .build();
        remove.connect_clicked(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.remove(id)
        ));
        chip.append(&open);
        chip.append(&remove);
        chip
    }

    pub fn activate_tag(&self, id: TagId, name: &str, partial: bool) {
        let imp = self.imp();
        if partial {
            if let Some(on_applied) = imp.on_applied.borrow().as_ref() {
                on_applied(name.to_owned());
            }
        } else if let Some(on_opened) = imp.on_opened.borrow().as_ref() {
            on_opened(id);
        }
    }

    pub fn remove(&self, id: TagId) {
        if let Some(on_removed) = self.imp().on_removed.borrow().as_ref() {
            on_removed(id);
        }
    }

    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(6);

        let chips = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .column_spacing(4)
            .row_spacing(4)
            .max_children_per_line(30)
            .homogeneous(false)
            .build();
        let entry = gtk::Entry::builder()
            .placeholder_text(gettext("Add a Tag…"))
            .has_frame(false)
            .width_chars(8)
            .hexpand(true)
            .css_classes(["tag-field-entry"])
            .build();
        let entry_child = gtk::FlowBoxChild::builder()
            .child(&entry)
            .focusable(false)
            .build();
        chips.append(&entry_child);
        let field = gtk::Box::builder().css_classes(["tag-field"]).build();
        field.append(&chips);
        chips.set_hexpand(true);
        let suggestions = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["navigation-sidebar"])
            .build();
        let popover = gtk::Popover::builder()
            .child(&suggestions)
            .autohide(false)
            .has_arrow(false)
            .position(gtk::PositionType::Bottom)
            .build();
        popover.set_parent(&entry);

        self.append(&field);
        set_part(&imp.chips, chips);
        set_part(&imp.entry, entry.clone());
        set_part(&imp.entry_child, entry_child);
        set_part(&imp.popover, popover.clone());
        set_part(&imp.suggestions, suggestions.clone());

        let click = gtk::GestureClick::new();
        click.connect_released(glib::clone!(
            #[weak]
            entry,
            move |_, _, _, _| {
                entry.grab_focus();
            }
        ));
        field.add_controller(click);

        entry.connect_changed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.on_text_changed()
        ));
        entry.connect_activate(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.submit()
        ));
        suggestions.connect_row_activated(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, row| editor.choose(row)
        ));

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| editor.on_entry_key(key)
        ));
        entry.add_controller(keys);

        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| {
                let popover = part(&editor.imp().popover);
                if !popover.has_focus() && popover.focus_child().is_none() {
                    popover.popdown();
                }
            }
        ));
        entry.add_controller(focus);
    }

    fn on_text_changed(&self) {
        let entry = part(&self.imp().entry);
        let text = entry.text();
        let (finished, rest) = tag_input::split_finished(&text);
        if rest != text.as_str() {
            entry.set_text(&rest);
            entry.set_position(-1);
            if !finished.is_empty()
                && let Some(on_added) = self.imp().on_added.borrow().as_ref()
            {
                on_added(finished);
            }
        }
        self.refresh_suggestions();
    }

    fn on_entry_key(&self, key: gdk::Key) -> glib::Propagation {
        let imp = self.imp();
        let popover = part(&imp.popover);
        match key {
            gdk::Key::Down if popover.is_visible() => {
                if let Some(first) = part(&imp.suggestions).row_at_index(0) {
                    first.grab_focus();
                }
                glib::Propagation::Stop
            }
            gdk::Key::Escape if popover.is_visible() => {
                popover.popdown();
                glib::Propagation::Stop
            }
            gdk::Key::BackSpace if part(&imp.entry).text().is_empty() => {
                let last = imp.current.borrow().last().map(|tag| tag.id);
                if let Some(last) = last {
                    self.remove(last);
                }
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    fn refresh_suggestions(&self) {
        let imp = self.imp();
        let entry = part(&imp.entry);
        let list = part(&imp.suggestions);
        let popover = part(&imp.popover);
        list.remove_all();
        let current_ids: Vec<TagId> = imp.current.borrow().iter().map(|tag| tag.id).collect();
        let text = entry.text();
        let all = imp.all.borrow();
        let found =
            tag_input::suggestions(&all, tag_input::fragment_being_typed(&text), &current_ids);
        for tag in &found {
            let label = name_label(&tag.name);
            label.set_xalign(0.0);
            list.append(&label);
        }
        if found.is_empty() || !entry.has_focus() && entry.focus_child().is_none() {
            popover.popdown();
        } else {
            popover.popup();
        }
    }

    fn choose(&self, row: &gtk::ListBoxRow) {
        let Some(name) = row
            .child()
            .and_downcast::<gtk::Label>()
            .map(|label| label.label())
        else {
            return;
        };
        let imp = self.imp();
        let entry = part(&imp.entry);
        part(&imp.popover).popdown();
        entry.set_text("");
        if let Some(on_added) = imp.on_added.borrow().as_ref() {
            on_added(vec![name.to_string()]);
        }
        entry.grab_focus();
    }

    fn submit(&self) {
        let imp = self.imp();
        let entry = part(&imp.entry);
        let names = tag_input::names_in(&entry.text());
        part(&imp.popover).popdown();
        if names.is_empty() {
            return;
        }
        entry.set_text("");
        if let Some(on_added) = imp.on_added.borrow().as_ref() {
            on_added(names);
        }
    }
}

impl Default for PigouneTagEditor {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn part<Widget: Clone>(cell: &std::cell::OnceCell<Widget>) -> Widget {
    cell.get()
        .cloned()
        .expect("the tag editor builds its parts at construction")
}

fn set_part<Widget>(cell: &std::cell::OnceCell<Widget>, widget: Widget) {
    if cell.set(widget).is_err() {
        unreachable!("the tag editor builds its parts once");
    }
}
