use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, graphene};
use pigoune_core::annotations::LayerMove;

const HANDLE_ICON: &str = "list-drag-handle-symbolic";
const IMAGE_ICON: &str = "image-x-generic-symbolic";
const CROP_ICON: &str = "crop-symbolic";
const SHOWN_ICON: &str = "view-reveal-symbolic";
const HIDDEN_ICON: &str = "view-conceal-symbolic";
const FREE_ICON: &str = "changes-allow-symbolic";
const LOCKED_ICON: &str = "changes-prevent-symbolic";
const SCROLL_MARGIN: f64 = 28.0;
const SCROLL_STEP: f64 = 14.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerAction {
    Select(u32),
    Rename(u32, Option<String>),
    Duplicate(u32),
    Delete(u32),
    Move(u32, LayerMove),
    SetHidden(u32, bool),
    SetLocked(u32, bool),
    RemoveMarker(u32),
    Drop { id: u32, target: u32, above: bool },
    OpenCrop,
}

pub type Dispatch = Rc<dyn Fn(LayerAction)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerRow {
    pub id: u32,
    pub icon: &'static str,
    pub label: String,
    pub hidden: bool,
    pub locked: bool,
    pub sealed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowState {
    pub hidden: bool,
    pub locked: bool,
    pub sealed: bool,
}

impl LayerRow {
    #[must_use]
    pub fn state(&self) -> RowState {
        RowState {
            hidden: self.hidden,
            locked: self.locked,
            sealed: self.sealed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LayersView {
    pub rows: Vec<LayerRow>,
    pub selected: Option<u32>,
    pub image: String,
    pub crop: String,
}

impl LayersView {
    fn same_structure(&self, other: &Self) -> bool {
        self.rows == other.rows && self.image == other.image && self.crop == other.crop
    }
}

pub struct LayersPanel {
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    dispatch: Dispatch,
    ids: RefCell<Vec<Option<u32>>>,
    shown: RefCell<Option<LayersView>>,
    pending: RefCell<Option<LayersView>>,
    labels: RefCell<Vec<(u32, gtk::EditableLabel)>>,
    syncing: Cell<bool>,
    editing: Cell<bool>,
}

pub fn menu_for(id: u32, state: RowState) -> gio::Menu {
    let target = id.to_variant();
    let item = |label: String, action: &str| {
        let item = gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(Some(action), Some(&target));
        item
    };
    let menu = gio::Menu::new();
    if state.sealed {
        let only = gio::Menu::new();
        only.append_item(&item(gettext("Remove Marker"), "layer.marker"));
        menu.append_section(None, &only);
        return menu;
    }
    let first = gio::Menu::new();
    first.append_item(&item(gettext("Rename"), "layer.rename"));
    first.append_item(&item(gettext("Duplicate"), "layer.duplicate"));
    menu.append_section(None, &first);
    let order = gio::Menu::new();
    order.append_item(&item(gettext("Bring to Front"), "layer.front"));
    order.append_item(&item(gettext("Bring Forward"), "layer.forward"));
    order.append_item(&item(gettext("Send Backward"), "layer.backward"));
    order.append_item(&item(gettext("Send to Back"), "layer.back"));
    menu.append_section(None, &order);
    let visibility = gio::Menu::new();
    if state.hidden {
        visibility.append_item(&item(gettext("Show"), "layer.show"));
    } else {
        visibility.append_item(&item(gettext("Hide"), "layer.hide"));
    }
    if state.locked {
        visibility.append_item(&item(gettext("Unlock"), "layer.unlock"));
    } else {
        visibility.append_item(&item(gettext("Lock"), "layer.lock"));
    }
    menu.append_section(None, &visibility);
    let last = gio::Menu::new();
    last.append_item(&item(gettext("Delete"), "layer.delete"));
    menu.append_section(None, &last);
    menu
}

impl LayersPanel {
    pub fn new(list: gtk::ListBox, scroller: gtk::ScrolledWindow, dispatch: Dispatch) -> Rc<Self> {
        let panel = Rc::new(Self {
            list,
            scroller,
            dispatch,
            ids: RefCell::new(Vec::new()),
            shown: RefCell::new(None),
            pending: RefCell::new(None),
            labels: RefCell::new(Vec::new()),
            syncing: Cell::new(false),
            editing: Cell::new(false),
        });
        panel.connect_list();
        panel
    }

    fn connect_list(self: &Rc<Self>) {
        let panel = Rc::downgrade(self);
        self.list.connect_row_selected(move |_, row| {
            let (Some(panel), Some(row)) = (panel.upgrade(), row) else {
                return;
            };
            if panel.syncing.get() {
                return;
            }
            if let Some(id) = panel.id_of(row) {
                (panel.dispatch)(LayerAction::Select(id));
            }
        });
        let panel = Rc::downgrade(self);
        self.list.connect_row_activated(move |_, row| {
            let Some(panel) = panel.upgrade() else {
                return;
            };
            if panel.id_of(row).is_none() {
                (panel.dispatch)(LayerAction::OpenCrop);
            }
        });
    }

    fn id_of(&self, row: &gtk::ListBoxRow) -> Option<u32> {
        let index = usize::try_from(row.index()).ok()?;
        self.ids.borrow().get(index).copied().flatten()
    }

    pub fn focus_inside(&self) -> bool {
        self.list
            .root()
            .and_then(|root| root.focus())
            .is_some_and(|focus| focus.is_ancestor(&self.list))
    }

    pub fn show(self: &Rc<Self>, view: LayersView) {
        if self.editing.get() {
            self.pending.replace(Some(view));
            return;
        }
        let same = self
            .shown
            .borrow()
            .as_ref()
            .is_some_and(|shown| shown.same_structure(&view));
        if same {
            self.select_in_list(view.selected);
            self.shown.replace(Some(view));
            return;
        }
        self.rebuild(&view);
        self.shown.replace(Some(view));
    }

    fn forget(&self) {
        self.shown.replace(None);
    }

    fn rebuild(self: &Rc<Self>, view: &LayersView) {
        self.syncing.set(true);
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.ids.borrow_mut().clear();
        self.labels.borrow_mut().clear();
        if view.rows.is_empty() {
            let row = Self::hint_row(&gettext("No annotations"));
            self.list.append(&row);
            self.ids.borrow_mut().push(None);
        }
        let mut heading_shown = false;
        for layer in &view.rows {
            if layer.sealed && !heading_shown {
                heading_shown = true;
                self.list
                    .append(&Self::heading_row(&gettext("Applied Masks")));
                self.ids.borrow_mut().push(None);
            }
            let row = self.build_row(layer);
            self.list.append(&row);
            self.ids.borrow_mut().push(Some(layer.id));
        }
        let background = Self::fixed_row(IMAGE_ICON, &gettext("Image"), &view.image, false);
        self.list.append(&background);
        self.ids.borrow_mut().push(None);
        let crop = Self::fixed_row(CROP_ICON, &gettext("Crop"), &view.crop, true);
        self.list.append(&crop);
        self.ids.borrow_mut().push(None);
        self.syncing.set(false);
        self.select_in_list(view.selected);
    }

    fn hint_row(text: &str) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        let label = gtk::Label::new(Some(text));
        label.add_css_class("dim-label");
        label.set_margin_top(12);
        label.set_margin_bottom(12);
        row.set_child(Some(&label));
        row
    }

    fn heading_row(text: &str) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        let label = gtk::Label::new(Some(text));
        label.add_css_class("dim-label");
        label.add_css_class("caption-heading");
        label.set_halign(gtk::Align::Start);
        label.set_margin_top(8);
        label.set_margin_bottom(4);
        label.set_margin_start(10);
        row.set_child(Some(&label));
        row
    }

    fn fixed_row(icon: &str, title: &str, detail: &str, activatable: bool) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(activatable);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.set_margin_top(8);
        content.set_margin_bottom(8);
        content.set_margin_start(10);
        content.set_margin_end(10);
        let icon = gtk::Image::from_icon_name(icon);
        icon.add_css_class("dim-label");
        let title = gtk::Label::new(Some(title));
        title.set_halign(gtk::Align::Start);
        let detail = gtk::Label::new(Some(detail));
        detail.add_css_class("dim-label");
        detail.set_hexpand(true);
        detail.set_halign(gtk::Align::End);
        detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        content.append(&icon);
        content.append(&title);
        content.append(&detail);
        row.set_child(Some(&content));
        row
    }

    fn select_in_list(&self, selected: Option<u32>) {
        self.syncing.set(true);
        let index = selected.and_then(|id| {
            self.ids
                .borrow()
                .iter()
                .position(|candidate| *candidate == Some(id))
        });
        match index.and_then(|index| i32::try_from(index).ok()) {
            Some(index) => {
                if let Some(row) = self.list.row_at_index(index) {
                    if !row.is_selected() {
                        self.list.select_row(Some(&row));
                    }
                    self.reveal(&row);
                }
            }
            None => self.list.unselect_all(),
        }
        self.syncing.set(false);
    }

    fn reveal(&self, row: &gtk::ListBoxRow) {
        let list = self.list.clone();
        let scroller = self.scroller.clone();
        let row = row.clone();
        glib::idle_add_local_once(move || {
            let Some(bounds) = row.compute_bounds(&list) else {
                return;
            };
            let adjustment = scroller.vadjustment();
            let top = f64::from(bounds.y());
            let bottom = top + f64::from(bounds.height());
            if top < adjustment.value() {
                adjustment.set_value(top);
            } else if bottom > adjustment.value() + adjustment.page_size() {
                adjustment.set_value(bottom - adjustment.page_size());
            }
        });
    }

    pub fn start_renaming(&self, id: u32) {
        let label = self
            .labels
            .borrow()
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .map(|(_, label)| label.clone());
        if let Some(label) = label {
            label.start_editing();
        }
    }

    fn build_row(self: &Rc<Self>, layer: &LayerRow) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.set_margin_top(4);
        content.set_margin_bottom(4);
        content.set_margin_start(6);
        content.set_margin_end(6);
        let handle = gtk::Image::from_icon_name(HANDLE_ICON);
        handle.add_css_class("dim-label");
        let movable = !layer.locked && !layer.sealed;
        if movable {
            handle.set_cursor_from_name(Some("grab"));
            handle.set_tooltip_text(Some(&gettext("Drag to Reorder")));
        } else {
            handle.set_opacity(0.25);
        }
        let icon = gtk::Image::from_icon_name(layer.icon);
        let label = gtk::EditableLabel::new(&layer.label);
        label.set_hexpand(true);
        label.set_halign(gtk::Align::Fill);
        label.set_sensitive(!layer.sealed);
        if layer.hidden {
            icon.set_opacity(0.4);
            label.set_opacity(0.4);
        }
        content.append(&handle);
        content.append(&icon);
        content.append(&label);
        if layer.sealed {
            let applied = gtk::Image::from_icon_name(LOCKED_ICON);
            applied.set_tooltip_text(Some(&gettext("Masking applied")));
            applied.add_css_class("dim-label");
            content.append(&applied);
        } else {
            content.append(&self.toggle_button(
                layer.id,
                layer.hidden,
                (HIDDEN_ICON, SHOWN_ICON),
                (gettext("Show"), gettext("Hide")),
                LayerAction::SetHidden,
            ));
            content.append(&self.toggle_button(
                layer.id,
                layer.locked,
                (LOCKED_ICON, FREE_ICON),
                (gettext("Unlock"), gettext("Lock")),
                LayerAction::SetLocked,
            ));
        }
        row.set_child(Some(&content));
        self.labels.borrow_mut().push((layer.id, label.clone()));
        self.watch_renaming(&label, layer);
        self.attach_menu(&row, layer.id, layer.state());
        if movable {
            Self::attach_drag(&row, &handle, layer.id);
        }
        if !layer.sealed {
            self.attach_drop(&row, layer.id);
        }
        row
    }

    fn toggle_button(
        &self,
        id: u32,
        on: bool,
        (on_icon, off_icon): (&str, &str),
        (on_tip, off_tip): (String, String),
        make: fn(u32, bool) -> LayerAction,
    ) -> gtk::Button {
        let button = gtk::Button::from_icon_name(if on { on_icon } else { off_icon });
        button.add_css_class("flat");
        button.add_css_class("circular");
        button.set_valign(gtk::Align::Center);
        button.set_focus_on_click(false);
        button.set_tooltip_text(Some(if on { &on_tip } else { &off_tip }));
        let dispatch = self.dispatch.clone();
        button.connect_clicked(move |_| dispatch(make(id, !on)));
        button
    }

    fn watch_renaming(self: &Rc<Self>, label: &gtk::EditableLabel, layer: &LayerRow) {
        let panel = Rc::downgrade(self);
        let (id, original) = (layer.id, layer.label.clone());
        label.connect_editing_notify(move |label| {
            let Some(panel) = panel.upgrade() else {
                return;
            };
            if label.is_editing() {
                panel.editing.set(true);
                return;
            }
            panel.editing.set(false);
            let text = label.text().trim().to_owned();
            if text != original {
                panel.forget();
                (panel.dispatch)(LayerAction::Rename(id, (!text.is_empty()).then_some(text)));
            }
            let pending = panel.pending.take();
            if let Some(view) = pending {
                panel.forget();
                panel.show(view);
            }
        });
    }

    fn attach_menu(&self, row: &gtk::ListBoxRow, id: u32, state: RowState) {
        let popover = gtk::PopoverMenu::from_model(Some(&menu_for(id, state)));
        popover.set_parent(row);
        popover.set_has_arrow(false);
        popover.set_halign(gtk::Align::Start);
        let unparent = popover.clone();
        row.connect_destroy(move |_| unparent.unparent());
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let list = self.list.clone();
        click.connect_pressed(glib::clone!(
            #[weak]
            row,
            move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if !row.is_selected() {
                    list.select_row(Some(&row));
                }
                popover.set_pointing_to(Some(&gdk::Rectangle::new(
                    whole_pixels(x),
                    whole_pixels(y),
                    1,
                    1,
                )));
                popover.popup();
            }
        ));
        row.add_controller(click);
    }

    fn attach_drag(row: &gtk::ListBoxRow, handle: &gtk::Image, id: u32) {
        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::MOVE);
        source
            .connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&id.to_value())));
        source.connect_drag_begin(glib::clone!(
            #[weak]
            row,
            move |source, _| {
                let paintable = gtk::WidgetPaintable::new(Some(&row));
                source.set_icon(Some(&paintable), 12, 12);
            }
        ));
        handle.add_controller(source);
    }

    fn attach_drop(&self, row: &gtk::ListBoxRow, target: u32) {
        let drop = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
        let scroller = self.scroller.clone();
        drop.connect_motion(glib::clone!(
            #[weak]
            row,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |_, _, y| {
                let above = y < f64::from(row.height()) / 2.0;
                mark(&row, Some(above));
                autoscroll(&scroller, &row, y);
                gdk::DragAction::MOVE
            }
        ));
        drop.connect_leave(glib::clone!(
            #[weak]
            row,
            move |_| mark(&row, None)
        ));
        let dispatch = self.dispatch.clone();
        drop.connect_drop(glib::clone!(
            #[weak]
            row,
            #[upgrade_or]
            false,
            move |_, value, _, y| {
                mark(&row, None);
                let Ok(id) = value.get::<u32>() else {
                    return false;
                };
                let above = y < f64::from(row.height()) / 2.0;
                dispatch(LayerAction::Drop { id, target, above });
                true
            }
        ));
        row.add_controller(drop);
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "pointer coordinates fit easily in 32 bits"
)]
fn whole_pixels(value: f64) -> i32 {
    value.round() as i32
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "pointer coordinates fit easily in 32 bits"
)]
fn single(value: f64) -> f32 {
    value as f32
}

fn mark(row: &gtk::ListBoxRow, above: Option<bool>) {
    row.remove_css_class("layer-drop-above");
    row.remove_css_class("layer-drop-below");
    match above {
        Some(true) => row.add_css_class("layer-drop-above"),
        Some(false) => row.add_css_class("layer-drop-below"),
        None => {}
    }
}

fn autoscroll(scroller: &gtk::ScrolledWindow, row: &gtk::ListBoxRow, y: f64) {
    let Some(point) = row.compute_point(scroller, &graphene::Point::new(0.0, single(y))) else {
        return;
    };
    let position = f64::from(point.y());
    let adjustment = scroller.vadjustment();
    if position < SCROLL_MARGIN {
        adjustment.set_value(adjustment.value() - SCROLL_STEP);
    } else if position > f64::from(scroller.height()) - SCROLL_MARGIN {
        adjustment.set_value(adjustment.value() + SCROLL_STEP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const ACTIONS: [&str; 13] = [
        "marker",
        "hide",
        "show",
        "lock",
        "unlock",
        "select",
        "rename",
        "duplicate",
        "delete",
        "front",
        "forward",
        "backward",
        "back",
    ];

    fn view(labels: &[&str], selected: Option<u32>) -> LayersView {
        LayersView {
            rows: labels
                .iter()
                .enumerate()
                .map(|(index, label)| LayerRow {
                    id: u32::try_from(index).expect("small") + 1,
                    icon: "rectangle-symbolic",
                    label: (*label).to_owned(),
                    hidden: false,
                    locked: false,
                    sealed: false,
                })
                .collect(),
            selected,
            image: "1200 x 800".to_owned(),
            crop: "Not cropped".to_owned(),
        }
    }

    #[test]
    fn a_change_of_selection_alone_does_not_rebuild_the_list() {
        let first = view(&["Rectangle 1", "Arrow 1"], Some(1));
        let second = view(&["Rectangle 1", "Arrow 1"], Some(2));
        assert!(first.same_structure(&second));
    }

    #[test]
    fn a_new_name_a_new_order_or_a_new_crop_rebuilds_the_list() {
        let base = view(&["Rectangle 1", "Arrow 1"], None);
        assert!(!base.same_structure(&view(&["Titre", "Arrow 1"], None)));
        assert!(!base.same_structure(&view(&["Arrow 1", "Rectangle 1"], None)));
        let mut cropped = base.clone();
        cropped.crop = "800 x 600".to_owned();
        assert!(!base.same_structure(&cropped));
    }

    #[test]
    fn every_action_of_the_menu_is_one_the_editor_registers() {
        let menu = menu_for(7, RowState::default());
        let mut seen = Vec::new();
        for section in 0..menu.n_items() {
            let Some(link) = menu.item_link(section, "section") else {
                continue;
            };
            for item in 0..link.n_items() {
                let action = link
                    .item_attribute_value(item, "action", None)
                    .and_then(|value| value.get::<String>())
                    .expect("an action");
                let name = action.strip_prefix("layer.").expect("the layer group");
                assert!(ACTIONS.contains(&name), "{name}");
                let target = link
                    .item_attribute_value(item, "target", None)
                    .and_then(|value| value.get::<u32>());
                assert_eq!(target, Some(7), "{name} acts on the row it was opened on");
                seen.push(name.to_owned());
            }
        }
        assert_eq!(seen.len(), 9);
    }
}
