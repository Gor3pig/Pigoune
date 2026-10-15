use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::ObjectSubclassIsExt;
use gettextrs::{gettext, ngettext};
use gtk::glib;
use pigoune_core::{AssetId, ChangeStamp};

use crate::duplicates_page::{DuplicateActions, GroupState, PigouneDuplicatesPage};

const IGNORED_GROUPS_SHOWN: usize = 50;

impl PigouneDuplicatesPage {
    fn actions(&self) -> Option<Rc<DuplicateActions>> {
        self.imp().actions.borrow().clone()
    }

    pub(crate) fn refresh_ignored_buttons(&self) {
        let imp = self.imp();
        let Some(actions) = self.actions() else {
            return;
        };
        let count = (actions.ignoring.ignored_groups)().len();
        let viewing = imp.viewing_ignored.get();
        let label = if viewing {
            gettext("Back to _Results")
        } else {
            gettext("_Ignored ({count})").replace("{count}", &count.to_string())
        };
        imp.ignored_button.set_use_underline(true);
        imp.ignored_button.set_label(&label);
        imp.ignored_button.set_visible(viewing || count > 0);
        imp.message_ignored_button.set_use_underline(true);
        imp.message_ignored_button
            .set_label(&gettext("_Ignored ({count})").replace("{count}", &count.to_string()));
        imp.message_ignored_button
            .set_visible(!viewing && count > 0);
        imp.show_all_button.set_visible(viewing);
    }

    pub(crate) fn toggle_ignored_view(&self) {
        if self.imp().viewing_ignored.get() {
            self.leave_ignored_view();
        } else {
            self.show_ignored();
        }
    }

    pub(crate) fn leave_ignored_view(&self) {
        let imp = self.imp();
        imp.viewing_ignored.set(false);
        if imp.results_stale.replace(false) {
            self.on_search_clicked();
        } else if imp.groups.borrow().is_empty() {
            self.show_idle();
        } else {
            self.show_groups();
        }
    }

    pub(crate) fn show_ignored(&self) {
        let imp = self.imp();
        let Some(actions) = self.actions() else {
            return;
        };
        let groups = (actions.ignoring.ignored_groups)();
        if groups.is_empty() {
            self.leave_ignored_view();
            return;
        }
        imp.viewing_ignored.set(true);
        let total = groups.len();
        let shown = &groups[..total.min(IGNORED_GROUPS_SHOWN)];
        let ids: Vec<AssetId> = shown.iter().flatten().copied().collect();
        self.ensure_details(&ids);
        imp.states.borrow_mut().clear();
        imp.waiting.borrow_mut().clear();
        while let Some(child) = imp.groups_box.first_child() {
            imp.groups_box.remove(&child);
        }
        imp.caption_label.set_label(&ignored_caption(total));
        for group in shown {
            imp.groups_box.append(&self.ignored_group_widget(group));
        }
        imp.stack.set_visible_child_name("results");
        self.follow_scrolling();
        self.load_visible_thumbnails_later();
        self.refresh_ignored_buttons();
    }

    fn ignored_group_widget(&self, group: &[AssetId]) -> adw::PreferencesGroup {
        let stop = gtk::Button::builder()
            .label(gettext("Stop Ignoring"))
            .valign(gtk::Align::Center)
            .build();
        let widget = adw::PreferencesGroup::builder()
            .title(
                ngettext(
                    "{count} ignored asset",
                    "{count} ignored assets",
                    u32::try_from(group.len()).unwrap_or(u32::MAX),
                )
                .replace("{count}", &group.len().to_string()),
            )
            .header_suffix(&stop)
            .build();
        let members = group.to_vec();
        stop.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| page.stop_ignoring(&members)
        ));
        for id in group {
            let Some(member) = self.imp().details.borrow().get(id).cloned() else {
                continue;
            };
            let id = member.id;
            let row = adw::ActionRow::builder()
                .title(&member.name)
                .subtitle(&member.summary)
                .use_markup(false)
                .activatable(true)
                .build();
            row.add_prefix(&self.thumbnail_of(&member));
            row.connect_activated(glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |_| page.show_resource(id)
            ));
            widget.add(&row);
        }
        widget
    }

    fn stop_ignoring(&self, group: &[AssetId]) {
        let Some(actions) = self.actions() else {
            return;
        };
        match (actions.ignoring.unignore_group)(group) {
            Ok(stamp) => self.after_showing_again(stamp, &gettext("Group shown again")),
            Err(refusal) => self.show_refusal(&refusal),
        }
    }

    fn after_showing_again(&self, stamp: ChangeStamp, text: &str) {
        self.imp().results_stale.set(true);
        self.show_ignored();
        self.offer_undo_text(stamp, text);
    }

    pub(crate) fn confirm_show_all(&self) {
        let count = self
            .actions()
            .map_or(0, |actions| (actions.ignoring.ignored_groups)().len());
        if count == 0 {
            return;
        }
        let body = ngettext(
            "{count} ignored group will be offered again at the next search.",
            "{count} ignored groups will be offered again at the next search.",
            u32::try_from(count).unwrap_or(u32::MAX),
        )
        .replace("{count}", &count.to_string());
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Show All Ignored Duplicates Again?")),
            Some(&body),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("show", &gettext("_Show Again"));
        dialog.set_response_appearance("show", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.connect_response(
            Some("show"),
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |_, _| page.show_all_again()
            ),
        );
        dialog.present(Some(self));
    }

    fn show_all_again(&self) {
        let Some(actions) = self.actions() else {
            return;
        };
        match (actions.ignoring.unignore_all)() {
            Ok(stamp) => {
                self.imp().results_stale.set(true);
                self.offer_undo_text(stamp, &gettext("All ignored groups shown again"));
                self.leave_ignored_view();
            }
            Err(refusal) => self.show_refusal(&refusal),
        }
    }

    pub(crate) fn ignore_group_of(&self, state: &Rc<GroupState>) {
        let Some(actions) = self.actions() else {
            return;
        };
        match (actions.ignoring.ignore_group)(&state.members) {
            Ok(stamp) => {
                let key = state.key;
                self.after_ignoring(
                    stamp,
                    state,
                    |group| !group.contains(&key),
                    &gettext("Group ignored"),
                );
            }
            Err(refusal) => self.show_refusal(&refusal),
        }
    }

    pub(crate) fn ignore_member_of(&self, state: &Rc<GroupState>, member: AssetId) {
        let Some(actions) = self.actions() else {
            return;
        };
        let others: Vec<AssetId> = state
            .members
            .iter()
            .copied()
            .filter(|other| *other != member)
            .collect();
        match (actions.ignoring.ignore_member)(member, &others) {
            Ok(stamp) => {
                let key = state.key;
                self.imp().groups.borrow_mut().iter_mut().for_each(|group| {
                    if group.contains(&key) {
                        group.retain(|id| *id != member);
                    }
                });
                self.after_ignoring(stamp, state, |_| true, &gettext("Asset ignored"));
            }
            Err(refusal) => self.show_refusal(&refusal),
        }
    }

    fn after_ignoring(
        &self,
        stamp: ChangeStamp,
        state: &Rc<GroupState>,
        keep: impl Fn(&Vec<AssetId>) -> bool,
        text: &str,
    ) {
        let imp = self.imp();
        self.save_selections(state.reference.get());
        {
            let mut groups = imp.groups.borrow_mut();
            groups.retain(|group| keep(group));
            groups.retain(|group| group.len() > 1);
        }
        self.show_groups();
        self.offer_undo_text(stamp, text);
    }
}

fn ignored_caption(total: usize) -> String {
    let mut text = ngettext(
        "{count} ignored group. It stays out of the results until you stop ignoring it.",
        "{count} ignored groups. They stay out of the results until you stop ignoring them.",
        u32::try_from(total).unwrap_or(u32::MAX),
    )
    .replace("{count}", &total.to_string());
    if total > IGNORED_GROUPS_SHOWN {
        text = format!(
            "{text} {}",
            gettext("The first {count} are shown.")
                .replace("{count}", &IGNORED_GROUPS_SHOWN.to_string())
        );
    }
    text
}
