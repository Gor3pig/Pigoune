use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

type SubmitCallback = Box<dyn Fn(&str) -> Result<(), String>>;

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::SubmitCallback;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/collection-name-dialog.ui")]
    pub struct PigouneCollectionNameDialog {
        #[template_child]
        pub name_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub confirm_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub error_label: TemplateChild<gtk::Label>,
        pub on_submit: RefCell<Option<SubmitCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneCollectionNameDialog {
        const NAME: &'static str = "PigouneCollectionNameDialog";
        type Type = super::PigouneCollectionNameDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneCollectionNameDialog {}
    impl WidgetImpl for PigouneCollectionNameDialog {}
    impl AdwDialogImpl for PigouneCollectionNameDialog {}
}

glib::wrapper! {
    pub struct PigouneCollectionNameDialog(ObjectSubclass<imp::PigouneCollectionNameDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PigouneCollectionNameDialog {
    pub fn new(
        title: &str,
        confirm_label: &str,
        name: &str,
        on_submit: impl Fn(&str) -> Result<(), String> + 'static,
    ) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        dialog.set_title(title);
        imp.confirm_button.set_label(confirm_label);
        imp.name_row.set_text(name);
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

    #[template_callback]
    fn on_cancel_clicked(&self) {
        self.close();
    }

    #[template_callback]
    fn on_name_changed(&self) {
        self.imp().error_label.set_visible(false);
        self.refresh_confirm_button();
    }

    #[template_callback]
    fn on_confirm_clicked(&self) {
        let imp = self.imp();
        if imp.name_row.text().trim().is_empty() {
            return;
        }
        let outcome = imp
            .on_submit
            .borrow()
            .as_ref()
            .map_or(Ok(()), |on_submit| on_submit(&imp.name_row.text()));
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
        imp.confirm_button
            .set_sensitive(!imp.name_row.text().trim().is_empty());
    }
}
