use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{
    AssetId, ChangeStamp, ConfirmedRemoval, DuplicateKind, DuplicatePlan, DuplicateProgress,
};

use crate::health_page::{Refusal, Replaced};
use crate::row_thumbnail;
use crate::thumbnails::ThumbnailCache;
use crate::toasts;

const GROUPS_SHOWN: usize = 20;
const MEMBERS_SHOWN: usize = 6;
const MORE_MEMBERS_SHOWN: usize = 20;
const THUMBNAIL_MARGIN: f64 = 300.0;

#[derive(Clone)]
pub struct DuplicateMember {
    pub id: AssetId,
    pub name: String,
    pub summary: String,
    pub file: PathBuf,
    pub thumbnail_file: PathBuf,
}

pub struct PlannedSearch {
    pub plan: DuplicatePlan,
    pub pending: u64,
}

pub struct WaitingThumbnail {
    picture: glib::WeakRef<gtk::Picture>,
    member: DuplicateMember,
}

pub type MembersLoader = Box<dyn Fn(&[AssetId]) -> Vec<DuplicateMember>>;

pub struct Attachments {
    pub collections: usize,
    pub tags: usize,
    pub favorite: bool,
    pub note: bool,
}

pub enum RemovalFailure {
    Changed,
    Refused(Refusal),
}

pub type Removed = Result<ChangeStamp, RemovalFailure>;

struct MemberControls {
    id: AssetId,
    tick: gtk::CheckButton,
    kept: gtk::Label,
}

struct GroupState {
    reference: Cell<AssetId>,
    ticked: RefCell<Vec<AssetId>>,
    rows: RefCell<Vec<MemberControls>>,
    first_radio: RefCell<Option<gtk::CheckButton>>,
    action: gtk::Button,
}

pub struct DuplicateActions {
    pub plan: Box<dyn Fn() -> Result<PlannedSearch, String>>,
    pub members: MembersLoader,
    pub thumbnails: Rc<ThumbnailCache>,
    pub remove: Box<dyn Fn(&ConfirmedRemoval) -> Removed>,
    pub attachments: Box<dyn Fn(AssetId) -> Attachments>,
    pub undo: Box<dyn Fn(ChangeStamp) -> Replaced>,
    pub changed: Box<dyn Fn()>,
    pub show: Box<dyn Fn(AssetId)>,
}

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::{DuplicateActions, DuplicateMember, WaitingThumbnail};
    use pigoune_core::{AssetId, DuplicateKind};

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/duplicates-page.ui")]
    pub struct PigouneDuplicatesPage {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub message_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub search_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub progress_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub scroller: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub caption_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub groups_box: TemplateChild<gtk::Box>,
        pub actions: RefCell<Option<Rc<DuplicateActions>>>,
        pub groups: RefCell<Vec<Vec<AssetId>>>,
        pub details: RefCell<HashMap<AssetId, DuplicateMember>>,
        pub waiting: RefCell<Vec<WaitingThumbnail>>,
        pub pending: Cell<u64>,
        pub cancel_requested: RefCell<Option<Arc<AtomicBool>>>,
        pub scroll_followed: Cell<bool>,
        pub removing: Cell<bool>,
        pub kinds: RefCell<HashMap<AssetId, DuplicateKind>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneDuplicatesPage {
        const NAME: &'static str = "PigouneDuplicatesPage";
        type Type = super::PigouneDuplicatesPage;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneDuplicatesPage {}
    impl WidgetImpl for PigouneDuplicatesPage {}
    impl BoxImpl for PigouneDuplicatesPage {}
}

glib::wrapper! {
    pub struct PigouneDuplicatesPage(ObjectSubclass<imp::PigouneDuplicatesPage>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

#[gtk::template_callbacks]
impl PigouneDuplicatesPage {
    pub fn connect_actions(&self, actions: DuplicateActions) {
        self.imp().actions.replace(Some(Rc::new(actions)));
    }

    pub fn cancel(&self) {
        if let Some(requested) = self.imp().cancel_requested.borrow().as_ref() {
            requested.store(true, Ordering::Relaxed);
        }
    }

    #[template_callback]
    fn on_cancel_clicked(&self) {
        self.imp().cancel_button.set_sensitive(false);
        self.cancel();
    }

    #[template_callback]
    fn on_search_clicked(&self) {
        let imp = self.imp();
        if imp.cancel_requested.borrow().is_some() {
            return;
        }
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let planned = match (actions.plan)() {
            Ok(planned) => planned,
            Err(message) => {
                self.show_message(&gettext("The Search Failed"), &message);
                return;
            }
        };
        imp.pending.set(planned.pending);
        let canceled = Arc::new(AtomicBool::new(false));
        imp.cancel_requested.replace(Some(Arc::clone(&canceled)));
        self.show_running();
        let (sender, receiver) = async_channel::unbounded();
        let plan = planned.plan;
        let work = gio::spawn_blocking(move || {
            plan.run(|progress| {
                let _ = sender.send_blocking(progress);
                if canceled.load(Ordering::Relaxed) {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
        });
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = page)]
            self,
            async move {
                while let Ok(progress) = receiver.recv().await {
                    page.show_progress(progress);
                }
            }
        ));
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = page)]
            self,
            async move {
                let found = work.await.ok().flatten();
                page.imp().cancel_requested.replace(None);
                match found {
                    Some(groups) => {
                        let imp = page.imp();
                        {
                            let mut kinds = imp.kinds.borrow_mut();
                            kinds.clear();
                            for member in groups.iter().flat_map(|group| group.members.iter()) {
                                kinds.insert(member.asset, member.kind);
                            }
                        }
                        imp.groups.replace(
                            groups
                                .iter()
                                .map(|group| {
                                    group.members.iter().map(|member| member.asset).collect()
                                })
                                .collect(),
                        );
                        page.show_groups();
                    }
                    None => page.show_idle(),
                }
            }
        ));
    }

    fn show_progress(&self, progress: DuplicateProgress) {
        if progress.total > 0 {
            let done = f64::from(u32::try_from(progress.done).unwrap_or(u32::MAX));
            let total = f64::from(u32::try_from(progress.total).unwrap_or(u32::MAX));
            let fraction = done / total;
            self.imp().progress_bar.set_fraction(fraction);
        }
    }

    fn show_idle(&self) {
        let imp = self.imp();
        imp.message_page
            .set_title(&gettext("Look for Possible Duplicates"));
        imp.message_page.set_description(Some(&gettext(
            "Pigoune compares the drawings of your assets and groups those that look alike, such as the same logo in two formats or sizes. Nothing is changed or deleted until you choose.",
        )));
        imp.search_button.set_label(&gettext("_Search"));
        imp.search_button.set_visible(true);
        imp.progress_box.set_visible(false);
        imp.stack.set_visible_child_name("message");
    }

    fn show_running(&self) {
        let imp = self.imp();
        imp.message_page.set_title(&gettext("Searching…"));
        imp.message_page.set_description(Some(&gettext(
            "Pigoune is comparing the drawings of your assets. Nothing is changed.",
        )));
        imp.search_button.set_visible(false);
        imp.progress_bar.set_fraction(0.0);
        imp.cancel_button.set_sensitive(true);
        imp.progress_box.set_visible(true);
        imp.stack.set_visible_child_name("message");
    }

    fn show_message(&self, title: &str, description: &str) {
        let imp = self.imp();
        imp.message_page.set_title(title);
        imp.message_page.set_description(Some(description));
        imp.search_button.set_label(&gettext("_Search Again"));
        imp.search_button.set_visible(true);
        imp.progress_box.set_visible(false);
        imp.stack.set_visible_child_name("message");
    }

    fn show_groups(&self) {
        let imp = self.imp();
        let total = imp.groups.borrow().len();
        if total == 0 {
            self.show_message(&gettext("No Duplicates Found"), &self.nothing_text());
            return;
        }
        imp.caption_label.set_label(&self.caption_text(total));
        while let Some(child) = imp.groups_box.first_child() {
            imp.groups_box.remove(&child);
        }
        imp.waiting.borrow_mut().clear();
        let shown: Vec<Vec<AssetId>> = imp
            .groups
            .borrow()
            .iter()
            .take(GROUPS_SHOWN)
            .cloned()
            .collect();
        let first_rows: Vec<AssetId> = shown
            .iter()
            .flat_map(|group| group.iter().take(MEMBERS_SHOWN).copied())
            .collect();
        self.ensure_details(&first_rows);
        for group in &shown {
            imp.groups_box.append(&self.group_widget(group));
        }
        imp.stack.set_visible_child_name("results");
        self.follow_scrolling();
        self.load_visible_thumbnails_later();
    }

    fn ensure_details(&self, ids: &[AssetId]) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let missing: Vec<AssetId> = {
            let details = imp.details.borrow();
            ids.iter()
                .copied()
                .filter(|id| !details.contains_key(id))
                .collect()
        };
        if missing.is_empty() {
            return;
        }
        let loaded = (actions.members)(&missing);
        imp.details
            .borrow_mut()
            .extend(loaded.into_iter().map(|member| (member.id, member)));
    }

    fn follow_scrolling(&self) {
        let imp = self.imp();
        if imp.scroll_followed.replace(true) {
            return;
        }
        imp.scroller
            .vadjustment()
            .connect_value_changed(glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |_| page.load_visible_thumbnails()
            ));
    }

    fn load_visible_thumbnails_later(&self) {
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move || page.load_visible_thumbnails()
        ));
    }

    fn load_visible_thumbnails(&self) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let view_height = f64::from(imp.scroller.height());
        let mut waiting = imp.waiting.borrow_mut();
        waiting.retain(|entry| {
            let Some(picture) = entry.picture.upgrade() else {
                return false;
            };
            let Some(bounds) = picture.compute_bounds(imp.scroller.upcast_ref::<gtk::Widget>())
            else {
                return true;
            };
            let top = f64::from(bounds.y());
            let bottom = top + f64::from(bounds.height());
            let visible = bottom > -THUMBNAIL_MARGIN && top < view_height + THUMBNAIL_MARGIN;
            if visible {
                row_thumbnail::load(
                    &picture,
                    entry.member.id,
                    entry.member.file.clone(),
                    entry.member.thumbnail_file.clone(),
                    &actions.thumbnails,
                );
            }
            !visible
        });
    }

    fn nothing_text(&self) -> String {
        let pending = self.imp().pending.get();
        if pending == 0 {
            gettext("No assets look alike.")
        } else {
            Self::pending_text(pending)
        }
    }

    fn caption_text(&self, total: usize) -> String {
        let mut text = ngettext(
            "{count} group of look-alikes",
            "{count} groups of look-alikes",
            u32::try_from(total).unwrap_or(u32::MAX),
        )
        .replace("{count}", &total.to_string());
        if total > GROUPS_SHOWN {
            text = format!(
                "{text}. {}",
                gettext("The first {count} are shown; deal with them, then search again.")
                    .replace("{count}", &GROUPS_SHOWN.to_string())
            );
        }
        let pending = self.imp().pending.get();
        if pending > 0 {
            text = format!("{text}. {}", Self::pending_text(pending));
        }
        text
    }

    fn pending_text(pending: u64) -> String {
        ngettext(
            "{count} asset is still being analysed. Search again later to include it.",
            "{count} assets are still being analysed. Search again later to include them.",
            u32::try_from(pending).unwrap_or(u32::MAX),
        )
        .replace("{count}", &pending.to_string())
    }

    fn group_widget(&self, group: &[AssetId]) -> adw::PreferencesGroup {
        let action = gtk::Button::builder()
            .valign(gtk::Align::Center)
            .sensitive(false)
            .build();
        action.add_css_class("destructive-action");
        let state = Rc::new(GroupState {
            reference: Cell::new(group[0]),
            ticked: RefCell::new(Vec::new()),
            rows: RefCell::new(Vec::new()),
            first_radio: RefCell::new(None),
            action: action.clone(),
        });
        Self::refresh_action(&state);
        let widget = adw::PreferencesGroup::builder()
            .title(
                gettext("{count} possible duplicates")
                    .replace("{count}", &group.len().to_string()),
            )
            .description(gettext(
                "Choose the asset to keep, then tick the assets to move to the trash. Nothing is ticked for you.",
            ))
            .header_suffix(&action)
            .build();
        action.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            #[strong]
            state,
            move |_| page.confirm(&state)
        ));
        self.append_members(&widget, &Rc::new(group.to_vec()), &state, 0, MEMBERS_SHOWN);
        widget
    }

    fn append_members(
        &self,
        widget: &adw::PreferencesGroup,
        group: &Rc<Vec<AssetId>>,
        state: &Rc<GroupState>,
        start: usize,
        batch: usize,
    ) {
        let end = (start + batch).min(group.len());
        self.ensure_details(&group[start..end]);
        for id in &group[start..end] {
            if let Some(member) = self.imp().details.borrow().get(id) {
                widget.add(&self.member_row(state, member));
            }
        }
        if end < group.len() {
            widget.add(&self.show_more_row(widget, group, state, end));
        }
    }

    fn show_more_row(
        &self,
        widget: &adw::PreferencesGroup,
        group: &Rc<Vec<AssetId>>,
        state: &Rc<GroupState>,
        next: usize,
    ) -> adw::ActionRow {
        let hidden = group.len() - next;
        let row = adw::ActionRow::builder()
            .title(gettext("Show {count} More").replace("{count}", &hidden.to_string()))
            .activatable(true)
            .build();
        row.connect_activated(glib::clone!(
            #[weak(rename_to = page)]
            self,
            #[weak]
            widget,
            #[strong]
            group,
            #[strong]
            state,
            move |row| {
                widget.remove(row);
                page.append_members(&widget, &group, &state, next, MORE_MEMBERS_SHOWN);
                page.load_visible_thumbnails_later();
            }
        ));
        row
    }

    fn member_row(&self, state: &Rc<GroupState>, member: &DuplicateMember) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(&member.name)
            .subtitle(&member.summary)
            .use_markup(false)
            .activatable(true)
            .build();
        let radio = gtk::CheckButton::builder()
            .valign(gtk::Align::Center)
            .tooltip_text(gettext("Keep this asset"))
            .build();
        if let Some(first) = state.first_radio.borrow().as_ref() {
            radio.set_group(Some(first));
        } else {
            state.first_radio.replace(Some(radio.clone()));
        }
        let id = member.id;
        let is_reference = state.reference.get() == id;
        radio.set_active(is_reference);
        radio.connect_toggled(glib::clone!(
            #[strong]
            state,
            move |radio| {
                if radio.is_active() {
                    Self::choose_reference(&state, id);
                }
            }
        ));
        let tick = gtk::CheckButton::builder()
            .label(gettext("Move to Trash"))
            .valign(gtk::Align::Center)
            .visible(!is_reference)
            .build();
        tick.connect_toggled(glib::clone!(
            #[strong]
            state,
            move |tick| Self::tick_changed(&state, id, tick.is_active())
        ));
        let kind = self.imp().kinds.borrow().get(&id).copied();
        let kind_label = gtk::Label::builder()
            .label(match kind {
                Some(DuplicateKind::VisuallyIdentical) => gettext("Identical"),
                Some(DuplicateKind::Probable) => gettext("Probable"),
                None => String::new(),
            })
            .tooltip_text(gettext("Compared with the largest asset of the group"))
            .css_classes(["dim-label"])
            .valign(gtk::Align::Center)
            .visible(kind.is_some() && state.reference.get() != id)
            .build();
        let kept = gtk::Label::builder()
            .label(gettext("Kept"))
            .css_classes(["dim-label"])
            .valign(gtk::Align::Center)
            .visible(is_reference)
            .build();
        state.rows.borrow_mut().push(MemberControls {
            id,
            tick: tick.clone(),
            kept: kept.clone(),
        });
        row.add_prefix(&radio);
        row.add_prefix(&self.thumbnail_of(member));
        row.add_suffix(&kind_label);
        row.add_suffix(&tick);
        row.add_suffix(&kept);
        row.connect_activated(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| page.show_resource(id)
        ));
        row
    }

    fn choose_reference(state: &Rc<GroupState>, id: AssetId) {
        state.reference.set(id);
        let controls: Vec<(AssetId, gtk::CheckButton, gtk::Label)> = state
            .rows
            .borrow()
            .iter()
            .map(|row| (row.id, row.tick.clone(), row.kept.clone()))
            .collect();
        for (row_id, tick, kept) in controls {
            let is_reference = row_id == id;
            if is_reference {
                tick.set_active(false);
            }
            tick.set_visible(!is_reference);
            kept.set_visible(is_reference);
        }
        Self::refresh_action(state);
    }

    fn tick_changed(state: &Rc<GroupState>, id: AssetId, on: bool) {
        {
            let mut ticked = state.ticked.borrow_mut();
            ticked.retain(|known| *known != id);
            if on && state.reference.get() != id {
                ticked.push(id);
            }
        }
        Self::refresh_action(state);
    }

    fn refresh_action(state: &Rc<GroupState>) {
        let count = state.ticked.borrow().len();
        let label = if count == 0 {
            gettext("Move to the Trash…")
        } else {
            ngettext(
                "Move {count} Asset to the Trash…",
                "Move {count} Assets to the Trash…",
                u32::try_from(count).unwrap_or(u32::MAX),
            )
            .replace("{count}", &count.to_string())
        };
        state.action.set_label(&label);
        state.action.set_sensitive(count > 0);
    }

    fn thumbnail_of(&self, member: &DuplicateMember) -> gtk::Picture {
        let imp = self.imp();
        let picture = row_thumbnail::blank();
        let remembered = imp
            .actions
            .borrow()
            .as_ref()
            .and_then(|actions| actions.thumbnails.remembered(member.id));
        match remembered {
            Some(texture) => picture.set_paintable(Some(&texture)),
            None => imp.waiting.borrow_mut().push(WaitingThumbnail {
                picture: picture.downgrade(),
                member: member.clone(),
            }),
        }
        picture
    }

    fn confirm(&self, state: &Rc<GroupState>) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let chosen = state.ticked.borrow().clone();
        let kept = state.reference.get();
        let Ok(removal) = ConfirmedRemoval::new(kept, chosen) else {
            return;
        };
        let kept_name = imp
            .details
            .borrow()
            .get(&kept)
            .map(|member| member.name.clone())
            .unwrap_or_default();
        let count = removal.chosen().len();
        let body = ngettext(
            "Keep “{name}” and move {count} other asset to the trash, where it stays for 30 days.",
            "Keep “{name}” and move {count} other assets to the trash, where they stay for 30 days.",
            u32::try_from(count).unwrap_or(u32::MAX),
        )
        .replace("{name}", &kept_name)
        .replace("{count}", &count.to_string());
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        for id in removal.chosen() {
            if let Some(member) = imp.details.borrow().get(id) {
                let attached = attachments_text(&(actions.attachments)(*id));
                let row = adw::ActionRow::builder()
                    .title(&member.name)
                    .subtitle(format!("{} · {attached}", member.summary))
                    .use_markup(false)
                    .build();
                row.add_prefix(&row_thumbnail::picture(
                    member.file.clone(),
                    member.thumbnail_file.clone(),
                ));
                list.append(&row);
            }
        }
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(260)
            .child(&list)
            .build();
        let note = gtk::Label::builder()
            .label(gettext(
                "Their collections, tags and notes stay with them in the trash. The asset you keep does not inherit them.",
            ))
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build();
        let extra = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .build();
        extra.append(&scroller);
        extra.append(&note);
        let dialog = adw::AlertDialog::new(Some(&gettext("Move to the Trash?")), Some(&body));
        dialog.set_extra_child(Some(&extra));
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("trash", &gettext("_Move to Trash"));
        dialog.set_response_appearance("trash", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.connect_response(
            Some("trash"),
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |_, _| page.remove(&removal)
            ),
        );
        dialog.present(Some(self));
    }

    fn remove(&self, removal: &ConfirmedRemoval) {
        let imp = self.imp();
        if imp.removing.replace(true) {
            return;
        }
        let Some(actions) = imp.actions.borrow().clone() else {
            imp.removing.set(false);
            return;
        };
        match (actions.remove)(removal) {
            Ok(stamp) => {
                let chosen = removal.chosen();
                {
                    let mut groups = imp.groups.borrow_mut();
                    for group in groups.iter_mut() {
                        if group.contains(&removal.kept()) {
                            group.retain(|id| !chosen.contains(id));
                        }
                    }
                    groups.retain(|group| group.len() > 1);
                }
                (actions.changed)();
                self.show_groups();
                self.offer_undo(stamp, chosen.len());
            }
            Err(RemovalFailure::Changed) => self.show_changed(),
            Err(RemovalFailure::Refused(refusal)) => self.show_refusal(&refusal),
        }
        imp.removing.set(false);
    }

    fn show_changed(&self) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("The Assets Changed")),
            Some(&gettext(
                "Some assets changed since the search, so nothing was moved to the trash. Pigoune searches again.",
            )),
        );
        alert.add_response("close", &gettext("_Close"));
        alert.connect_response(
            None,
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |_, _| page.on_search_clicked()
            ),
        );
        alert.present(Some(self));
    }

    fn offer_undo(&self, stamp: ChangeStamp, count: usize) {
        let text = ngettext(
            "{count} asset moved to the trash",
            "{count} assets moved to the trash",
            u32::try_from(count).unwrap_or(u32::MAX),
        )
        .replace("{count}", &count.to_string());
        let undo: Box<dyn Fn()> = Box::new(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move || page.undo_removal(stamp)
        ));
        self.show_toast(&text, Some((gettext("_Undo"), undo)));
    }

    fn undo_removal(&self, stamp: ChangeStamp) {
        let Some(actions) = self.imp().actions.borrow().clone() else {
            return;
        };
        match (actions.undo)(stamp) {
            Ok(()) => {
                (actions.changed)();
                self.on_search_clicked();
            }
            Err(refusal) => self.show_refusal(&refusal),
        }
    }

    fn show_resource(&self, id: AssetId) {
        let Some(actions) = self.imp().actions.borrow().clone() else {
            return;
        };
        if let Some(dialog) = self
            .ancestor(adw::Dialog::static_type())
            .and_downcast::<adw::Dialog>()
        {
            dialog.close();
        }
        (actions.show)(id);
    }

    fn show_refusal(&self, refusal: &Refusal) {
        let alert = adw::AlertDialog::new(Some(&refusal.title), Some(&refusal.body));
        alert.add_response("close", &gettext("_Close"));
        alert.present(Some(self));
    }

    fn show_toast(&self, text: &str, button: Option<(String, Box<dyn Fn()>)>) {
        let Some(overlay) = self
            .ancestor(adw::ToastOverlay::static_type())
            .and_downcast::<adw::ToastOverlay>()
        else {
            return;
        };
        let toast = adw::Toast::new(text);
        if let Some((label, on_click)) = button {
            toast.set_button_label(Some(&label));
            toast.connect_button_clicked(move |_| on_click());
        }
        toasts::announce(&overlay, &toast);
    }
}

fn attachments_text(attached: &Attachments) -> String {
    let mut parts = Vec::new();
    if attached.collections > 0 {
        parts.push(
            ngettext(
                "{count} collection",
                "{count} collections",
                u32::try_from(attached.collections).unwrap_or(u32::MAX),
            )
            .replace("{count}", &attached.collections.to_string()),
        );
    }
    if attached.tags > 0 {
        parts.push(
            ngettext(
                "{count} tag",
                "{count} tags",
                u32::try_from(attached.tags).unwrap_or(u32::MAX),
            )
            .replace("{count}", &attached.tags.to_string()),
        );
    }
    if attached.favorite {
        parts.push(gettext("Favorite"));
    }
    if attached.note {
        parts.push(gettext("Has a note"));
    }
    if parts.is_empty() {
        gettext("No collection, tag or note")
    } else {
        parts.join(" · ")
    }
}
