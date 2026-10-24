use adw::prelude::*;
use adw::subclass::prelude::ObjectSubclassIsExt;
use gettextrs::{gettext, ngettext};
use gtk::glib;
use pigoune_core::CollectionId;

use crate::collection_skipping::{self, LeftOutEntry};
use crate::duplicates_page::PigouneDuplicatesPage;

const INCLUDE_RESPONSE: &str = "include";
const CANCEL_RESPONSE: &str = "cancel";

impl PigouneDuplicatesPage {
    fn left_out_entries(&self) -> Vec<LeftOutEntry> {
        let actions = self.imp().actions.borrow().clone();
        actions.map_or_else(Vec::new, |actions| (actions.left_out)())
    }

    pub(crate) fn refresh_left_out_buttons(&self) {
        let imp = self.imp();
        let count = self.left_out_entries().len();
        let label = gettext("_Left Out ({count})").replace("{count}", &count.to_string());
        imp.left_out_button.set_use_underline(true);
        imp.left_out_button.set_label(&label);
        imp.left_out_button.set_visible(count > 0);
        imp.message_left_out_button.set_use_underline(true);
        imp.message_left_out_button.set_label(&label);
        imp.message_left_out_button.set_visible(
            !imp.viewing_ignored.get() && count > 0 && imp.cancel_requested.borrow().is_none(),
        );
    }

    pub(crate) fn show_left_out(&self) {
        let imp = self.imp();
        let entries = self.left_out_entries();
        if entries.is_empty() {
            self.leave_left_out_view();
            return;
        }
        imp.viewing_left_out.set(true);
        imp.left_out_caption_label
            .set_label(&left_out_caption(entries.len()));
        while let Some(child) = imp.left_out_list.first_child() {
            imp.left_out_list.remove(&child);
        }
        let group = adw::PreferencesGroup::new();
        for entry in &entries {
            group.add(&self.left_out_row(entry));
        }
        imp.left_out_list.append(&group);
        imp.stack.set_visible_child_name("left-out");
    }

    pub(crate) fn leave_left_out_view(&self) {
        let imp = self.imp();
        imp.viewing_left_out.set(false);
        let stale = imp.results_stale.replace(false);
        if imp.groups.borrow().is_empty() {
            self.show_idle();
        } else if stale {
            self.on_search_clicked();
        } else {
            self.show_groups();
        }
    }

    fn left_out_row(&self, entry: &LeftOutEntry) -> adw::ActionRow {
        let include = gtk::Button::builder()
            .label(collection_skipping::confirmation_button(false))
            .use_underline(true)
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&entry.name))
            .subtitle(glib::markup_escape_text(
                &collection_skipping::left_out_subtitle(entry),
            ))
            .build();
        row.add_suffix(&include);
        let id = entry.id;
        let name = entry.name.clone();
        include.connect_clicked(glib::clone!(
            #[weak(rename_to = page)]
            self,
            move |_| page.confirm_include(id, &name)
        ));
        row
    }

    fn confirm_include(&self, id: CollectionId, name: &str) {
        let dialog = adw::AlertDialog::new(
            Some(&collection_skipping::confirmation_title(name, false)),
            Some(&collection_skipping::confirmation_body(false)),
        );
        dialog.add_response(CANCEL_RESPONSE, &gettext("_Cancel"));
        dialog.add_response(
            INCLUDE_RESPONSE,
            &collection_skipping::confirmation_button(false),
        );
        dialog.set_response_appearance(INCLUDE_RESPONSE, adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some(CANCEL_RESPONSE));
        dialog.set_close_response(CANCEL_RESPONSE);
        let name = name.to_owned();
        dialog.connect_response(
            Some(INCLUDE_RESPONSE),
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                move |_, _| page.include(id, &name)
            ),
        );
        dialog.present(Some(self));
    }

    fn include(&self, id: CollectionId, name: &str) {
        let Some(actions) = self.imp().actions.borrow().clone() else {
            return;
        };
        match (actions.include_collection)(id) {
            Ok(stamp) => {
                self.imp().results_stale.set(true);
                self.show_left_out();
                let text =
                    gettext("“{name}” included in the duplicate search").replace("{name}", name);
                self.offer_undo_text(stamp, &text);
            }
            Err(refusal) => self.show_refusal(&refusal),
        }
    }
}

fn left_out_caption(total: usize) -> String {
    ngettext(
        "{count} collection is not checked for duplicates, with its sub-collections. An asset that is also in another collection stays in the search.",
        "{count} collections are not checked for duplicates, with their sub-collections. An asset that is also in another collection stays in the search.",
        u32::try_from(total).unwrap_or(u32::MAX),
    )
    .replace("{count}", &total.to_string())
}
