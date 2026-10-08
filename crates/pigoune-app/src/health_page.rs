use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{HealthIssue, HealthPlan, HealthProgress, HealthReport};
use std::ops::ControlFlow;

const SHOWN_AT_FIRST: usize = 5;
const PROGRESS_STEP: usize = 10;

type PlanSource = Box<dyn Fn() -> Result<HealthPlan, String>>;

mod imp {
    use std::cell::RefCell;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::PlanSource;

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
        pub plan_source: RefCell<Option<PlanSource>>,
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
    pub fn connect_plan_source(&self, source: impl Fn() -> Result<HealthPlan, String> + 'static) {
        self.imp().plan_source.replace(Some(Box::new(source)));
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
        let planned = imp.plan_source.borrow().as_ref().map(|source| source());
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
                    Some(report) => page.show_report(&report),
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

    fn show_report(&self, report: &HealthReport) {
        let imp = self.imp();
        imp.progress_box.set_visible(false);
        while let Some(row) = imp.results_list.first_child() {
            imp.results_list.remove(&row);
        }
        let problems = report.problems();
        if problems == 0 {
            imp.status.set_icon_name(Some("object-select-symbolic"));
            imp.status.set_title(&gettext("Everything Is in Order"));
            imp.status.set_description(Some(&count_text(
                &ngettext(
                    "{count} file checked, no problem found.",
                    "{count} files checked, no problem found.",
                    u32::try_from(report.checked).unwrap_or(u32::MAX),
                ),
                report.checked,
            )));
            imp.results_list.set_visible(false);
        } else {
            imp.status.set_icon_name(Some("dialog-warning-symbolic"));
            imp.status.set_title(&count_text(
                &ngettext(
                    "{count} problem found",
                    "{count} problems found",
                    u32::try_from(problems).unwrap_or(u32::MAX),
                ),
                problems,
            ));
            imp.status
                .set_description(Some(&gettext("Nothing was changed.")));
            self.fill_results(report);
            imp.results_list.set_visible(true);
        }
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

    fn fill_results(&self, report: &HealthReport) {
        let list = &self.imp().results_list;
        let groups = [
            (gettext("Missing Files"), issue_rows(&report.missing)),
            (gettext("Damaged Files"), issue_rows(&report.damaged)),
            (
                gettext("Files Without a Record"),
                report
                    .unrecorded
                    .iter()
                    .map(|path| {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default();
                        (name, path.to_string_lossy().to_string())
                    })
                    .collect(),
            ),
        ];
        for (title, rows) in groups {
            if !rows.is_empty() {
                list.append(&group_row(&title, &rows));
            }
        }
    }
}

fn issue_rows(issues: &[HealthIssue]) -> Vec<(String, String)> {
    issues
        .iter()
        .map(|issue| {
            let place = match &issue.collection {
                Some(name) => gettext("In the collection “{name}”").replace("{name}", name),
                None => gettext("Not in a collection"),
            };
            (issue.display_name.clone(), place)
        })
        .collect()
}

fn group_row(title: &str, rows: &[(String, String)]) -> adw::ExpanderRow {
    let group = adw::ExpanderRow::builder()
        .title(title)
        .expanded(rows.len() <= SHOWN_AT_FIRST)
        .build();
    let count = gtk::Label::builder()
        .label(rows.len().to_string())
        .css_classes(["numeric", "dim-label"])
        .build();
    group.add_suffix(&count);
    for (name, detail) in rows.iter().take(SHOWN_AT_FIRST) {
        group.add_row(&detail_row(name, detail));
    }
    if rows.len() > SHOWN_AT_FIRST {
        group.add_row(&show_all_row(&group, rows));
    }
    group
}

fn show_all_row(group: &adw::ExpanderRow, rows: &[(String, String)]) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(count_text(&gettext("Show All {count}"), rows.len()))
        .activatable(true)
        .build();
    let remaining: Vec<(String, String)> = rows[SHOWN_AT_FIRST..].to_vec();
    row.connect_activated(glib::clone!(
        #[weak]
        group,
        move |row| {
            group.remove(row);
            for (name, detail) in &remaining {
                group.add_row(&detail_row(name, detail));
            }
        }
    ));
    row
}

fn detail_row(name: &str, detail: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(name))
        .subtitle(glib::markup_escape_text(detail))
        .subtitle_selectable(true)
        .build();
    row.set_title_lines(1);
    row.set_subtitle_lines(2);
    row
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
