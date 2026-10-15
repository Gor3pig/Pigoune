use std::collections::HashSet;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::ObjectSubclassIsExt;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use pigoune_core::{AssetId, ChangeStamp, IgnoreId, IgnoreKind, IgnoreOutcome, IgnoredEntry};

use crate::duplicates_page::{
    DuplicateActions, GroupState, PigouneDuplicatesPage, ResultsSnapshot,
};

const IGNORED_GROUPS_SHOWN: usize = 50;

impl PigouneDuplicatesPage {
    fn actions(&self) -> Option<Rc<DuplicateActions>> {
        self.imp().actions.borrow().clone()
    }

    fn ignored_entries(&self) -> Vec<IgnoredEntry> {
        self.actions()
            .map(|actions| (actions.ignoring.ignored_entries)())
            .unwrap_or_default()
    }

    fn ignored_count(&self) -> usize {
        self.actions()
            .map(|actions| (actions.ignoring.ignored_count)())
            .unwrap_or_default()
    }

    pub(crate) fn refresh_ignored_buttons(&self) {
        let imp = self.imp();
        let count = self
            .actions()
            .map_or(0, |actions| (actions.ignoring.ignored_asset_count)());
        let viewing = imp.viewing_ignored.get();
        let label = gettext("_Ignored ({count})").replace("{count}", &count.to_string());
        imp.ignored_button.set_use_underline(true);
        imp.ignored_button.set_label(&label);
        imp.ignored_button.set_visible(count > 0);
        imp.message_ignored_button.set_use_underline(true);
        imp.message_ignored_button.set_label(&label);
        imp.message_ignored_button
            .set_visible(!viewing && count > 0 && imp.cancel_requested.borrow().is_none());
        imp.results_bar.set_visible(!viewing);
        imp.ignored_bar.set_visible(viewing);
    }

    pub(crate) fn more_menu(label: &str, entries: Vec<(String, Box<dyn Fn()>)>) -> gtk::MenuButton {
        let menu = gio::Menu::new();
        let group = gio::SimpleActionGroup::new();
        for (index, (title, handler)) in entries.into_iter().enumerate() {
            let name = format!("entry{index}");
            let action = gio::SimpleAction::new(&name, None);
            action.connect_activate(move |_, _| handler());
            group.add_action(&action);
            menu.append(Some(&title), Some(&format!("more.{name}")));
        }
        let button = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(label)
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .menu_model(&menu)
            .build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        button.insert_action_group("more", Some(&group));
        button
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

    pub(crate) fn history_changed(&self) {
        let imp = self.imp();
        imp.last_snapshot.replace(None);
        if imp.viewing_ignored.get() {
            self.show_ignored();
        } else if imp.stack.visible_child_name().as_deref() == Some("results") {
            if self.is_mapped() {
                self.on_search_clicked();
            } else {
                self.forget_results();
            }
        } else {
            self.refresh_ignored_buttons();
        }
    }

    fn forget_results(&self) {
        let imp = self.imp();
        imp.groups.borrow_mut().clear();
        imp.selections.borrow_mut().clear();
        imp.states.borrow_mut().clear();
        self.show_idle();
    }

    pub(crate) fn show_after_undo(&self, stamp: ChangeStamp) {
        let imp = self.imp();
        if imp.viewing_ignored.get() {
            self.show_ignored();
            return;
        }
        let kept = imp.last_snapshot.take();
        match kept {
            Some((kept_stamp, snapshot)) if kept_stamp == stamp => {
                imp.groups.replace(snapshot.groups);
                imp.selections.replace(snapshot.selections);
                self.show_groups();
            }
            _ => self.on_search_clicked(),
        }
    }

    pub(crate) fn snapshot_results(&self) -> ResultsSnapshot {
        let imp = self.imp();
        ResultsSnapshot {
            groups: imp.groups.borrow().clone(),
            selections: imp
                .states
                .borrow()
                .iter()
                .map(|state| {
                    (
                        state.key,
                        (state.reference.get(), state.ticked.borrow().clone()),
                    )
                })
                .collect(),
        }
    }

    pub(crate) fn show_ignored(&self) {
        let imp = self.imp();
        let entries = self.ignored_entries();
        if entries.is_empty() {
            self.leave_ignored_view();
            return;
        }
        imp.viewing_ignored.set(true);
        let total = entries.len();
        let shown = &entries[..total.min(IGNORED_GROUPS_SHOWN)];
        let ids: Vec<AssetId> = shown
            .iter()
            .flat_map(|entry| entry.assets.iter().chain(entry.against.first()).copied())
            .collect();
        self.ensure_details(&ids);
        self.keep_selections();
        imp.states.borrow_mut().clear();
        imp.waiting.borrow_mut().clear();
        while let Some(child) = imp.groups_box.first_child() {
            imp.groups_box.remove(&child);
        }
        imp.ignored_caption_label.set_label(&ignored_caption(total));
        for entry in shown {
            imp.groups_box.append(&self.ignored_entry_widget(entry));
        }
        imp.stack.set_visible_child_name("results");
        self.follow_scrolling();
        self.load_visible_thumbnails_later();
        self.refresh_ignored_buttons();
    }

    fn ignored_entry_widget(&self, entry: &IgnoredEntry) -> adw::PreferencesGroup {
        let stop = gtk::Button::builder()
            .label(gettext("Stop Ignoring"))
            .valign(gtk::Align::Center)
            .build();
        let mut named: Vec<AssetId> = entry.assets.clone();
        if entry.kind == IgnoreKind::Member {
            named.extend(entry.against.iter().copied());
        }
        let widget = adw::PreferencesGroup::builder()
            .title(self.group_name(&named))
            .description(self.ignored_description(entry))
            .header_suffix(&stop)
            .build();
        let action = entry.action;
        stop.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| page.stop_ignoring(action)
        ));
        for id in &entry.assets {
            let Some(member) = self.imp().details.borrow().get(id).cloned() else {
                continue;
            };
            let id = member.id;
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&member.name))
                .subtitle(glib::markup_escape_text(&member.summary))
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

    fn ignored_description(&self, entry: &IgnoredEntry) -> String {
        if entry.kind == IgnoreKind::Group {
            return ngettext(
                "{count} ignored asset",
                "{count} ignored assets",
                u32::try_from(entry.assets.len()).unwrap_or(u32::MAX),
            )
            .replace("{count}", &entry.assets.len().to_string());
        }
        let details = self.imp().details.borrow();
        let name = entry
            .against
            .first()
            .and_then(|id| details.get(id))
            .map(|member| glib::markup_escape_text(&member.name).to_string())
            .unwrap_or_default();
        let others = entry.against.len().saturating_sub(1);
        if others == 0 {
            gettext("Not offered with “{name}”").replace("{name}", &name)
        } else {
            ngettext(
                "Not offered with “{name}” and {count} other asset",
                "Not offered with “{name}” and {count} other assets",
                u32::try_from(others).unwrap_or(u32::MAX),
            )
            .replace("{count}", &others.to_string())
            .replace("{name}", &name)
        }
    }

    fn stop_ignoring(&self, action: IgnoreId) {
        let Some(actions) = self.actions() else {
            return;
        };
        match (actions.ignoring.unignore)(action) {
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
        let count = self.ignored_count();
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
        let before = self.snapshot_results();
        match (actions.ignoring.ignore_group)(&state.members) {
            Ok(outcome) => {
                let covered: HashSet<AssetId> = state.members.iter().copied().collect();
                self.imp()
                    .groups
                    .borrow_mut()
                    .iter_mut()
                    .for_each(|group| group.retain(|id| !covered.contains(id)));
                self.after_ignoring(outcome, before, state, true);
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
        let before = self.snapshot_results();
        match (actions.ignoring.ignore_member)(member, &others) {
            Ok(outcome) => {
                let key = state.key;
                self.imp().groups.borrow_mut().iter_mut().for_each(|group| {
                    if group.contains(&key) {
                        group.retain(|id| *id != member);
                    }
                });
                self.after_ignoring(outcome, before, state, false);
            }
            Err(refusal) => self.show_refusal(&refusal),
        }
    }

    fn after_ignoring(
        &self,
        outcome: IgnoreOutcome,
        before: ResultsSnapshot,
        state: &Rc<GroupState>,
        group: bool,
    ) {
        let imp = self.imp();
        imp.last_snapshot.replace(Some((outcome.stamp, before)));
        self.keep_selections();
        imp.groups.borrow_mut().retain(|group| group.len() > 1);
        self.show_groups();
        let text = if outcome.covered > state.members.len() {
            ngettext(
                "{count} asset ignored",
                "{count} assets ignored",
                u32::try_from(outcome.covered).unwrap_or(u32::MAX),
            )
            .replace("{count}", &outcome.covered.to_string())
        } else if group {
            gettext("Group ignored")
        } else {
            gettext("Asset ignored")
        };
        self.offer_undo_text(outcome.stamp, &text);
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
