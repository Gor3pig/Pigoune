use std::cell::{Cell, RefCell};
use std::path::Path;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, pango};

use crate::asset_preview::zoom_text;
use crate::editor_tool::EditorTool;

type ClosedCallback = Box<dyn Fn()>;

const TOOLS_PER_ROW: usize = 4;

mod imp {
    use super::{Cell, ClosedCallback, RefCell};
    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use crate::editor_tool::EditorTool;
    use crate::zoom_view::PigouneZoomView;

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PigouneEditor)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/editor.ui")]
    pub struct PigouneEditor {
        #[template_child]
        pub tools_grid: TemplateChild<gtk::Grid>,
        #[template_child]
        pub sidebar_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub sheet_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub panel: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub bottom_sheet: TemplateChild<adw::BottomSheet>,
        #[template_child]
        pub bar_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub bar_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub zoom_view: TemplateChild<PigouneZoomView>,
        #[template_child]
        pub style_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub zoom_out_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub zoom_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub zoom_in_button: TemplateChild<gtk::Button>,
        #[property(get, set)]
        pub compact: Cell<bool>,
        pub active_tool: Cell<EditorTool>,
        pub tool_buttons: RefCell<Vec<(EditorTool, gtk::ToggleButton)>>,
        pub on_closed: RefCell<Option<ClosedCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneEditor {
        const NAME: &'static str = "PigouneEditor";
        type Type = super::PigouneEditor;
        type ParentType = adw::Bin;

        fn class_init(class: &mut Self::Class) {
            PigouneZoomView::ensure_type();
            class.bind_template();
            super::PigouneEditor::bind_template_callbacks(class);
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for PigouneEditor {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().prepare();
        }
    }

    impl WidgetImpl for PigouneEditor {}
    impl BinImpl for PigouneEditor {}
}

glib::wrapper! {
    pub struct PigouneEditor(ObjectSubclass<imp::PigouneEditor>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl PigouneEditor {
    pub fn connect_closed(&self, callback: impl Fn() + 'static) {
        self.imp().on_closed.replace(Some(Box::new(callback)));
    }

    pub fn open(&self, image: &Path) -> Result<(), glib::Error> {
        let texture = gdk::Texture::from_filename(image)?;
        let width = u32::try_from(texture.width()).unwrap_or(1);
        let height = u32::try_from(texture.height()).unwrap_or(1);
        self.imp()
            .zoom_view
            .show_image(Some(&texture), width, height, false);
        Ok(())
    }

    pub fn close(&self) {
        self.imp().zoom_view.show_image(None, 1, 1, false);
    }

    #[template_callback]
    fn on_back_clicked(&self) {
        if let Some(on_closed) = self.imp().on_closed.borrow().as_ref() {
            on_closed();
        }
    }

    fn prepare(&self) {
        let imp = self.imp();
        self.fill_tools_grid();
        self.show_active_tool();
        self.follow_zoom();
        self.connect_compact_notify(Self::place_panel);
        imp.bar_icon
            .set_icon_name(Some(EditorTool::Select.icon_name()));
    }

    fn fill_tools_grid(&self) {
        let imp = self.imp();
        let mut first: Option<gtk::ToggleButton> = None;
        let mut buttons = Vec::new();
        for (index, tool) in EditorTool::ALL.into_iter().enumerate() {
            let button = gtk::ToggleButton::new();
            let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
            content.append(&gtk::Image::from_icon_name(tool.icon_name()));
            let label = gtk::Label::new(Some(&tool.label()));
            label.set_ellipsize(pango::EllipsizeMode::End);
            label.add_css_class("caption");
            content.append(&label);
            button.set_child(Some(&content));
            button.set_tooltip_text(Some(&tool.label()));
            match &first {
                Some(first) => button.set_group(Some(first)),
                None => first = Some(button.clone()),
            }
            button.set_active(tool == EditorTool::default());
            button.connect_toggled(glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move |button| {
                    if button.is_active() {
                        editor.imp().active_tool.set(tool);
                        editor.show_active_tool();
                    }
                }
            ));
            let column = i32::try_from(index % TOOLS_PER_ROW).unwrap_or(0);
            let row = i32::try_from(index / TOOLS_PER_ROW).unwrap_or(0);
            imp.tools_grid.attach(&button, column, row, 1, 1);
            buttons.push((tool, button));
        }
        imp.tool_buttons.replace(buttons);
    }

    fn place_panel(&self) {
        let imp = self.imp();
        let panel = imp.panel.get();
        let (from, to) = if self.compact() {
            (&*imp.sidebar_bin, &*imp.sheet_bin)
        } else {
            (&*imp.sheet_bin, &*imp.sidebar_bin)
        };
        if from.child().is_some_and(|child| child == panel) {
            from.set_child(gtk::Widget::NONE);
            to.set_child(Some(&panel));
        }
        if !self.compact() {
            imp.bottom_sheet.set_open(false);
        }
    }

    fn show_active_tool(&self) {
        let imp = self.imp();
        let tool = imp.active_tool.get();
        imp.style_group.set_title(&tool.label());
        imp.bar_icon.set_icon_name(Some(tool.icon_name()));
        imp.bar_label.set_label(&tool.label());
    }

    fn follow_zoom(&self) {
        let imp = self.imp();
        imp.zoom_button.set_label(&zoom_text(1.0));
        imp.zoom_view.connect_zoom_changed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |zoom| editor.imp().zoom_button.set_label(&zoom_text(zoom))
        ));
        imp.zoom_out_button.connect_clicked(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.imp().zoom_view.zoom_out()
        ));
        imp.zoom_in_button.connect_clicked(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.imp().zoom_view.zoom_in()
        ));
        imp.zoom_button.connect_clicked(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.imp().zoom_view.fit_to_view()
        ));
    }
}
