use gtk::{gio, glib};

mod imp {
    use adw::subclass::prelude::*;
    use gtk::glib;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/window.ui")]
    pub struct PigouneWindow {}

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneWindow {
        const NAME: &'static str = "PigouneWindow";
        type Type = super::PigouneWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneWindow {}
    impl WidgetImpl for PigouneWindow {}
    impl WindowImpl for PigouneWindow {}
    impl ApplicationWindowImpl for PigouneWindow {}
    impl AdwApplicationWindowImpl for PigouneWindow {}
}

glib::wrapper! {
    pub struct PigouneWindow(ObjectSubclass<imp::PigouneWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl PigouneWindow {
    pub fn new(application: &adw::Application) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }
}
