use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;
use pigoune_core::{AssetId, ChangeStamp};

use crate::health_page::{Refusal, Replaced, Trashed};
use crate::row_thumbnail;
use crate::toasts;

const GROUPS_SHOWN: usize = 20;

#[derive(Clone)]
pub struct DuplicateMember {
    pub id: AssetId,
    pub name: String,
    pub summary: String,
    pub file: PathBuf,
    pub thumbnail_file: PathBuf,
}

pub struct DuplicateReport {
    pub groups: Vec<Vec<DuplicateMember>>,
    pub pending: u64,
}

pub struct DuplicateActions {
    pub search: Box<dyn Fn() -> Result<DuplicateReport, String>>,
    pub trash: Box<dyn Fn(Vec<AssetId>) -> Trashed>,
    pub undo: Box<dyn Fn(ChangeStamp) -> Replaced>,
    pub changed: Box<dyn Fn()>,
    pub show: Box<dyn Fn(AssetId)>,
}

mod imp {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::{DuplicateActions, DuplicateMember};

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
        pub caption_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub groups_box: TemplateChild<gtk::Box>,
        pub actions: RefCell<Option<Rc<DuplicateActions>>>,
        pub groups: RefCell<Vec<Vec<DuplicateMember>>>,
        pub pending: Cell<u64>,
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

    #[template_callback]
    fn on_search_clicked(&self) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        match (actions.search)() {
            Ok(report) => {
                imp.pending.set(report.pending);
                imp.groups.replace(report.groups);
                self.show_groups();
            }
            Err(message) => self.show_message(&gettext("The Search Failed"), &message),
        }
    }

    fn show_message(&self, title: &str, description: &str) {
        let imp = self.imp();
        imp.message_page.set_title(title);
        imp.message_page.set_description(Some(description));
        imp.search_button.set_label(&gettext("_Search Again"));
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
        for group in imp.groups.borrow().iter().take(GROUPS_SHOWN) {
            imp.groups_box.append(&self.group_widget(group));
        }
        imp.stack.set_visible_child_name("results");
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

    fn group_widget(&self, group: &[DuplicateMember]) -> adw::PreferencesGroup {
        let widget = adw::PreferencesGroup::builder()
            .title(
                gettext("{count} assets look alike").replace("{count}", &group.len().to_string()),
            )
            .build();
        let key = group[0].id;
        for member in group {
            widget.add(&self.member_row(key, member));
        }
        widget
    }

    fn member_row(&self, key: AssetId, member: &DuplicateMember) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(&member.name)
            .subtitle(&member.summary)
            .use_markup(false)
            .activatable(true)
            .build();
        row.add_prefix(&row_thumbnail::picture(
            member.file.clone(),
            member.thumbnail_file.clone(),
        ));
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

    fn keep(&self, key: AssetId, kept: AssetId) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let Some(position) = imp
            .groups
            .borrow()
            .iter()
            .position(|group| group[0].id == key)
        else {
            return;
        };
        let group = imp.groups.borrow()[position].clone();
        let others: Vec<AssetId> = group
            .iter()
            .map(|member| member.id)
            .filter(|id| *id != kept)
            .collect();
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

    fn offer_undo(
        &self,
        stamp: ChangeStamp,
        position: usize,
        group: Vec<DuplicateMember>,
        count: usize,
    ) {
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

    fn undo_keeping(&self, stamp: ChangeStamp, position: usize, group: Vec<DuplicateMember>) {
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
        toast.set_use_markup(false);
        if let Some((label, on_click)) = button {
            toast.set_button_label(Some(&label));
            toast.connect_button_clicked(move |_| on_click());
        }
        toasts::announce(&overlay, &toast);
    }
}
