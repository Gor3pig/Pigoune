use std::ops::ControlFlow;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{AssetId, ChangeStamp, DuplicatePlan, DuplicateProgress};

use crate::health_page::{Refusal, Replaced, Trashed};
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

pub struct DuplicateActions {
    pub plan: Box<dyn Fn() -> Result<PlannedSearch, String>>,
    pub members: MembersLoader,
    pub thumbnails: Rc<ThumbnailCache>,
    pub trash: Box<dyn Fn(Vec<AssetId>) -> Trashed>,
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
    use pigoune_core::AssetId;

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
                        page.imp().groups.replace(groups);
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
        imp.message_page.set_title(&gettext("Look for Duplicates"));
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
        let widget = adw::PreferencesGroup::builder()
            .title(
                gettext("{count} assets look alike").replace("{count}", &group.len().to_string()),
            )
            .build();
        self.append_members(&widget, &Rc::new(group.to_vec()), 0, MEMBERS_SHOWN);
        widget
    }

    fn append_members(
        &self,
        widget: &adw::PreferencesGroup,
        group: &Rc<Vec<AssetId>>,
        start: usize,
        batch: usize,
    ) {
        let end = (start + batch).min(group.len());
        self.ensure_details(&group[start..end]);
        let key = group[0];
        for id in &group[start..end] {
            if let Some(member) = self.imp().details.borrow().get(id) {
                widget.add(&self.member_row(key, member));
            }
        }
        if end < group.len() {
            widget.add(&self.show_more_row(widget, group, end));
        }
    }

    fn show_more_row(
        &self,
        widget: &adw::PreferencesGroup,
        group: &Rc<Vec<AssetId>>,
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
            move |row| {
                widget.remove(row);
                page.append_members(&widget, &group, next, MORE_MEMBERS_SHOWN);
                page.load_visible_thumbnails_later();
            }
        ));
        row
    }

    fn member_row(&self, key: AssetId, member: &DuplicateMember) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(&member.name)
            .subtitle(&member.summary)
            .use_markup(false)
            .activatable(true)
            .build();
        row.add_prefix(&self.thumbnail_of(member));
        let keep = gtk::Button::builder()
            .label(gettext("_Keep This One"))
            .use_underline(true)
            .valign(gtk::Align::Center)
            .tooltip_text(gettext("Move the other assets of this group to the trash"))
            .build();
        let kept = member.id;
        keep.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| page.keep(key, kept)
        ));
        row.add_suffix(&keep);
        row.connect_activated(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| page.show_resource(kept)
        ));
        row
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

    fn keep(&self, key: AssetId, kept: AssetId) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let Some(position) = imp.groups.borrow().iter().position(|group| group[0] == key) else {
            return;
        };
        let group = imp.groups.borrow()[position].clone();
        let others: Vec<AssetId> = group.iter().copied().filter(|id| *id != kept).collect();
        let count = others.len();
        match (actions.trash)(others) {
            Ok(stamp) => {
                imp.groups.borrow_mut().remove(position);
                (actions.changed)();
                self.show_groups();
                self.offer_undo(stamp, position, group, count);
            }
            Err(refusal) => self.show_refusal(&refusal),
        }
    }

    fn offer_undo(&self, stamp: ChangeStamp, position: usize, group: Vec<AssetId>, count: usize) {
        let text = ngettext(
            "{count} asset moved to the trash",
            "{count} assets moved to the trash",
            u32::try_from(count).unwrap_or(u32::MAX),
        )
        .replace("{count}", &count.to_string());
        let undo: Box<dyn Fn()> = Box::new(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move || page.undo_keeping(stamp, position, group.clone())
        ));
        self.show_toast(&text, Some((gettext("_Undo"), undo)));
    }

    fn undo_keeping(&self, stamp: ChangeStamp, position: usize, group: Vec<AssetId>) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        match (actions.undo)(stamp) {
            Ok(()) => {
                (actions.changed)();
                let mut groups = imp.groups.borrow_mut();
                let position = position.min(groups.len());
                groups.insert(position, group);
                drop(groups);
                self.show_groups();
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
