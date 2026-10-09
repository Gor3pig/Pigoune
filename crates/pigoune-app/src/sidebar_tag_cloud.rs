use std::cell::Cell;
use std::rc::Rc;

use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::prelude::*;
use gtk::{gdk, glib};
use pigoune_core::{AssetView, TagId};

use crate::asset_grid::MENU_KEYS;
use crate::drag_content::{DraggedAssets, DraggedTag};
use crate::shortened_label::name_label;
use crate::sidebar::{HoveredDrop, PigouneSidebar};
use crate::sidebar_row::{control_is_held, tag_menu};
use crate::tag_tree;

const MOST_PILLS: usize = 12;
const PILL_SPACING: i32 = 4;
const CHEVRON_PIXELS: i32 = 10;
const SELECTED: &str = "selected";
const DROP_HIGHLIGHT: &str = "drop-highlight";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CloudPlace {
    selected: Option<TagId>,
    level: Option<TagId>,
    show_all: bool,
}

pub type SharedCloudPlace = Rc<Cell<CloudPlace>>;

struct PillParts {
    id: TagId,
    root: gtk::Widget,
    name: gtk::Button,
    chevron: Option<gtk::Button>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPill {
    pub id: TagId,
    pub name: String,
    pub parent: Option<TagId>,
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
        pub crumbs: OnceCell<gtk::Box>,
        pub wrap: OnceCell<adw::WrapBox>,
        pub more: OnceCell<gtk::Button>,
        pub menu: OnceCell<gtk::PopoverMenu>,
        pub tags: RefCell<Vec<TagPill>>,
        pub pills: RefCell<Vec<(TagId, gtk::Button)>>,
        pub chevrons: RefCell<Vec<(TagId, gtk::Button)>>,
        pub crumb_buttons: RefCell<Vec<(TagId, gtk::Button)>>,
        pub selected: Cell<Option<TagId>>,
        pub level: Cell<Option<TagId>>,
        pub show_all: Cell<bool>,
        pub place: RefCell<Option<super::SharedCloudPlace>>,
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
    pub fn show_tags(&self, tags: Vec<TagPill>, selected: Option<TagId>, place: &SharedCloudPlace) {
        let imp = self.imp();
        let remembered = place.get();
        let same_place = imp
            .place
            .borrow()
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, place));
        let unchanged = same_place
            && *imp.tags.borrow() == tags
            && imp.selected.get() == selected
            && imp.level.get() == remembered.level
            && imp.show_all.get() == remembered.show_all;
        if unchanged {
            return;
        }
        imp.place.replace(Some(place.clone()));
        imp.tags.replace(tags);
        imp.level.set(remembered.level);
        imp.show_all.set(remembered.show_all);
        imp.selected.set(selected);
        if remembered.selected != selected || !self.level_is_open() {
            self.follow(selected);
        }
        self.rebuild();
    }

    fn remember_place(&self) {
        let imp = self.imp();
        if let Some(place) = imp.place.borrow().as_ref() {
            place.set(CloudPlace {
                selected: imp.selected.get(),
                level: imp.level.get(),
                show_all: imp.show_all.get(),
            });
        }
    }

    fn level_is_open(&self) -> bool {
        self.imp()
            .level
            .get()
            .is_none_or(|level| self.has_children(level))
    }

    fn has_children(&self, tag: TagId) -> bool {
        self.imp()
            .tags
            .borrow()
            .iter()
            .any(|candidate| candidate.parent == Some(tag))
    }

    pub fn highlight(&self, selected: Option<TagId>) {
        let imp = self.imp();
        imp.selected.set(selected);
        if self.follow(selected) {
            self.rebuild();
        } else {
            self.mark_selected();
            self.remember_place();
        }
    }

    pub fn highlight_in_place(&self, selected: TagId) {
        self.imp().selected.set(Some(selected));
        self.mark_selected();
        self.remember_place();
    }

    fn step_into(&self, tag: TagId) {
        if !self.has_children(tag) {
            return;
        }
        self.imp().level.set(Some(tag));
        self.rebuild();
        let first = self
            .imp()
            .pills
            .borrow()
            .first()
            .map(|(_, pill)| pill.clone());
        if let Some(first) = first {
            first.grab_focus();
        }
    }

    fn step_out(&self) {
        let imp = self.imp();
        let Some(left) = imp.level.get() else {
            return;
        };
        let parent = imp
            .tags
            .borrow()
            .iter()
            .find(|tag| tag.id == left)
            .and_then(|tag| tag.parent);
        imp.level.set(parent);
        self.rebuild();
        let came_from = imp
            .pills
            .borrow()
            .iter()
            .find(|(id, _)| *id == left)
            .map(|(_, pill)| pill.clone());
        if let Some(came_from) = came_from {
            came_from.grab_focus();
        }
    }

    pub fn pill_of(&self, tag: TagId) -> Option<gtk::Button> {
        if let Some(shown) = self.shown_button_of(tag) {
            return Some(shown);
        }
        if !self.knows(tag) {
            return None;
        }
        let imp = self.imp();
        let level = self.level_of(tag);
        if imp.level.get() != level || !imp.show_all.get() {
            imp.level.set(level);
            imp.show_all.set(true);
            self.rebuild();
        }
        self.shown_button_of(tag)
    }

    fn shown_button_of(&self, tag: TagId) -> Option<gtk::Button> {
        let imp = self.imp();
        let in_pills = imp
            .pills
            .borrow()
            .iter()
            .find(|(id, _)| *id == tag)
            .map(|(_, pill)| pill.clone());
        in_pills.or_else(|| {
            imp.crumb_buttons
                .borrow()
                .iter()
                .find(|(id, _)| *id == tag)
                .map(|(_, crumb)| crumb.clone())
        })
    }

    fn knows(&self, tag: TagId) -> bool {
        self.imp().tags.borrow().iter().any(|known| known.id == tag)
    }

    fn level_of(&self, tag: TagId) -> Option<TagId> {
        let tags = self.imp().tags.borrow();
        let known = tags.iter().find(|candidate| candidate.id == tag)?;
        if tags.iter().any(|child| child.parent == Some(tag)) {
            Some(tag)
        } else {
            known.parent
        }
    }

    fn follow(&self, selected: Option<TagId>) -> bool {
        let imp = self.imp();
        let known_level = imp.level.get().filter(|level| self.has_children(*level));
        let wanted = match selected {
            Some(tag) if self.knows(tag) => self.level_of(tag),
            _ => known_level,
        };
        let changed = imp.level.get() != wanted;
        imp.level.set(wanted);
        changed
    }

    fn mark_selected(&self) {
        let selected = self.imp().selected.get();
        for (id, pill) in self.imp().pills.borrow().iter() {
            if Some(*id) == selected {
                pill.add_css_class(SELECTED);
            } else {
                pill.remove_css_class(SELECTED);
            }
        }
        for (id, chevron) in self.imp().chevrons.borrow().iter() {
            if Some(*id) == selected {
                chevron.add_css_class(SELECTED);
            } else {
                chevron.remove_css_class(SELECTED);
            }
        }
        for (id, crumb) in self.imp().crumb_buttons.borrow().iter() {
            if Some(*id) == selected {
                crumb.add_css_class(SELECTED);
            } else {
                crumb.remove_css_class(SELECTED);
            }
        }
    }

    fn rebuild(&self) {
        let imp = self.imp();
        self.rebuild_crumbs();
        let wrap = part(&imp.wrap);
        wrap.remove_all();
        let level = imp.level.get();
        let all = imp.tags.borrow().clone();
        let tags: Vec<TagPill> = all
            .iter()
            .filter(|tag| tag.parent == level)
            .cloned()
            .collect();
        let selected = imp.selected.get();
        let hidden = tags.len().saturating_sub(MOST_PILLS);
        let show_all = imp.show_all.get()
            || hidden == 0
            || tags
                .iter()
                .skip(MOST_PILLS)
                .any(|tag| Some(tag.id) == selected);
        let shown = if show_all { tags.len() } else { MOST_PILLS };
        let mut pills = Vec::new();
        let mut chevrons = Vec::new();
        for tag in tags.iter().take(shown) {
            let has_children = all.iter().any(|child| child.parent == Some(tag.id));
            let parts = self.pill(tag, has_children);
            wrap.append(&parts.root);
            if let Some(chevron) = parts.chevron {
                chevrons.push((parts.id, chevron));
            }
            pills.push((parts.id, parts.name));
        }
        imp.pills.replace(pills);
        imp.chevrons.replace(chevrons);
        self.mark_selected();
        self.remember_place();
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

    fn rebuild_crumbs(&self) {
        let imp = self.imp();
        let crumbs = part(&imp.crumbs);
        while let Some(child) = crumbs.first_child() {
            crumbs.remove(&child);
        }
        imp.crumb_buttons.borrow_mut().clear();
        let Some(level) = imp.level.get() else {
            crumbs.set_visible(false);
            return;
        };
        crumbs.set_visible(true);
        let trail: Vec<(TagId, String)> = tag_tree::branch(&imp.tags.borrow(), level)
            .into_iter()
            .map(|tag| (tag.id, tag.name.clone()))
            .collect();
        crumbs.append(&self.crumb_to_root());
        for (id, name) in trail {
            crumbs.append(
                &gtk::Image::builder()
                    .icon_name("go-next-symbolic")
                    .pixel_size(12)
                    .css_classes(["dim-label"])
                    .build(),
            );
            let crumb = self.crumb(&name, id);
            imp.crumb_buttons.borrow_mut().push((id, crumb.clone()));
            crumbs.append(&crumb);
        }
    }

    fn crumb_to_root(&self) -> gtk::Button {
        let button = gtk::Button::builder()
            .label(gettext("Tags"))
            .css_classes(["flat", "sidebar-tag-crumb"])
            .build();
        button.connect_clicked(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            move |_| {
                cloud.imp().level.set(None);
                cloud.rebuild();
            }
        ));
        self.accept_tag_drops(&button, None);
        button
    }

    fn crumb(&self, name: &str, id: TagId) -> gtk::Button {
        let button = gtk::Button::builder()
            .child(&name_label(name))
            .css_classes(["flat", "sidebar-tag-crumb"])
            .tooltip_text(gettext("Open the Tag “{name}”").replace("{name}", name))
            .build();
        button.connect_clicked(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            move |_| {
                if let Some(sidebar) = cloud.sidebar() {
                    sidebar.choose_tag(id);
                }
            }
        ));
        self.accept_tag_drops(&button, Some(id));
        button
    }

    fn pill(&self, tag: &TagPill, has_children: bool) -> PillParts {
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
        if has_children {
            pill.add_css_class("with-chevron");
        }
        pill.update_property(&[gtk::accessible::Property::Label(&spoken_label(
            tag,
            has_children,
        ))]);
        let id = tag.id;
        pill.connect_clicked(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            move |_| {
                if let Some(sidebar) = cloud.sidebar() {
                    sidebar.choose_tag_in_place(id);
                }
            }
        ));
        self.open_menu_on_secondary_click(&pill, id);
        self.offer_level_keys(&pill, id, has_children);
        Self::rename_on_f2(&pill, id);
        self.accept_drops(&pill, id);
        Self::offer_tag_drag(&pill, id);
        self.accept_tag_drops(&pill, Some(id));
        let chevron = has_children.then(|| self.chevron(tag));
        let root: gtk::Widget = match &chevron {
            Some(chevron) => {
                let group = gtk::Box::builder().build();
                group.append(&pill);
                group.append(chevron);
                group.upcast()
            }
            None => pill.clone().upcast(),
        };
        PillParts {
            id,
            root,
            name: pill,
            chevron,
        }
    }

    fn chevron(&self, tag: &TagPill) -> gtk::Button {
        let label = gettext("Show the Sub-tags of “{name}”").replace("{name}", &tag.name);
        let chevron = gtk::Button::builder()
            .child(
                &gtk::Image::builder()
                    .icon_name("go-next-symbolic")
                    .pixel_size(CHEVRON_PIXELS)
                    .build(),
            )
            .css_classes(["flat", "sidebar-tag-chevron"])
            .tooltip_text(&label)
            .build();
        chevron.update_property(&[gtk::accessible::Property::Label(&label)]);
        let id = tag.id;
        chevron.connect_clicked(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            move |_| cloud.step_into(id)
        ));
        chevron
    }

    fn offer_level_keys(&self, pill: &gtk::Button, id: TagId, has_children: bool) {
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| {
                let only_alt = modifiers.contains(gdk::ModifierType::ALT_MASK)
                    && !modifiers.intersects(
                        gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
                    );
                if !only_alt {
                    return glib::Propagation::Proceed;
                }
                match key {
                    gdk::Key::Right if has_children => {
                        cloud.step_into(id);
                        glib::Propagation::Stop
                    }
                    gdk::Key::Left if cloud.imp().level.get().is_some() => {
                        cloud.step_out();
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            }
        ));
        pill.add_controller(keys);
    }

    fn offer_tag_drag(pill: &gtk::Button, id: TagId) {
        let source = gtk::DragSource::builder()
            .actions(gdk::DragAction::MOVE)
            .build();
        source.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_value(&DraggedTag(id).to_value()))
        });
        source.connect_drag_begin(glib::clone!(
            #[weak]
            pill,
            move |source, _| {
                source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&pill))), 0, 0);
            }
        ));
        pill.add_controller(source);
    }

    fn accept_tag_drops(&self, target_widget: &impl IsA<gtk::Widget>, target: Option<TagId>) {
        let widget = target_widget.as_ref();
        let drop_target = gtk::DropTarget::new(DraggedTag::static_type(), gdk::DragAction::MOVE);
        drop_target.set_preload(true);
        let hover = glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            #[weak]
            widget,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |drop_target: &gtk::DropTarget, _: f64, _: f64| {
                let dragged = drop_target
                    .value()
                    .and_then(|value| value.get::<DraggedTag>().ok());
                if dragged.is_some_and(|dragged| !cloud.can_move_into(dragged.0, target)) {
                    widget.remove_css_class(DROP_HIGHLIGHT);
                    return gdk::DragAction::empty();
                }
                widget.add_css_class(DROP_HIGHLIGHT);
                gdk::DragAction::MOVE
            }
        );
        drop_target.connect_enter(hover.clone());
        drop_target.connect_motion(hover);
        drop_target.connect_leave(glib::clone!(
            #[weak]
            widget,
            move |_| widget.remove_css_class(DROP_HIGHLIGHT)
        ));
        drop_target.connect_drop(glib::clone!(
            #[weak(rename_to = cloud)]
            self,
            #[weak]
            widget,
            #[upgrade_or]
            false,
            move |_, value, _, _| {
                widget.remove_css_class(DROP_HIGHLIGHT);
                let (Some(sidebar), Ok(dragged)) = (cloud.sidebar(), value.get::<DraggedTag>())
                else {
                    return false;
                };
                if !cloud.can_move_into(dragged.0, target) {
                    return false;
                }
                sidebar.tag_dropped(dragged.0, target);
                true
            }
        ));
        widget.add_controller(drop_target);
    }

    fn can_move_into(&self, dragged: TagId, target: Option<TagId>) -> bool {
        let tags = self.imp().tags.borrow();
        let already_there = tag_tree::find(&tags, dragged).is_some_and(|tag| tag.parent == target);
        let inside_itself =
            target.is_some_and(|target| tag_tree::is_within(&tags, target, dragged));
        !already_there && !inside_itself
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
        target.set_types(&[DraggedAssets::static_type()]);
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
        let Ok(dragged) = value.get::<DraggedAssets>() else {
            return false;
        };
        sidebar.assets_dropped(view, dragged.0, control_is_held(pill));
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
        let crumbs = gtk::Box::builder()
            .spacing(2)
            .css_classes(["sidebar-tag-crumbs"])
            .visible(false)
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
        self.append(&crumbs);
        self.append(&wrap);
        self.append(&more);
        set_part(&imp.crumbs, crumbs);
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

fn spoken_label(tag: &TagPill, has_children: bool) -> String {
    let name = if has_children {
        gettext("Tag {name}, with sub-tags")
    } else {
        gettext("Tag {name}")
    }
    .replace("{name}", &tag.name);
    match tag.count {
        Some(count) => ngettext(
            "{name}, {count} asset",
            "{name}, {count} assets",
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
