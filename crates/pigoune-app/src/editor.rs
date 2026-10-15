use std::cell::{Cell, RefCell};
use std::path::Path;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib, graphene, pango};
use pigoune_core::annotations::{
    CursorKind, DrawingTool, Handle, Interaction, Point, Style, Tool, restyled, style_of,
};

use crate::annotation_render;
use crate::asset_preview::zoom_text;
use crate::editor_palette;
use crate::editor_tool::EditorTool;
use crate::zoom_view::PointerEvent;

type ClosedCallback = Box<dyn Fn()>;

const TOOLS_PER_ROW: usize = 3;
const HANDLE_REACH_PIXELS: f64 = 9.0;

mod imp {
    use super::{Cell, ClosedCallback, RefCell};
    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use pigoune_core::annotations::{AnnotationDocument, EditSession, Interaction, Style};

    use crate::editor_tool::EditorTool;
    use crate::zoom_view::PigouneZoomView;

    pub struct Drawing {
        pub session: EditSession,
        pub interaction: Interaction,
        pub style: Style,
    }

    impl Default for Drawing {
        fn default() -> Self {
            Self {
                session: EditSession::new(AnnotationDocument::default()),
                interaction: Interaction::new(),
                style: Style::default(),
            }
        }
    }

    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::PigouneEditor)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/editor.ui")]
    pub struct PigouneEditor {
        #[template_child]
        pub tools_grid: TemplateChild<gtk::Grid>,
        #[template_child]
        pub panel_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub left_header: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub right_header: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub output_button: TemplateChild<gtk::MenuButton>,
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
        #[template_child]
        pub undo_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub redo_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub annotations_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub color_swatches: TemplateChild<gtk::Box>,
        #[template_child]
        pub thickness_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub opacity_scale: TemplateChild<gtk::Scale>,
        pub drawing: RefCell<Drawing>,
        pub swatches: RefCell<Vec<gtk::ToggleButton>>,
        pub updating_controls: Cell<bool>,
        pub slider_gesture: Cell<bool>,
        #[property(get, set)]
        pub compact: Cell<bool>,
        pub active_tool: Cell<EditorTool>,
        pub output_in_left_header: Cell<bool>,
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
        let imp = self.imp();
        imp.drawing.replace(imp::Drawing::default());
        imp.zoom_view
            .show_image(Some(&texture), width, height, false);
        self.refresh();
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

    #[template_callback]
    fn on_undo_clicked(&self) {
        self.imp().drawing.borrow_mut().session.undo();
        self.after_edit();
    }

    #[template_callback]
    fn on_redo_clicked(&self) {
        self.imp().drawing.borrow_mut().session.redo();
        self.after_edit();
    }

    fn prepare(&self) {
        let imp = self.imp();
        self.fill_tools_grid();
        self.fill_swatches();
        self.follow_sliders();
        self.connect_drawing();
        self.listen_to_keys();
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
                        editor.tool_changed();
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
        let compact = self.compact();
        if compact {
            imp.panel_view.set_content(gtk::Widget::NONE);
            imp.sheet_bin.set_child(Some(&panel));
        } else {
            imp.sheet_bin.set_child(gtk::Widget::NONE);
            imp.panel_view.set_content(Some(&panel));
            imp.bottom_sheet.set_open(false);
        }
        self.place_output_buttons();
    }

    fn place_output_buttons(&self) {
        let imp = self.imp();
        let save = imp.save_button.get();
        let output = imp.output_button.get();
        let compact = self.compact();
        if imp.output_in_left_header.replace(compact) == compact {
            return;
        }
        let (from, to) = if compact {
            (&*imp.right_header, &*imp.left_header)
        } else {
            (&*imp.left_header, &*imp.right_header)
        };
        from.remove(&save);
        from.remove(&output);
        to.pack_end(&save);
        to.pack_end(&output);
    }

    fn current_tool(&self) -> Tool {
        match self.imp().active_tool.get() {
            EditorTool::Select => Tool::Select,
            EditorTool::Rectangle => Tool::Draw(DrawingTool::Rectangle),
            EditorTool::Ellipse => Tool::Draw(DrawingTool::Ellipse),
            EditorTool::Line => Tool::Draw(DrawingTool::Line),
            EditorTool::Arrow => Tool::Draw(DrawingTool::Arrow),
            _ => Tool::Unsupported,
        }
    }

    fn reach(&self) -> f32 {
        let zoom = self.imp().zoom_view.zoom().max(0.01);
        point_to_f32(HANDLE_REACH_PIXELS / zoom)
    }

    fn connect_drawing(&self) {
        let imp = self.imp();
        imp.zoom_view.connect_overlay(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |snapshot, bounds, zoom| editor.draw_overlay(snapshot, bounds, zoom)
        ));
        imp.zoom_view.connect_pointer(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            #[upgrade_or]
            false,
            move |event| editor.handle_pointer(event)
        ));
    }

    fn draw_overlay(&self, snapshot: &gtk::Snapshot, bounds: &graphene::Rect, zoom: f64) {
        let drawing = self.imp().drawing.borrow();
        let scale = point_to_f32(zoom);
        snapshot.save();
        snapshot.push_clip(bounds);
        snapshot.translate(&graphene::Point::new(bounds.x(), bounds.y()));
        snapshot.scale(scale, scale);
        annotation_render::append_document(snapshot, drawing.session.document());
        if let Some(pending) = drawing.interaction.pending() {
            annotation_render::append_shape(snapshot, pending);
        }
        snapshot.pop();
        snapshot.restore();
        if drawing.interaction.pending().is_none()
            && let Some(shape) = drawing.session.selected_shape()
        {
            annotation_render::append_selection(snapshot, shape, bounds, scale);
        }
    }

    fn handle_pointer(&self, event: PointerEvent) -> bool {
        let imp = self.imp();
        let tool = self.current_tool();
        let reach = self.reach();
        let claimed = {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            match event {
                PointerEvent::Press(point) => drawing.interaction.press(
                    &mut drawing.session,
                    tool,
                    &drawing.style,
                    image_point(point),
                    reach,
                ),
                PointerEvent::Drag(point) => {
                    drawing
                        .interaction
                        .drag(&mut drawing.session, image_point(point));
                    true
                }
                PointerEvent::Release(point) => {
                    drawing
                        .interaction
                        .release(&mut drawing.session, image_point(point));
                    true
                }
                PointerEvent::Hover(point) => {
                    let kind =
                        Interaction::cursor(&drawing.session, tool, image_point(point), reach);
                    drop(guard);
                    imp.zoom_view.set_cursor_override(cursor_name(kind));
                    return false;
                }
            }
        };
        match event {
            PointerEvent::Drag(_) => imp.zoom_view.queue_draw(),
            _ => self.after_edit(),
        }
        claimed
    }

    fn after_edit(&self) {
        self.sync_controls_with_selection();
        self.refresh();
    }

    fn refresh(&self) {
        let imp = self.imp();
        let drawing = imp.drawing.borrow();
        imp.undo_button.set_sensitive(drawing.session.can_undo());
        imp.redo_button.set_sensitive(drawing.session.can_redo());
        let count = drawing.session.count();
        imp.annotations_label.set_label(&if count == 0 {
            gettext("No annotations")
        } else {
            gettext("Annotations: {count}").replace("{count}", &count.to_string())
        });
        drop(drawing);
        imp.zoom_view.queue_draw();
    }

    fn listen_to_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, modifiers| editor.handle_key(key, modifiers)
        ));
        self.add_controller(keys);
    }

    fn handle_key(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        if !self.is_mapped() {
            return glib::Propagation::Proceed;
        }
        let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let handled = match key {
            gdk::Key::z | gdk::Key::Z if control && shift => self.redo(),
            gdk::Key::z | gdk::Key::Z if control => self.undo(),
            gdk::Key::y | gdk::Key::Y if control => self.redo(),
            gdk::Key::Delete | gdk::Key::BackSpace => self.delete_selection(),
            gdk::Key::Escape => self.escape(),
            _ => false,
        };
        if handled {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    }

    fn undo(&self) -> bool {
        let done = self.imp().drawing.borrow_mut().session.undo();
        if done {
            self.after_edit();
        }
        done
    }

    fn redo(&self) -> bool {
        let done = self.imp().drawing.borrow_mut().session.redo();
        if done {
            self.after_edit();
        }
        done
    }

    fn delete_selection(&self) -> bool {
        let done = self.imp().drawing.borrow_mut().session.remove_selected();
        if done {
            self.after_edit();
        }
        done
    }

    fn escape(&self) -> bool {
        let imp = self.imp();
        let handled = {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            let active = drawing.interaction.is_active();
            drawing.interaction.cancel(&mut drawing.session);
            let selected = drawing.session.selected().is_some();
            drawing.session.select(None);
            active || selected
        };
        if handled {
            self.after_edit();
        }
        handled
    }

    fn fill_swatches(&self) {
        let imp = self.imp();
        let mut first: Option<gtk::ToggleButton> = None;
        let mut buttons = Vec::new();
        for (index, color) in editor_palette::COLORS.iter().enumerate() {
            let button = gtk::ToggleButton::new();
            button.add_css_class("annotation-swatch");
            button.add_css_class(&format!("swatch-{index}"));
            let name = editor_palette::color_name(index);
            button.set_tooltip_text(Some(&name));
            button.update_property(&[gtk::accessible::Property::Label(&name)]);
            match &first {
                Some(first) => button.set_group(Some(first)),
                None => first = Some(button.clone()),
            }
            let color = *color;
            button.connect_toggled(glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move |button| {
                    if button.is_active() {
                        editor.color_chosen(color);
                    }
                }
            ));
            imp.color_swatches.append(&button);
            buttons.push(button);
        }
        if let Some(first) = buttons.first() {
            first.set_active(true);
        }
        imp.swatches.replace(buttons);
    }

    fn follow_sliders(&self) {
        let imp = self.imp();
        for scale in [&*imp.thickness_scale, &*imp.opacity_scale] {
            scale.connect_value_changed(glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move |_| editor.style_changed()
            ));
            let click = gtk::GestureClick::new();
            click.set_propagation_phase(gtk::PropagationPhase::Capture);
            click.connect_pressed(glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move |_, _, _, _| editor.slider_pressed()
            ));
            click.connect_released(glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move |_, _, _, _| editor.slider_released()
            ));
            scale.add_controller(click);
        }
    }

    fn slider_pressed(&self) {
        let imp = self.imp();
        let mut drawing = imp.drawing.borrow_mut();
        if let Some(id) = drawing.session.selected()
            && drawing.session.begin_change(id)
        {
            imp.slider_gesture.set(true);
        }
    }

    fn slider_released(&self) {
        let imp = self.imp();
        if imp.slider_gesture.replace(false) {
            imp.drawing.borrow_mut().session.end_change();
            self.after_edit();
        }
    }

    fn color_chosen(&self, color: pigoune_core::annotations::Color) {
        if self.imp().updating_controls.get() {
            return;
        }
        let mut style = self.imp().drawing.borrow().style;
        style.color = color;
        self.apply_style(style);
    }

    fn style_changed(&self) {
        let imp = self.imp();
        if imp.updating_controls.get() {
            return;
        }
        let mut style = imp.drawing.borrow().style;
        style.width = point_to_f32(imp.thickness_scale.value());
        style.opacity = point_to_f32(imp.opacity_scale.value() / 100.0);
        self.apply_style(style);
    }

    fn apply_style(&self, style: Style) {
        let imp = self.imp();
        {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            drawing.style = style;
            if imp.slider_gesture.get() {
                if let Some(shape) = drawing.session.selected_shape() {
                    let shape = restyled(shape, &style);
                    drawing.session.preview(shape);
                }
            } else {
                drawing.session.restyle_selected(&style);
            }
        }
        self.refresh();
    }

    fn sync_controls_with_selection(&self) {
        let imp = self.imp();
        let style = {
            let drawing = imp.drawing.borrow();
            drawing.session.selected_shape().and_then(style_of)
        };
        let Some(style) = style else {
            return;
        };
        imp.updating_controls.set(true);
        imp.drawing.borrow_mut().style = style;
        imp.thickness_scale.set_value(f64::from(style.width));
        imp.opacity_scale
            .set_value(f64::from(style.opacity) * 100.0);
        if let Some(index) = editor_palette::index_of(style.color)
            && let Some(button) = imp.swatches.borrow().get(index)
        {
            button.set_active(true);
        }
        imp.updating_controls.set(false);
    }

    fn tool_changed(&self) {
        let imp = self.imp();
        {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            drawing.interaction.cancel(&mut drawing.session);
            if !matches!(self.current_tool(), Tool::Select) {
                drawing.session.select(None);
            }
        }
        imp.zoom_view.set_cursor_override(None);
        self.show_active_tool();
        imp.zoom_view.queue_draw();
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

fn image_point(point: crate::zoom_math::Point) -> Point {
    Point {
        x: point_to_f32(point.x),
        y: point_to_f32(point.y),
    }
}

fn point_to_f32(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "drawing coordinates fit easily in f32"
    )]
    let narrowed = value as f32;
    narrowed
}

fn cursor_name(kind: CursorKind) -> Option<&'static str> {
    match kind {
        CursorKind::Default => None,
        CursorKind::Crosshair => Some("crosshair"),
        CursorKind::Move => Some("move"),
        CursorKind::Resize(handle) => Some(match handle {
            Handle::TopLeft | Handle::BottomRight => "nwse-resize",
            Handle::TopRight | Handle::BottomLeft => "nesw-resize",
            Handle::Top | Handle::Bottom => "ns-resize",
            Handle::Left | Handle::Right => "ew-resize",
            Handle::Start | Handle::End => "crosshair",
        }),
    }
}
