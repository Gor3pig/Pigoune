use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{
    AssetId, ChangeStamp, ConfirmedRemoval, DuplicateKind, DuplicatePlan, DuplicateProgress,
    IgnoreId, IgnoreOutcome, IgnoredEntry,
};

use crate::chart_slices::ratio;
use crate::group_name;
use crate::health_page::{Refusal, Replaced};
use crate::row_thumbnail;
use crate::thumbnails::ThumbnailCache;
use crate::toasts;

const GROUPS_SHOWN: usize = 20;
const MEMBERS_SHOWN: usize = 6;
const MORE_MEMBERS_SHOWN: usize = 20;
const THUMBNAIL_MARGIN: f64 = 300.0;
const PAINT_DELAY: std::time::Duration = std::time::Duration::from_millis(40);

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

pub type Preparing = Box<
    dyn Fn(
        Arc<AtomicBool>,
        Rc<dyn Fn(u64, u64)>,
    ) -> Pin<Box<dyn Future<Output = Result<bool, String>>>>,
>;

pub type MembersLoader = Box<dyn Fn(&[AssetId]) -> Vec<DuplicateMember>>;

#[derive(Default)]
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

pub type Ignored = Result<ChangeStamp, Refusal>;
pub type IgnoredDone = Result<IgnoreOutcome, Refusal>;
pub type IgnoreGroup = Box<dyn Fn(&[AssetId]) -> IgnoredDone>;
pub type IgnoreMember = Box<dyn Fn(AssetId, &[AssetId]) -> IgnoredDone>;
pub type IgnoredEntries = Box<dyn Fn() -> Vec<IgnoredEntry>>;

pub struct IgnoringActions {
    pub ignore_group: IgnoreGroup,
    pub ignore_member: IgnoreMember,
    pub unignore: Box<dyn Fn(IgnoreId) -> Ignored>,
    pub unignore_all: Box<dyn Fn() -> Ignored>,
    pub ignored_entries: IgnoredEntries,
}

pub struct ResultsSnapshot {
    pub groups: Vec<Vec<AssetId>>,
    pub selections: HashMap<AssetId, Selection>,
}

pub struct MemberControls {
    id: AssetId,
    tick: gtk::CheckButton,
    kept: gtk::Label,
    kind: gtk::Label,
}

pub struct GroupState {
    pub(crate) key: AssetId,
    pub(crate) members: Vec<AssetId>,
    actions: Rc<DuplicateActions>,
    pub(crate) reference: Cell<AssetId>,
    pub(crate) ticked: RefCell<Vec<AssetId>>,
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
    pub prepare: Preparing,
    pub ignoring: IgnoringActions,
    pub compare: Box<dyn Fn(AssetId, AssetId) -> Option<DuplicateKind>>,
    pub trash_expires: Box<dyn Fn() -> bool>,
}

pub type Selection = (AssetId, Vec<AssetId>);

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::{DuplicateActions, DuplicateMember, GroupState, Selection, WaitingThumbnail};
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
        pub message_ignored_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub ignored_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub show_all_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub results_bar: TemplateChild<gtk::Box>,
        #[template_child]
        pub ignored_bar: TemplateChild<gtk::Box>,
        #[template_child]
        pub ignored_caption_label: TemplateChild<gtk::Label>,
        pub viewing_ignored: Cell<bool>,
        pub results_stale: Cell<bool>,
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
        pub search_again: Cell<bool>,
        pub last_snapshot: RefCell<Option<(pigoune_core::ChangeStamp, super::ResultsSnapshot)>>,
        pub states: RefCell<Vec<Rc<GroupState>>>,
        pub selections: RefCell<HashMap<AssetId, Selection>>,
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
        self.refresh_ignored_buttons();
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
    fn on_ignored_clicked(&self) {
        self.toggle_ignored_view();
    }

    #[template_callback]
    fn on_back_clicked(&self) {
        self.leave_ignored_view();
    }

    #[template_callback]
    fn on_show_all_clicked(&self) {
        self.confirm_show_all();
    }

    #[template_callback]
    pub(crate) fn on_search_clicked(&self) {
        let imp = self.imp();
        if imp.cancel_requested.borrow().is_some() {
            imp.search_again.set(true);
            return;
        }
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let canceled = Arc::new(AtomicBool::new(false));
        imp.cancel_requested.replace(Some(Arc::clone(&canceled)));
        imp.viewing_ignored.set(false);
        imp.results_stale.set(false);
        imp.last_snapshot.replace(None);
        self.show_running();
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = page)]
            self,
            async move { page.search(actions, canceled).await }
        ));
    }

    async fn search(&self, actions: Rc<DuplicateActions>, canceled: Arc<AtomicBool>) {
        let imp = self.imp();
        glib::timeout_future(PAINT_DELAY).await;
        let progress: Rc<dyn Fn(u64, u64)> = Rc::new(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |done, total| page.show_preparation(done, total)
        ));
        match (actions.prepare)(Arc::clone(&canceled), progress).await {
            Ok(true) => {}
            Ok(false) => {
                imp.cancel_requested.replace(None);
                imp.search_again.set(false);
                self.show_idle();
                return;
            }
            Err(message) => {
                imp.cancel_requested.replace(None);
                imp.search_again.set(false);
                self.show_message(&gettext("The Search Failed"), &message);
                return;
            }
        }
        let planned = match (actions.plan)() {
            Ok(planned) => planned,
            Err(message) => {
                imp.cancel_requested.replace(None);
                imp.search_again.set(false);
                self.show_message(&gettext("The Search Failed"), &message);
                return;
            }
        };
        imp.pending.set(planned.pending);
        self.show_comparing();
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
        let found = work.await.ok().flatten();
        imp.cancel_requested.replace(None);
        let again = imp.search_again.replace(false);
        match found {
            Some(_) if again => self.on_search_clicked(),
            Some(groups) => {
                imp.selections.borrow_mut().clear();
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
                        .map(|group| group.members.iter().map(|member| member.asset).collect())
                        .collect(),
                );
                self.show_groups();
            }
            None => self.show_idle(),
        }
    }

    fn show_preparation(&self, done: u64, total: u64) {
        let imp = self.imp();
        imp.message_page.set_description(Some(
            &ngettext(
                "Pigoune is analyzing the details of {count} asset, once. Nothing is changed.",
                "Pigoune is analyzing the details of {count} assets, once. Nothing is changed.",
                u32::try_from(total).unwrap_or(u32::MAX),
            )
            .replace("{count}", &total.to_string()),
        ));
        if total > 0 {
            imp.progress_bar.set_fraction(ratio(done, total));
        }
    }

    fn show_comparing(&self) {
        let imp = self.imp();
        imp.message_page.set_description(Some(&gettext(
            "Pigoune is comparing the drawings of your assets. Nothing is changed.",
        )));
        imp.progress_bar.set_fraction(0.0);
    }

    fn show_progress(&self, progress: DuplicateProgress) {
        if progress.total > 0 {
            let done = f64::from(u32::try_from(progress.done).unwrap_or(u32::MAX));
            let total = f64::from(u32::try_from(progress.total).unwrap_or(u32::MAX));
            let fraction = done / total;
            self.imp().progress_bar.set_fraction(fraction);
        }
    }

    pub(crate) fn show_idle(&self) {
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
        self.refresh_ignored_buttons();
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

    pub(crate) fn show_message(&self, title: &str, description: &str) {
        let imp = self.imp();
        imp.message_page.set_title(title);
        imp.message_page.set_description(Some(description));
        imp.search_button.set_label(&gettext("_Search Again"));
        imp.search_button.set_visible(true);
        imp.progress_box.set_visible(false);
        imp.stack.set_visible_child_name("message");
        self.refresh_ignored_buttons();
    }

    pub(crate) fn show_groups(&self) {
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
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        imp.states.borrow_mut().clear();
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
            if imp.details.borrow().contains_key(&group[0]) {
                imp.groups_box.append(&self.group_widget(&actions, group));
            }
        }
        imp.stack.set_visible_child_name("results");
        self.refresh_ignored_buttons();
        self.follow_scrolling();
        self.load_visible_thumbnails_later();
    }

    pub(crate) fn ensure_details(&self, ids: &[AssetId]) {
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

    pub(crate) fn follow_scrolling(&self) {
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

    pub(crate) fn load_visible_thumbnails_later(&self) {
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
        text = format!(
            "{text}. {}",
            gettext("Choose the asset to keep, then tick the assets to move to the trash.")
        );
        if total > GROUPS_SHOWN {
            text = format!(
                "{text} {}",
                gettext("The first {count} are shown; deal with them, then search again.")
                    .replace("{count}", &GROUPS_SHOWN.to_string())
            );
        }
        let pending = self.imp().pending.get();
        if pending > 0 {
            text = format!("{text} {}", Self::pending_text(pending));
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

    fn group_widget(
        &self,
        actions: &Rc<DuplicateActions>,
        group: &[AssetId],
    ) -> adw::PreferencesGroup {
        let imp = self.imp();
        let action = gtk::Button::builder()
            .valign(gtk::Align::Center)
            .visible(false)
            .build();
        action.add_css_class("destructive-action");
        let saved = imp.selections.borrow().get(&group[0]).cloned();
        let (reference, ticked) = match saved {
            Some((reference, ticked)) if group.contains(&reference) => (
                reference,
                ticked
                    .into_iter()
                    .filter(|id| *id != reference && group.contains(id))
                    .collect(),
            ),
            _ => (group[0], Vec::new()),
        };
        let state = Rc::new(GroupState {
            key: group[0],
            members: group.to_vec(),
            actions: Rc::clone(actions),
            reference: Cell::new(reference),
            ticked: RefCell::new(ticked),
            rows: RefCell::new(Vec::new()),
            first_radio: RefCell::new(None),
            action: action.clone(),
        });
        imp.states.borrow_mut().push(Rc::clone(&state));
        Self::refresh_action(&state);
        let more = Self::more_menu(
            &gettext("Group Actions"),
            vec![(
                gettext("Ignore Group"),
                Box::new(glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    #[weak]
                    state,
                    move || page.ignore_group_of(&state)
                )),
            )],
        );
        let widget = adw::PreferencesGroup::builder()
            .title(self.group_name(group))
            .description(
                ngettext(
                    "{count} similar asset",
                    "{count} similar assets",
                    u32::try_from(group.len()).unwrap_or(u32::MAX),
                )
                .replace("{count}", &group.len().to_string()),
            )
            .header_suffix(&Self::group_buttons(&action, &more))
            .build();
        action.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            #[weak]
            state,
            move |_| page.confirm(&state)
        ));
        self.append_members(&widget, &Rc::new(group.to_vec()), &state, 0, MEMBERS_SHOWN);
        widget
    }

    pub(crate) fn group_name(&self, group: &[AssetId]) -> String {
        let details = self.imp().details.borrow();
        let names: Vec<&str> = group
            .iter()
            .take(MEMBERS_SHOWN)
            .filter_map(|id| details.get(id))
            .map(|member| member.name.as_str())
            .collect();
        let fallback = names.first().copied().unwrap_or_default();
        glib::markup_escape_text(&group_name::common_name(&names, fallback)).to_string()
    }

    fn group_buttons(action: &gtk::Button, more: &gtk::MenuButton) -> gtk::Box {
        let buttons = gtk::Box::builder()
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();
        buttons.append(action);
        buttons.append(more);
        buttons
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
            #[weak]
            state,
            move |radio| {
                if radio.is_active() {
                    Self::choose_reference(&state, id);
                }
            }
        ));
        let kind = if is_reference {
            None
        } else if state.reference.get() == state.key {
            self.imp().kinds.borrow().get(&id).copied()
        } else {
            (state.actions.compare)(state.reference.get(), id)
        };
        let tick = gtk::CheckButton::builder()
            .tooltip_text(gettext("Move to Trash"))
            .valign(gtk::Align::Center)
            .visible(!is_reference)
            .sensitive(kind.is_some())
            .active(kind.is_some() && state.ticked.borrow().contains(&id))
            .build();
        tick.update_property(&[gtk::accessible::Property::Label(&gettext("Move to Trash"))]);
        tick.connect_toggled(glib::clone!(
            #[weak]
            state,
            move |tick| Self::tick_changed(&state, id, tick.is_active())
        ));
        let kind_label = gtk::Label::builder()
            .label(kind_text(kind))
            .tooltip_text(gettext("Compared with the asset you keep"))
            .css_classes(["dim-label"])
            .valign(gtk::Align::Center)
            .visible(!is_reference)
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
            kind: kind_label.clone(),
        });
        row.add_prefix(&radio);
        row.add_prefix(&self.thumbnail_of(member));
        row.add_suffix(&kind_label);
        row.add_suffix(&tick);
        row.add_suffix(&kept);
        let more = Self::more_menu(
            &gettext("Asset Actions"),
            vec![(
                gettext("Not a Duplicate"),
                Box::new(glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    #[weak]
                    state,
                    move || page.ignore_member_of(&state, id)
                )),
            )],
        );
        row.add_suffix(&more);
        row.connect_activated(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| page.show_resource(id)
        ));
        row
    }

    fn choose_reference(state: &Rc<GroupState>, id: AssetId) {
        state.reference.set(id);
        state.ticked.borrow_mut().retain(|known| *known != id);
        let controls: Vec<(AssetId, gtk::CheckButton, gtk::Label, gtk::Label)> = state
            .rows
            .borrow()
            .iter()
            .map(|row| (row.id, row.tick.clone(), row.kept.clone(), row.kind.clone()))
            .collect();
        for (row_id, tick, kept, kind_label) in controls {
            let is_reference = row_id == id;
            let kind = if is_reference {
                None
            } else {
                (state.actions.compare)(id, row_id)
            };
            if is_reference || kind.is_none() {
                tick.set_active(false);
            }
            tick.set_sensitive(kind.is_some());
            tick.set_visible(!is_reference);
            kept.set_visible(is_reference);
            kind_label.set_label(&kind_text(kind));
            kind_label.set_visible(!is_reference);
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
        let label = ngettext(
            "Move {count} Asset to the Trash…",
            "Move {count} Assets to the Trash…",
            u32::try_from(count).unwrap_or(u32::MAX),
        )
        .replace("{count}", &count.to_string());
        state.action.set_label(&label);
        state.action.set_visible(count > 0);
    }

    pub(crate) fn thumbnail_of(&self, member: &DuplicateMember) -> gtk::Picture {
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
        let known = removal
            .chosen()
            .iter()
            .chain([&kept])
            .all(|id| imp.details.borrow().contains_key(id));
        if !known {
            self.show_changed();
            return;
        }
        let kept_name = imp
            .details
            .borrow()
            .get(&kept)
            .map(|member| member.name.clone())
            .unwrap_or_default();
        let count = removal.chosen().len();
        let count_text = u32::try_from(count).unwrap_or(u32::MAX);
        let body = if (actions.trash_expires)() {
            ngettext(
                "Keep “{name}” and move {count} other asset to the trash, where it stays for 30 days.",
                "Keep “{name}” and move {count} other assets to the trash, where they stay for 30 days.",
                count_text,
            )
        } else {
            ngettext(
                "Keep “{name}” and move {count} other asset to the trash, where it stays until the trash is emptied.",
                "Keep “{name}” and move {count} other assets to the trash, where they stay until the trash is emptied.",
                count_text,
            )
        }
        .replace("{count}", &count.to_string())
        .replace("{name}", &kept_name);
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
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let before = self.snapshot_results();
        match (actions.remove)(removal) {
            Ok(stamp) => {
                imp.last_snapshot.replace(Some((stamp, before)));
                let chosen = removal.chosen();
                self.save_selections(removal.kept());
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
    }

    pub(crate) fn save_selections(&self, except_reference: AssetId) {
        let imp = self.imp();
        let mut selections = imp.selections.borrow_mut();
        for state in imp.states.borrow().iter() {
            if state.reference.get() == except_reference {
                selections.remove(&state.key);
            } else {
                selections.insert(
                    state.key,
                    (state.reference.get(), state.ticked.borrow().clone()),
                );
            }
        }
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
        self.offer_undo_text(stamp, &text);
    }

    pub(crate) fn offer_undo_text(&self, stamp: ChangeStamp, text: &str) {
        let undo: Box<dyn Fn()> = Box::new(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move || page.undo_removal(stamp)
        ));
        self.show_toast(text, Some((gettext("_Undo"), undo)));
    }

    fn undo_removal(&self, stamp: ChangeStamp) {
        let Some(actions) = self.imp().actions.borrow().clone() else {
            return;
        };
        match (actions.undo)(stamp) {
            Ok(()) => {
                (actions.changed)();
                self.show_after_undo(stamp);
            }
            Err(refusal) => self.show_refusal(&refusal),
        }
    }

    pub(crate) fn show_resource(&self, id: AssetId) {
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

    pub(crate) fn show_refusal(&self, refusal: &Refusal) {
        let alert = adw::AlertDialog::new(Some(&refusal.title), Some(&refusal.body));
        alert.add_response("close", &gettext("_Close"));
        alert.present(Some(self));
    }

    pub(crate) fn show_toast(&self, text: &str, button: Option<(String, Box<dyn Fn()>)>) {
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

fn kind_text(kind: Option<DuplicateKind>) -> String {
    match kind {
        Some(DuplicateKind::VisuallyIdentical) => gettext("Identical"),
        Some(DuplicateKind::Probable) => gettext("Probable"),
        None => gettext("Different"),
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
