use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;
use pigoune_core::Tag;

use crate::removable_pill::removable_pill;
use crate::shortened_label::{name_label, naming};
use crate::tag_cloud::PigouneTagCloud;
use crate::tag_editor::{PigouneTagEditor, SharedTag, parent_label};
use crate::tag_input;
use crate::tag_tree;

const EDITOR_WIDTH: i32 = 300;

mod imp {
    use std::cell::OnceCell;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use crate::tag_cloud::PigouneTagCloud;
    use crate::tag_editor::PigouneTagEditor;

    #[derive(Default)]
    pub struct PigouneTagSummary {
        pub cloud: OnceCell<PigouneTagCloud>,
        pub editor: OnceCell<PigouneTagEditor>,
        pub popover: OnceCell<gtk::Popover>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneTagSummary {
        const NAME: &'static str = "PigouneTagSummary";
        type Type = super::PigouneTagSummary;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PigouneTagSummary {
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

    impl WidgetImpl for PigouneTagSummary {}
    impl BoxImpl for PigouneTagSummary {}
}

glib::wrapper! {
    pub struct PigouneTagSummary(ObjectSubclass<imp::PigouneTagSummary>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneTagSummary {
    pub fn editor(&self) -> PigouneTagEditor {
        part(&self.imp().editor)
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the bounds of a button inside the panel fit in an i32"
    )]
    pub fn open_editor(&self) {
        let imp = self.imp();
        let popover = part(&imp.popover);
        if let Some(bounds) = part(&imp.cloud).add_button().compute_bounds(self) {
            popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
                bounds.x() as i32,
                bounds.y() as i32,
                bounds.width() as i32,
                bounds.height() as i32,
            )));
        }
        popover.popup();
        part(&imp.editor).focus_entry();
    }

    pub fn show_tags(&self, current: Vec<SharedTag>, all: Vec<Tag>) {
        let pills = current
            .iter()
            .map(|shared| self.pill(shared, &all))
            .collect();
        part(&self.imp().cloud).set_pills(pills);
        self.editor().show_tags(current, all);
    }

    fn pill(&self, shared: &SharedTag, all: &[Tag]) -> gtk::Widget {
        let tag = &shared.tag;
        let partial = shared.carried_by < shared.out_of;
        let content = gtk::Box::builder().spacing(4).build();
        if let Some(parent) = tag_input::ancestors_of(all, tag).last() {
            content.append(&parent_label(parent));
        }
        content.append(&name_label(&tag.name));
        let pill = gtk::Button::builder()
            .child(&content)
            .css_classes(["flat", "tag-pill"])
            .build();
        if partial {
            content.append(
                &gtk::Label::builder()
                    .label(format!("{}/{}", shared.carried_by, shared.out_of))
                    .css_classes(["dim-label", "numeric"])
                    .build(),
            );
            pill.set_tooltip_text(Some(&naming(
                &tag_tree::path_text(all, tag.id),
                &gettext("On {count} of {total} assets")
                    .replace("{count}", &shared.carried_by.to_string())
                    .replace("{total}", &shared.out_of.to_string()),
            )));
        } else {
            pill.set_tooltip_text(Some(
                &gettext("Open the Tag “{name}”")
                    .replace("{name}", &tag_tree::path_text(all, tag.id)),
            ));
        }
        let id = tag.id;
        let path = tag_input::path_of(all, tag);
        pill.connect_clicked(glib::clone!(
            #[weak(rename_to = summary)]
            self,
            move |_| summary.editor().open_tag(id)
        ));
        let add_tooltip = partial.then(|| {
            gettext("Add the Tag “{name}” to All the Selected Assets").replace("{name}", &tag.name)
        });
        let removable = removable_pill(
            &pill,
            &gettext("Remove the Tag “{name}”").replace("{name}", &tag.name),
            partial,
            add_tooltip.as_deref(),
        );
        if let Some(add) = &removable.add {
            add.connect_clicked(glib::clone!(
                #[weak(rename_to = summary)]
                self,
                #[strong]
                path,
                move |_| summary.editor().add_to_all(&path)
            ));
        }
        removable.remove.connect_clicked(glib::clone!(
            #[weak(rename_to = summary)]
            self,
            move |_| summary.editor().remove(id)
        ));
        removable.pill.upcast()
    }

    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(6);

        let heading = gtk::Label::builder()
            .label(gettext("Tags"))
            .xalign(0.0)
            .css_classes(["caption", "dim-label"])
            .build();
        let cloud = PigouneTagCloud::default();
        let editor = PigouneTagEditor::default();
        editor.set_width_request(EDITOR_WIDTH);
        let popover = gtk::Popover::builder()
            .child(&editor)
            .position(gtk::PositionType::Bottom)
            .build();
        popover.set_parent(self);
        cloud
            .add_button()
            .set_tooltip_text(Some(&gettext("Edit the Tags")));
        for button in [cloud.add_button(), cloud.overflow_button()] {
            button.connect_clicked(glib::clone!(
                #[weak(rename_to = summary)]
                self,
                move |_| summary.open_editor()
            ));
        }

        self.append(&heading);
        self.append(&cloud);
        set_part(&imp.cloud, cloud);
        set_part(&imp.editor, editor);
        set_part(&imp.popover, popover);
    }
}

impl Default for PigouneTagSummary {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn part<Widget: Clone>(cell: &std::cell::OnceCell<Widget>) -> Widget {
    cell.get()
        .cloned()
        .expect("the tag summary builds its parts at construction")
}

fn set_part<Widget>(cell: &std::cell::OnceCell<Widget>, widget: Widget) {
    if cell.set(widget).is_err() {
        unreachable!("the tag summary builds its parts once");
    }
}
