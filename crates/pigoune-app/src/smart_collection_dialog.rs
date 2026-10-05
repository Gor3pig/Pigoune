use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use pigoune_core::AssetFilter;

type SubmitCallback = Box<dyn Fn(&str, &AssetFilter) -> Result<(), String>>;

pub struct SmartCollectionDraft<'a> {
    pub title: &'a str,
    pub confirm_label: &'a str,
    pub scope_name: &'a str,
    pub name: &'a str,
    pub filter: &'a AssetFilter,
}

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use super::SubmitCallback;
    use crate::filter_choices::PigouneFilterChoices;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/smart-collection-dialog.ui")]
    pub struct PigouneSmartCollectionDialog {
        #[template_child]
        pub name_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub scope_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub text_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub choices: TemplateChild<PigouneFilterChoices>,
        #[template_child]
        pub hint_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub confirm_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub error_label: TemplateChild<gtk::Label>,
        pub on_submit: RefCell<Option<SubmitCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneSmartCollectionDialog {
        const NAME: &'static str = "PigouneSmartCollectionDialog";
        type Type = super::PigouneSmartCollectionDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            PigouneFilterChoices::ensure_type();
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneSmartCollectionDialog {}
    impl WidgetImpl for PigouneSmartCollectionDialog {}
    impl AdwDialogImpl for PigouneSmartCollectionDialog {}
}

glib::wrapper! {
    pub struct PigouneSmartCollectionDialog(ObjectSubclass<imp::PigouneSmartCollectionDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PigouneSmartCollectionDialog {
    pub fn new(
        draft: &SmartCollectionDraft<'_>,
        on_submit: impl Fn(&str, &AssetFilter) -> Result<(), String> + 'static,
    ) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        dialog.set_title(draft.title);
        imp.confirm_button.set_label(draft.confirm_label);
        imp.scope_row.set_subtitle(draft.scope_name);
        imp.name_row.set_text(draft.name);
        imp.text_row.set_text(&draft.filter.text);
        imp.choices.choose(draft.filter);
        imp.choices.connect_changed(glib::clone!(
            #[weak]
            dialog,
            move || dialog.on_changed()
        ));
        imp.on_submit.replace(Some(Box::new(on_submit)));
        dialog.refresh_confirm_button();
        dialog.connect_map(Self::select_whole_name);
        dialog
    }

    fn select_whole_name(&self) {
        let name_row = &self.imp().name_row;
        name_row.grab_focus();
        name_row.select_region(0, -1);
    }

    fn filter(&self) -> AssetFilter {
        let imp = self.imp();
        imp.choices.chosen(imp.text_row.text().trim())
    }

    fn is_complete(&self) -> bool {
        !self.imp().name_row.text().trim().is_empty() && self.filter().narrows()
    }

    #[template_callback]
    fn on_cancel_clicked(&self) {
        self.close();
    }

    #[template_callback]
    fn on_changed(&self) {
        self.imp().error_label.set_visible(false);
        self.refresh_confirm_button();
    }

    #[template_callback]
    fn on_confirm_clicked(&self) {
        if !self.is_complete() {
            return;
        }
        let imp = self.imp();
        let filter = self.filter();
        let outcome = imp
            .on_submit
            .borrow()
            .as_ref()
            .map_or(Ok(()), |on_submit| on_submit(&imp.name_row.text(), &filter));
        match outcome {
            Ok(()) => {
                self.close();
            }
            Err(message) => {
                imp.error_label.set_label(&message);
                imp.error_label.set_visible(true);
            }
        }
    }

    fn refresh_confirm_button(&self) {
        let imp = self.imp();
        imp.confirm_button.set_sensitive(self.is_complete());
        imp.hint_label.set_visible(!self.filter().narrows());
    }
}

#[must_use]
pub fn free_name(wanted: &str, taken: &[String]) -> String {
    let is_taken = |name: &str| {
        taken
            .iter()
            .any(|existing| existing.trim().to_lowercase() == name.trim().to_lowercase())
    };
    if !is_taken(wanted) {
        return wanted.to_owned();
    }
    (2..=taken.len() + 2)
        .map(|number| format!("{wanted} {number}"))
        .find(|candidate| !is_taken(candidate))
        .unwrap_or_else(|| wanted.to_owned())
}

#[cfg(test)]
mod tests {
    use super::free_name;

    #[test]
    fn a_free_name_is_kept() {
        assert_eq!(free_name("Logos", &[]), "Logos");
    }

    #[test]
    fn a_taken_name_gets_the_first_free_number() {
        let taken = vec!["logos".to_owned(), "Logos 2".to_owned()];
        assert_eq!(free_name("Logos", &taken), "Logos 3");
    }
}
