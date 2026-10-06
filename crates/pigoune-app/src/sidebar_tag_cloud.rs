use std::path::PathBuf;

use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{AssetView, TagId};

use crate::asset_grid::MENU_KEYS;
use crate::drag_content::DraggedAssets;
use crate::shortened_label::name_label;
use crate::sidebar::{HoveredDrop, PigouneSidebar};
use crate::sidebar_row::{control_is_held, tag_menu};

const MOST_PILLS: usize = 12;
const PILL_SPACING: i32 = 4;
const SELECTED: &str = "selected";
const DROP_HIGHLIGHT: &str = "drop-highlight";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPill {
    pub id: TagId,
    pub name: String,
    pub count: Option<usize>,
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;
    use pigoune_core::TagId;

    use super::TagPill;

    #[derive(Default)]
    pub struct PigouneSidebarTagCloud {
        pub wrap: OnceCell<adw::WrapBox>,
        pub more: OnceCell<gtk::Button>,
        pub menu: OnceCell<gtk::PopoverMenu>,
        pub tags: RefCell<Vec<TagPill>>,
        pub pills: RefCell<Vec<(TagId, gtk::Button)>>,
        pub selected: Cell<Option<TagId>>,
        pub show_all: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneSidebarTagCloud {
        const NAME: &'static str = "PigouneSidebarTagCloud";
        type Type = super::PigouneSidebarTagCloud;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PigouneSidebarTagCloud {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }

        fn dispose(&self) {
            if let Some(menu) = self.menu.get() {
                menu.unparent();
            }
        }
    }

    impl WidgetImpl for PigouneSidebarTagCloud {}
    impl BoxImpl for PigouneSidebarTagCloud {}
}

glib::wrapper! {
    pub struct PigouneSidebarTagCloud(ObjectSubclass<imp::PigouneSidebarTagCloud>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PigouneSidebarTagCloud {
    pub fn show_tags(&self, tags: Vec<TagPill>, selected: Option<TagId>) {
        let imp = self.imp();
        let unchanged = *imp.tags.borrow() == tags && imp.selected.get() == selected;
        if unchanged {
            return;
        }
        imp.tags.replace(tags);
        imp.selected.set(selected);
        self.rebuild();
    }

    pub fn highlight(&self, selected: Option<TagId>) {
        self.imp().selected.set(selected);
        for (id, pill) in self.imp().pills.borrow().iter() {
            if Some(*id) == selected {
                pill.add_css_class(SELECTED);
            } else {
                pill.remove_css_class(SELECTED);
            }
        }
    }

    pub fn pill_of(&self, tag: TagId) -> Option<gtk::Button> {
        if !self.imp().pills.borrow().iter().any(|(id, _)| *id == tag) {
            self.imp().show_all.set(true);
            self.rebuild();
        }
        self.imp()
            .pills
            .borrow()
            .iter()
            .find(|(id, _)| *id == tag)
            .map(|(_, pill)| pill.clone())
    }

    fn rebuild(&self) {
        let imp = self.imp();
        let wrap = part(&imp.wrap);
        wrap.remove_all();
        let tags = imp.tags.borrow().clone();
        let selected = imp.selected.get();
        let hidden = tags.len().saturating_sub(MOST_PILLS);
        let show_all = imp.show_all.get()
            || hidden == 0
            || tags
                .iter()
                .skip(MOST_PILLS)
                .any(|tag| Some(tag.id) == selected);
        let shown = if show_all { tags.len() } else { MOST_PILLS };
        let pills: Vec<(TagId, gtk::Button)> = tags
            .iter()
            .take(shown)
            .map(|tag| (tag.id, self.pill(tag)))
            .collect();
        for (_, pill) in &pills {
            wrap.append(pill);
        }
        imp.pills.replace(pills);
        self.highlight(selected);
        let more = part(&imp.more);
        more.set_visible(hidden > 0);
        more.set_label(&if show_all {
            gettext("Show Fewer")
        } else {
            ngettext(
                "+ {count} other",
                "+ {count} others",
                count_for_plural(hidden),
            )
            .replace("{count}", &hidden.to_string())
        });
    }

    fn pill(&self, tag: &TagPill) -> gtk::Button {
        let content = gtk::Box::builder().spacing(6).build();
        content.append(&name_label(&tag.name));
        if let Some(count) = tag.count.filter(|count| *count > 0) {
            content.append(
                &gtk::Label::builder()
                    .label(count.to_string())
                    .css_classes(["dim-label", "numeric", "caption"])
                    .build(),
            );
        }
        let pill = gtk::Button::builder()
            .child(&content)
            .css_classes(["flat", "sidebar-tag-pill"])
            .tooltip_text(gettext("Open the Tag “{name}”").replace("{name}", &tag.name))
            .build();
        pill.update_property(&[gtk::accessible::Property::Label(&spoken_label(tag))]);
        let id = tag.id;
        pill.connect_clicked(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            move |_| {
                if let Some(sidebar) = cloud.sidebar() {
                    sidebar.choose_tag(id);
                }
            }
        ));
        self.open_menu_on_secondary_click(&pill, id);
        Self::rename_on_f2(&pill, id);
        self.accept_drops(&pill, id);
        pill
    }

    fn open_menu_on_secondary_click(&self, pill: &gtk::Button, id: TagId) {
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        click.connect_pressed(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            #[weak]
            pill,
            move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                cloud.show_menu(&pill, id);
            }
        ));
        pill.add_controller(click);
        let keys = gtk::ShortcutController::new();
        keys.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string(MENU_KEYS),
            Some(gtk::CallbackAction::new(glib::clone!(
                #[weak(rename_to = cloud)]
                self,
                #[weak]
                pill,
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_, _| {
                    cloud.show_menu(&pill, id);
                    glib::Propagation::Stop
                }
            ))),
        ));
        pill.add_controller(keys);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the bounds of a pill inside the sidebar fit in an i32"
    )]
    fn show_menu(&self, pill: &gtk::Button, id: TagId) {
        let menu = part(&self.imp().menu);
        menu.set_menu_model(Some(&tag_menu(&id.to_string())));
        if let Some(bounds) = pill.compute_bounds(self) {
            menu.set_pointing_to(Some(&gdk::Rectangle::new(
                bounds.x() as i32,
                bounds.y() as i32,
                bounds.width() as i32,
                bounds.height() as i32,
            )));
        }
        menu.popup();
    }

    fn rename_on_f2(pill: &gtk::Button, id: TagId) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak]
            pill,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                if key != gdk::Key::F2 || !modifiers.is_empty() {
                    return glib::Propagation::Proceed;
                }
                let _ = pill.activate_action("win.rename-tag", Some(&id.to_string().to_variant()));
                glib::Propagation::Stop
            }
        ));
        pill.add_controller(keys);
    }

    fn accept_drops(&self, pill: &gtk::Button, id: TagId) {
        let view = AssetView::Tag(id);
        let target = gtk::DropTarget::new(glib::Type::INVALID, gdk::DragAction::COPY);
        target.set_preload(true);
        target.set_types(&[DraggedAssets::static_type(), gdk::FileList::static_type()]);
        let hover = glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            #[weak]
            pill,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |target: &gtk::DropTarget, _: f64, _: f64| {
                pill.add_css_class(DROP_HIGHLIGHT);
                let dragged = target
                    .value()
                    .and_then(|value| value.get::<DraggedAssets>().ok());
                if let (Some(sidebar), Some(dragged)) = (cloud.sidebar(), dragged) {
                    sidebar.assets_hovered(Some(HoveredDrop {
                        view,
                        assets: dragged.0,
                        keep_source: control_is_held(&pill),
                    }));
                }
                gdk::DragAction::COPY
            }
        );
        target.connect_enter(hover.clone());
        target.connect_motion(hover);
        target.connect_leave(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            #[weak]
            pill,
            move |_| cloud.forget_hover(&pill)
        ));
        target.connect_drop(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            #[weak]
            pill,
            #[upgrade_or]
            false,
            move |_, value, _, _| {
                cloud.forget_hover(&pill);
                cloud.receive(&pill, view, value)
            }
        ));
        pill.add_controller(target);
    }

    fn forget_hover(&self, pill: &gtk::Button) {
        pill.remove_css_class(DROP_HIGHLIGHT);
        if let Some(sidebar) = self.sidebar() {
            sidebar.assets_hovered(None);
        }
    }

    fn receive(&self, pill: &gtk::Button, view: AssetView, value: &glib::Value) -> bool {
        let Some(sidebar) = self.sidebar() else {
            return false;
        };
        if let Ok(dragged) = value.get::<DraggedAssets>() {
            sidebar.assets_dropped(view, dragged.0, control_is_held(pill));
            return true;
        }
        let Ok(files) = value.get::<gdk::FileList>() else {
            return false;
        };
        let paths: Vec<PathBuf> = files.files().iter().filter_map(gio::File::path).collect();
        sidebar.files_dropped(view, paths);
        true
    }

    fn sidebar(&self) -> Option<PigouneSidebar> {
        self.ancestor(PigouneSidebar::static_type())
            .and_downcast::<PigouneSidebar>()
    }

    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(PILL_SPACING);
        self.add_css_class("sidebar-tag-cloud");
        let wrap = adw::WrapBox::builder()
            .child_spacing(PILL_SPACING)
            .line_spacing(PILL_SPACING)
            .build();
        let more = gtk::Button::builder()
            .halign(gtk::Align::Start)
            .css_classes(["flat", "sidebar-tag-more"])
            .visible(false)
            .build();
        more.connect_clicked(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            move |_| {
                let imp = cloud.imp();
                imp.show_all.set(!imp.show_all.get());
                cloud.rebuild();
            }
        ));
        let menu = gtk::PopoverMenu::builder().has_arrow(false).build();
        menu.set_parent(self);
        self.append(&wrap);
        self.append(&more);
        set_part(&imp.wrap, wrap);
        set_part(&imp.more, more);
        set_part(&imp.menu, menu);
    }
}

impl Default for PigouneSidebarTagCloud {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn spoken_label(tag: &TagPill) -> String {
    let name = gettext("Tag {name}").replace("{name}", &tag.name);
    match tag.count {
        Some(count) => ngettext(
            "{name}, {count} resource",
            "{name}, {count} resources",
            count_for_plural(count),
        )
        .replace("{name}", &name)
        .replace("{count}", &count.to_string()),
        None => name,
    }
}

fn count_for_plural(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn part<Widget: Clone>(cell: &std::cell::OnceCell<Widget>) -> Widget {
    cell.get()
        .cloned()
        .expect("the tag cloud builds its parts at construction")
}

fn set_part<Widget>(cell: &std::cell::OnceCell<Widget>, widget: Widget) {
    if cell.set(widget).is_err() {
        unreachable!("the tag cloud builds its parts once");
    }
}
