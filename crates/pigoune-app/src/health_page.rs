use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::toasts;
use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{AssetId, ChangeStamp, HealthIssue, HealthPlan, HealthProgress, HealthReport};
use std::ops::ControlFlow;

const SHOWN_AT_FIRST: usize = 5;
const PROGRESS_STEP: usize = 10;

const CLOSE_RESPONSE: &str = "close";
const AGAIN_RESPONSE: &str = "again";
const REMOVE_RESPONSE: &str = "remove";

#[derive(Default)]
pub struct Refusal {
    pub title: String,
    pub body: String,
    pub try_again: bool,
}

pub type Adopted = Result<(AssetId, String), Refusal>;

pub type Replaced = Result<(), Refusal>;
pub type Trashed = Result<ChangeStamp, Refusal>;
pub type ReplaceAction = Box<dyn Fn(AssetId, &Path) -> Replaced>;

pub struct HealthActions {
    pub plan: Box<dyn Fn() -> Result<HealthPlan, String>>,
    pub adopt: Box<dyn Fn(&Path) -> Adopted>,
    pub replace: ReplaceAction,
    pub remove: Box<dyn Fn(AssetId) -> Replaced>,
    pub trash: Box<dyn Fn(AssetId) -> Trashed>,
    pub undo: Box<dyn Fn(ChangeStamp) -> Replaced>,
    pub changed: Box<dyn Fn()>,
    pub show: Box<dyn Fn(AssetId)>,
}

#[derive(Default)]
pub struct ListState {
    expanded: [Option<bool>; GROUPS],
    all_shown: [bool; GROUPS],
}

const GROUPS: usize = 3;
const MISSING: usize = 0;
const DAMAGED: usize = 1;
const UNRECORDED: usize = 2;

mod imp {
    use std::cell::RefCell;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::{HealthActions, ListState};
    use pigoune_core::HealthReport;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/health-page.ui")]
    pub struct PigouneHealthPage {
        #[template_child]
        pub status: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub progress_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub progress_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub results_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub action_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub caption_label: TemplateChild<gtk::Label>,
        pub actions: RefCell<Option<Rc<HealthActions>>>,
        pub report: RefCell<Option<HealthReport>>,
        pub list_state: RefCell<ListState>,
        pub repaired: Cell<bool>,
        pub cancel_requested: RefCell<Option<Arc<AtomicBool>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneHealthPage {
        const NAME: &'static str = "PigouneHealthPage";
        type Type = super::PigouneHealthPage;
        type ParentType = gtk::Box;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneHealthPage {}
    impl WidgetImpl for PigouneHealthPage {}
    impl BoxImpl for PigouneHealthPage {}
}

glib::wrapper! {
    pub struct PigouneHealthPage(ObjectSubclass<imp::PigouneHealthPage>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

#[gtk::template_callbacks]
impl PigouneHealthPage {
    pub fn connect_actions(&self, actions: HealthActions) {
        self.imp().actions.replace(Some(Rc::new(actions)));
    }

    pub fn cancel(&self) {
        if let Some(requested) = self.imp().cancel_requested.borrow().as_ref() {
            requested.store(true, Ordering::Relaxed);
        }
    }

    #[template_callback]
    fn on_action_clicked(&self) {
        if self.imp().cancel_requested.borrow().is_some() {
            self.imp().action_button.set_sensitive(false);
            self.cancel();
        } else {
            self.start();
        }
    }

    fn start(&self) {
        let imp = self.imp();
        let planned = imp
            .actions
            .borrow()
            .as_ref()
            .map(|actions| (actions.plan)());
        let plan = match planned {
            Some(Ok(plan)) => plan,
            Some(Err(message)) => {
                self.show_idle();
                imp.status.set_description(Some(&message));
                return;
            }
            None => return,
        };
        let cancel_requested = Arc::new(AtomicBool::new(false));
        imp.cancel_requested
            .replace(Some(Arc::clone(&cancel_requested)));
        self.show_running();
        let (progress_sender, progress_receiver) = async_channel::unbounded();
        let work = gio::spawn_blocking(move || {
            plan.run(|progress| {
                if progress.done % PROGRESS_STEP == 0 || progress.done == progress.total {
                    let _ = progress_sender.send_blocking(progress);
                }
                if cancel_requested.load(Ordering::Relaxed) {
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
                glib::spawn_future_local(glib::clone!(
                    #[weak]
                    page,
                    async move {
                        while let Ok(progress) = progress_receiver.recv().await {
                            page.show_progress(progress);
                        }
                    }
                ));
                let report = work.await.ok().flatten();
                page.imp().cancel_requested.replace(None);
                match report {
                    Some(report) => page.finish_check(report),
                    None => page.show_idle(),
                }
            }
        ));
    }

    fn show_idle(&self) {
        let imp = self.imp();
        imp.status.set_icon_name(Some("security-high-symbolic"));
        imp.status.set_title(&gettext("Check the Library"));
        imp.status.set_description(Some(&gettext(
            "Pigoune compares the files on the disk with the database. Nothing is changed or deleted.",
        )));
        imp.progress_box.set_visible(false);
        imp.results_list.set_visible(false);
        imp.caption_label.set_visible(true);
        self.set_action(&gettext("_Check"), true);
    }

    fn show_running(&self) {
        let imp = self.imp();
        imp.status.set_icon_name(Some("security-high-symbolic"));
        imp.status.set_title(&gettext("Checking…"));
        imp.status.set_description(Some(&gettext(
            "Reading the files and computing their fingerprints. You can keep using Pigoune.",
        )));
        imp.progress_bar.set_fraction(0.0);
        imp.progress_label.set_label("");
        imp.progress_box.set_visible(true);
        imp.results_list.set_visible(false);
        imp.caption_label.set_visible(false);
        self.set_action(&gettext("_Cancel"), false);
    }

    fn set_action(&self, label: &str, suggested: bool) {
        let button = &self.imp().action_button;
        button.set_label(label);
        button.set_sensitive(true);
        if suggested {
            button.add_css_class("suggested-action");
        } else {
            button.remove_css_class("suggested-action");
        }
    }

    fn show_progress(&self, progress: HealthProgress) {
        let imp = self.imp();
        if progress.total > 0 {
            imp.progress_bar
                .set_fraction(fraction(progress.done, progress.total));
        }
        imp.progress_label.set_label(
            &ngettext(
                "{done} of {total} file checked",
                "{done} of {total} files checked",
                u32::try_from(progress.total).unwrap_or(u32::MAX),
            )
            .replace("{done}", &progress.done.to_string())
            .replace("{total}", &progress.total.to_string()),
        );
    }

    fn finish_check(&self, report: HealthReport) {
        let imp = self.imp();
        imp.report.replace(Some(report));
        imp.repaired.set(false);
        imp.list_state.replace(ListState::default());
        self.show_report();
        let time = glib::DateTime::now_local()
            .ok()
            .and_then(|now| now.format("%H:%M").ok())
            .map(|time| time.to_string())
            .unwrap_or_default();
        imp.caption_label
            .set_label(&gettext("Checked today at {time}").replace("{time}", &time));
        imp.caption_label.set_visible(true);
        self.set_action(&gettext("Check _Again"), false);
    }

    fn show_report(&self) {
        let imp = self.imp();
        let Some(report) = imp.report.borrow().clone() else {
            return;
        };
        imp.progress_box.set_visible(false);
        while let Some(row) = imp.results_list.first_child() {
            imp.results_list.remove(&row);
        }
        let problems = report.problems();
        if problems == 0 {
            imp.status.set_icon_name(Some("object-select-symbolic"));
            imp.status.set_title(&gettext("Everything Is in Order"));
            imp.status.set_description(Some(&if imp.repaired.get() {
                gettext("All the problems are repaired.")
            } else {
                count_text(
                    &ngettext(
                        "{count} file checked, no problem found.",
                        "{count} files checked, no problem found.",
                        u32::try_from(report.checked).unwrap_or(u32::MAX),
                    ),
                    report.checked,
                )
            }));
            imp.results_list.set_visible(false);
        } else {
            imp.status.set_icon_name(Some("dialog-warning-symbolic"));
            let count = u32::try_from(problems).unwrap_or(u32::MAX);
            let title = if imp.repaired.get() {
                ngettext(
                    "{count} problem remaining",
                    "{count} problems remaining",
                    count,
                )
            } else {
                ngettext("{count} problem found", "{count} problems found", count)
            };
            imp.status.set_title(&count_text(&title, problems));
            imp.status
                .set_description(Some(&gettext("Nothing is repaired unless you ask for it.")));
            self.fill_results(&report);
            imp.results_list.set_visible(true);
        }
    }

    fn fill_results(&self, report: &HealthReport) {
        let list = &self.imp().results_list;
        let unrecorded = report
            .unrecorded
            .iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default();
                Line {
                    name,
                    detail: path.to_string_lossy().to_string(),
                    action: LineAction::Add(path.clone()),
                }
            })
            .collect();
        let groups = [
            (
                MISSING,
                gettext("Missing Files"),
                issue_lines(&report.missing, true, false),
            ),
            (
                DAMAGED,
                gettext("Damaged Files"),
                issue_lines(&report.damaged, false, true),
            ),
            (UNRECORDED, gettext("Files Without a Record"), unrecorded),
        ];
        for (index, title, lines) in groups {
            if !lines.is_empty() {
                list.append(&self.group_row(index, &title, &lines));
            }
        }
    }

    fn group_row(&self, index: usize, title: &str, lines: &[Line]) -> adw::ExpanderRow {
        let state = self.imp().list_state.borrow();
        let all_shown = state.all_shown[index];
        let group = adw::ExpanderRow::builder()
            .title(title)
            .expanded(state.expanded[index].unwrap_or(lines.len() <= SHOWN_AT_FIRST))
            .build();
        drop(state);
        group.connect_expanded_notify(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |group| page.imp().list_state.borrow_mut().expanded[index] =
                Some(group.is_expanded())
        ));
        if index == UNRECORDED && lines.len() > 1 {
            group.add_suffix(&self.add_all_button());
        }
        let count = gtk::Label::builder()
            .label(lines.len().to_string())
            .css_classes(["numeric", "dim-label"])
            .build();
        group.add_suffix(&count);
        let shown = if all_shown {
            lines.len()
        } else {
            SHOWN_AT_FIRST
        };
        for line in lines.iter().take(shown) {
            group.add_row(&self.detail_row(line));
        }
        if lines.len() > shown {
            group.add_row(&self.show_all_row(index, lines.len()));
        }
        group
    }

    fn show_all_row(&self, index: usize, count: usize) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(count_text(&gettext("Show All {count}"), count))
            .activatable(true)
            .build();
        row.connect_activated(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| {
                page.imp().list_state.borrow_mut().all_shown[index] = true;
                page.show_report();
            }
        ));
        row
    }

    fn detail_row(&self, line: &Line) -> adw::WrapBox {
        let buttons = gtk::Box::builder()
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();
        match &line.action {
            LineAction::Add(path) => {
                let button = suffix_button(&gettext("_Add"));
                button.connect_clicked(glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    #[strong]
                    path,
                    move |_| page.add_one(&path)
                ));
                buttons.append(&button);
            }
            LineAction::Repair {
                id,
                removable,
                trashable,
            } => {
                let button = suffix_button(&gettext("_Replace…"));
                button.connect_clicked(glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    #[strong(rename_to = name)]
                    line.name,
                    #[strong]
                    id,
                    move |_| page.choose_copy(id, &name)
                ));
                buttons.append(&button);
                if *removable {
                    let button = suffix_button(&gettext("Re_move…"));
                    button.connect_clicked(glib::clone!(
                        #[weak(rename_to = page)]
                        self,
                        #[strong(rename_to = name)]
                        line.name,
                        #[strong]
                        id,
                        move |_| page.confirm_removal(id, &name)
                    ));
                    buttons.append(&button);
                }
                if *trashable {
                    let button = suffix_button(&gettext("Move to _Trash"));
                    button.connect_clicked(glib::clone!(
                        #[weak(rename_to = page)]
                        self,
                        #[strong(rename_to = name)]
                        line.name,
                        #[strong]
                        id,
                        move |_| page.trash_damaged(id, &name)
                    ));
                    buttons.append(&button);
                }
            }
        }
        let row = adw::WrapBox::builder()
            .child_spacing(12)
            .line_spacing(8)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        row.append(&line_text(line));
        row.append(&buttons);
        row
    }

    fn add_all_button(&self) -> gtk::Button {
        let button = gtk::Button::builder()
            .label(gettext("Add _All"))
            .use_underline(true)
            .valign(gtk::Align::Center)
            .build();
        button.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| page.add_all()
        ));
        button
    }

    fn add_one(&self, path: &Path) {
        let Some(actions) = self.imp().actions.borrow().clone() else {
            return;
        };
        match (actions.adopt)(path) {
            Ok((id, name)) => {
                (actions.changed)();
                self.forget_unrecorded(&[path.to_path_buf()]);
                let text = gettext("“{name}” added to Unclassified").replace("{name}", &name);
                let show: Box<dyn Fn()> = Box::new(glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    move || page.show_resource(id)
                ));
                self.show_toast(&text, Some((gettext("_Show"), show)));
            }
            Err(refusal) => self.show_refusal(&refusal, None),
        }
    }

    fn add_all(&self) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let paths = imp
            .report
            .borrow()
            .as_ref()
            .map(|report| report.unrecorded.clone())
            .unwrap_or_default();
        imp.results_list.set_sensitive(false);
        imp.action_button.set_sensitive(false);
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = page)]
            self,
            async move {
                let mut added = Vec::new();
                for path in &paths {
                    if (actions.adopt)(path).is_ok() {
                        added.push(path.clone());
                    }
                    glib::timeout_future(Duration::ZERO).await;
                }
                let imp = page.imp();
                imp.results_list.set_sensitive(true);
                imp.action_button.set_sensitive(true);
                if !added.is_empty() {
                    (actions.changed)();
                    page.forget_unrecorded(&added);
                }
                page.show_toast(&added_text(added.len(), paths.len() - added.len()), None);
            }
        ));
    }

    fn forget_unrecorded(&self, paths: &[PathBuf]) {
        let imp = self.imp();
        if let Some(report) = imp.report.borrow_mut().as_mut() {
            report.unrecorded.retain(|path| !paths.contains(path));
        }
        imp.repaired.set(true);
        self.show_report();
    }

    fn choose_copy(&self, id: AssetId, name: &str) {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Choose a Copy of “{name}”").replace("{name}", name))
            .accept_label(gettext("_Open"))
            .modal(true)
            .build();
        let parent = self.root().and_downcast::<gtk::Window>();
        let name = name.to_owned();
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = page)]
            self,
            async move {
                let Ok(file) = dialog.open_future(parent.as_ref()).await else {
                    return;
                };
                if let Some(path) = file.path() {
                    page.replace_with(id, &name, &path);
                }
            }
        ));
    }

    fn replace_with(&self, id: AssetId, name: &str, copy: &Path) {
        let Some(actions) = self.imp().actions.borrow().clone() else {
            return;
        };
        match (actions.replace)(id, copy) {
            Ok(()) => {
                (actions.changed)();
                self.forget_issue(id);
                let text =
                    gettext("“{name}” replaced: the file is back in place").replace("{name}", name);
                self.show_toast(&text, None);
            }
            Err(refusal) => self.show_refusal(&refusal, Some((id, name))),
        }
    }

    fn confirm_removal(&self, id: AssetId, name: &str) {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Remove the Record of “{name}”?").replace("{name}", name)),
            Some(&gettext(
                "The file is missing. The resource, its tags, its notes and its place in the collections will be removed from the library. This cannot be undone.",
            )),
        );
        alert.add_response(CLOSE_RESPONSE, &gettext("_Cancel"));
        alert.add_response(REMOVE_RESPONSE, &gettext("_Remove the Record"));
        alert.set_response_appearance(REMOVE_RESPONSE, adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some(CLOSE_RESPONSE));
        alert.set_close_response(CLOSE_RESPONSE);
        let name = name.to_owned();
        alert.connect_response(
            Some(REMOVE_RESPONSE),
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |_, _| page.remove_record(id, &name)
            ),
        );
        alert.present(Some(self));
    }

    fn remove_record(&self, id: AssetId, name: &str) {
        let Some(actions) = self.imp().actions.borrow().clone() else {
            return;
        };
        match (actions.remove)(id) {
            Ok(()) => {
                (actions.changed)();
                self.forget_issue(id);
                let text = gettext("The record of “{name}” was removed").replace("{name}", name);
                self.show_toast(&text, None);
            }
            Err(refusal) => self.show_refusal(&refusal, None),
        }
    }

    fn trash_damaged(&self, id: AssetId, name: &str) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        let issue = imp
            .report
            .borrow()
            .as_ref()
            .and_then(|report| report.damaged.iter().find(|issue| issue.id == id).cloned());
        match (actions.trash)(id) {
            Ok(stamp) => {
                (actions.changed)();
                self.forget_issue(id);
                let text = gettext("“{name}” moved to the trash").replace("{name}", name);
                let undo: Box<dyn Fn()> = Box::new(glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    move || page.undo_trashing(stamp, issue.clone())
                ));
                self.show_toast(&text, Some((gettext("_Undo"), undo)));
            }
            Err(refusal) => self.show_refusal(&refusal, None),
        }
    }

    fn undo_trashing(&self, stamp: ChangeStamp, issue: Option<HealthIssue>) {
        let imp = self.imp();
        let Some(actions) = imp.actions.borrow().clone() else {
            return;
        };
        match (actions.undo)(stamp) {
            Ok(()) => {
                (actions.changed)();
                if let (Some(issue), Some(report)) = (issue, imp.report.borrow_mut().as_mut()) {
                    report.damaged.push(issue);
                }
                self.show_report();
            }
            Err(refusal) => self.show_refusal(&refusal, None),
        }
    }

    fn forget_issue(&self, id: AssetId) {
        let imp = self.imp();
        if let Some(report) = imp.report.borrow_mut().as_mut() {
            report.missing.retain(|issue| issue.id != id);
            report.damaged.retain(|issue| issue.id != id);
        }
        imp.repaired.set(true);
        self.show_report();
    }

    fn show_refusal(&self, refusal: &Refusal, again: Option<(AssetId, &str)>) {
        let alert = adw::AlertDialog::new(Some(&refusal.title), Some(&refusal.body));
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        if let (true, Some((id, name))) = (refusal.try_again, again) {
            alert.add_response(AGAIN_RESPONSE, &gettext("Choose Another _File"));
            alert.set_response_appearance(AGAIN_RESPONSE, adw::ResponseAppearance::Suggested);
            alert.set_default_response(Some(AGAIN_RESPONSE));
            let name = name.to_owned();
            alert.connect_response(
                Some(AGAIN_RESPONSE),
                glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    move |_, _| page.choose_copy(id, &name)
                ),
            );
        }
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
}

enum LineAction {
    Add(PathBuf),
    Repair {
        id: AssetId,
        removable: bool,
        trashable: bool,
    },
}

struct Line {
    name: String,
    detail: String,
    action: LineAction,
}

fn added_text(added: usize, refused: usize) -> String {
    let added_text = count_text(
        &ngettext(
            "{count} resource added to Unclassified",
            "{count} resources added to Unclassified",
            u32::try_from(added).unwrap_or(u32::MAX),
        ),
        added,
    );
    let refused_text = count_text(
        &ngettext(
            "{count} file could not be added.",
            "{count} files could not be added.",
            u32::try_from(refused).unwrap_or(u32::MAX),
        ),
        refused,
    );
    match (added, refused) {
        (_, 0) => added_text,
        (0, _) => refused_text,
        _ => format!("{added_text}. {refused_text}"),
    }
}

fn issue_lines(issues: &[HealthIssue], removable: bool, trashable: bool) -> Vec<Line> {
    issues
        .iter()
        .map(|issue| {
            let place = match &issue.collection {
                Some(name) => gettext("In the collection “{name}”").replace("{name}", name),
                None => gettext("Not in a collection"),
            };
            Line {
                name: issue.display_name.clone(),
                detail: place,
                action: LineAction::Repair {
                    id: issue.id,
                    removable,
                    trashable,
                },
            }
        })
        .collect()
}

fn line_text(line: &Line) -> gtk::Box {
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .valign(gtk::Align::Center)
        .build();
    let name = gtk::Label::builder()
        .label(&line.name)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .max_width_chars(28)
        .build();
    let detail = gtk::Label::builder()
        .label(&line.detail)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::Char)
        .max_width_chars(36)
        .selectable(true)
        .css_classes(["dim-label", "caption"])
        .build();
    text.append(&name);
    text.append(&detail);
    text
}

fn suffix_button(label: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .use_underline(true)
        .valign(gtk::Align::Center)
        .build()
}

fn count_text(template: &str, count: usize) -> String {
    template.replace("{count}", &count.to_string())
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a progress bar does not need exact counts beyond 2^52 files"
)]
fn fraction(done: usize, total: usize) -> f64 {
    done as f64 / total as f64
}
