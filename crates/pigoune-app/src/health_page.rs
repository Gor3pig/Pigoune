use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{AssetId, HealthIssue, HealthPlan, HealthProgress, HealthReport};
use std::ops::ControlFlow;

const SHOWN_AT_FIRST: usize = 5;
const PROGRESS_STEP: usize = 10;

const CLOSE_RESPONSE: &str = "close";

#[derive(Default)]
pub struct Refusal {
    pub title: String,
    pub body: String,
}

pub type Adopted = Result<(AssetId, String), Refusal>;

pub struct HealthActions {
    pub plan: Box<dyn Fn() -> Result<HealthPlan, String>>,
    pub adopt: Box<dyn Fn(&Path) -> Adopted>,
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
                    path: Some(path.clone()),
                }
            })
            .collect();
        let groups = [
            (
                MISSING,
                gettext("Missing Files"),
                issue_lines(&report.missing),
            ),
            (
                DAMAGED,
                gettext("Damaged Files"),
                issue_lines(&report.damaged),
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

    fn detail_row(&self, line: &Line) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&line.name))
            .subtitle(glib::markup_escape_text(&line.detail))
            .subtitle_selectable(true)
            .build();
        row.set_title_lines(1);
        row.set_subtitle_lines(2);
        if let Some(path) = &line.path {
            let button = gtk::Button::builder()
                .label(gettext("_Add"))
                .use_underline(true)
                .valign(gtk::Align::Center)
                .build();
            button.connect_clicked(glib::clone!(
                #[weak(rename_to = page)]
                self,
                #[strong]
                path,
                move |_| page.add_one(&path)
            ));
            row.add_suffix(&button);
        }
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
                self.show_toast(&text, Some(id));
            }
            Err(refusal) => self.show_refusal(&refusal),
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

    fn show_refusal(&self, refusal: &Refusal) {
        let alert = adw::AlertDialog::new(Some(&refusal.title), Some(&refusal.body));
        alert.add_response(CLOSE_RESPONSE, &gettext("_Close"));
        alert.present(Some(self));
    }

    fn show_toast(&self, text: &str, show: Option<AssetId>) {
        let Some(overlay) = self
            .ancestor(adw::ToastOverlay::static_type())
            .and_downcast::<adw::ToastOverlay>()
        else {
            return;
        };
        let toast = adw::Toast::new(text);
        toast.set_use_markup(false);
        if let Some(id) = show {
            toast.set_button_label(Some(&gettext("_Show")));
            toast.connect_button_clicked(glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |_| page.show_resource(id)
            ));
        }
        overlay.add_toast(toast);
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

struct Line {
    name: String,
    detail: String,
    path: Option<PathBuf>,
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

fn issue_lines(issues: &[HealthIssue]) -> Vec<Line> {
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
                path: None,
            }
        })
        .collect()
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
