use std::cell::{Cell, RefCell};
use std::path::Path;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib, graphene, pango};
use pigoune_core::annotations::{
    CropEditor, CropRatio, CursorKind, Handle, Interaction, Options, Point, Shape, Tool,
    ToolDefinition, Value,
};

use crate::annotation_render;
use crate::asset_preview::zoom_text;
use crate::editor_options::{self, Notify, OptionEvent, OptionRow};
use crate::editor_tool::EditorTool;
use crate::zoom_view::PointerEvent;

type ClosedCallback = Box<dyn Fn()>;

const TOOLS_PER_ROW: usize = 3;
const HANDLE_REACH_PIXELS: f64 = 9.0;

mod imp {
    use super::{Cell, ClosedCallback, OptionRow, RefCell};
    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    use std::collections::HashMap;

    use pigoune_core::annotations::{
        AnnotationDocument, CropEditor, EditSession, Interaction, Options,
    };

    use crate::editor_tool::EditorTool;
    use crate::zoom_view::PigouneZoomView;

    pub struct Drawing {
        pub session: EditSession,
        pub interaction: Interaction,
        pub tool_options: HashMap<&'static str, Options>,
        pub crop_editor: Option<CropEditor>,
    }

    impl Default for Drawing {
        fn default() -> Self {
            Self {
                session: EditSession::new(AnnotationDocument::default()),
                interaction: Interaction::new(),
                tool_options: HashMap::new(),
                crop_editor: None,
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
        pub crop_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub crop_ratio_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub crop_size_row: TemplateChild<adw::ActionRow>,
        pub image_size: Cell<(u32, u32)>,
        pub drawing: RefCell<Drawing>,
        pub option_rows: RefCell<Vec<OptionRow>>,
        pub panel_kind: Cell<Option<&'static str>>,
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
        imp.image_size.set((width, height));
        imp.zoom_view
            .show_image(Some(&texture), width, height, false);
        self.update_panel();
        self.refresh();
        Ok(())
    }

    pub fn close(&self) {
        let imp = self.imp();
        imp.drawing.borrow_mut().crop_editor = None;
        imp.zoom_view.show_image(None, 1, 1, false);
    }

    #[template_callback]
    fn on_back_clicked(&self) {
        if let Some(on_closed) = self.imp().on_closed.borrow().as_ref() {
            on_closed();
        }
    }

    #[template_callback]
    fn on_undo_clicked(&self) {
        self.undo();
    }

    #[template_callback]
    fn on_redo_clicked(&self) {
        self.redo();
    }

    fn prepare(&self) {
        let imp = self.imp();
        self.fill_tools_grid();
        self.connect_drawing();
        imp.crop_ratio_row.connect_selected_notify(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.change_crop_ratio()
        ));
        self.listen_to_keys();
        self.show_active_tool();
        self.update_panel();
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
        let tool = self.imp().active_tool.get();
        match (tool, tool.definition()) {
            (EditorTool::Select, _) => Tool::Select,
            (_, Some(definition)) => Tool::Draw(definition),
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
            move |snapshot, visible, whole, zoom| editor
                .draw_overlay(snapshot, visible, whole, zoom)
        ));
        imp.zoom_view.connect_pointer(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            #[upgrade_or]
            false,
            move |event| editor.handle_pointer(event)
        ));
    }

    fn draw_overlay(
        &self,
        snapshot: &gtk::Snapshot,
        visible: &graphene::Rect,
        whole: &graphene::Rect,
        zoom: f64,
    ) {
        let drawing = self.imp().drawing.borrow();
        let scale = point_to_f32(zoom);
        snapshot.save();
        snapshot.push_clip(visible);
        snapshot.translate(&graphene::Point::new(whole.x(), whole.y()));
        snapshot.scale(scale, scale);
        let context = self.pango_context();
        annotation_render::append_document(snapshot, drawing.session.document(), &context);
        if let Some(pending) = drawing.interaction.pending() {
            let ordinal = drawing.session.document().next_ordinal(&pending.kind);
            annotation_render::append_shape(snapshot, pending, ordinal, &context);
        }
        snapshot.pop();
        snapshot.restore();
        if let Some(crop) = &drawing.crop_editor {
            annotation_render::append_crop(snapshot, crop, whole, scale);
        } else if drawing.interaction.pending().is_none()
            && let Some(shape) = drawing.session.selected_shape()
        {
            annotation_render::append_selection(snapshot, shape, whole, scale);
        }
    }

    fn handle_pointer(&self, event: PointerEvent) -> bool {
        let imp = self.imp();
        if imp.active_tool.get() == EditorTool::Crop {
            return self.handle_crop_pointer(event);
        }
        let tool = self.current_tool();
        let reach = self.reach();
        let claimed = {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            match event {
                PointerEvent::Press(point) => {
                    let options = match tool {
                        Tool::Draw(definition) => remembered_options(drawing, definition),
                        _ => Options::new(),
                    };
                    drawing.interaction.press(
                        &mut drawing.session,
                        tool,
                        &options,
                        image_point(point),
                        reach,
                    )
                }
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

    fn handle_crop_pointer(&self, event: PointerEvent) -> bool {
        let imp = self.imp();
        let reach = self.reach();
        let claimed = {
            let mut drawing = imp.drawing.borrow_mut();
            let Some(crop) = drawing.crop_editor.as_mut() else {
                return false;
            };
            match event {
                PointerEvent::Press(point) => crop.press(image_point(point), reach),
                PointerEvent::Drag(point) => {
                    crop.drag(image_point(point));
                    true
                }
                PointerEvent::Release(point) => {
                    crop.release(image_point(point));
                    true
                }
                PointerEvent::Hover(point) => {
                    let kind = crop.cursor(image_point(point), reach);
                    drop(drawing);
                    imp.zoom_view.set_cursor_override(cursor_name(kind));
                    return false;
                }
            }
        };
        self.sync_crop_panel();
        imp.zoom_view.queue_draw();
        claimed
    }

    fn start_cropping(&self) {
        let imp = self.imp();
        let (width, height) = imp.image_size.get();
        {
            let mut drawing = imp.drawing.borrow_mut();
            drawing.session.select(None);
            let current = drawing.session.crop();
            drawing.crop_editor = Some(CropEditor::new(width, height, current));
        }
        imp.zoom_view.set_crop(None);
    }

    fn stop_cropping(&self) {
        let imp = self.imp();
        imp.drawing.borrow_mut().crop_editor = None;
        self.show_session_crop();
    }

    fn show_session_crop(&self) {
        let imp = self.imp();
        let crop = imp.drawing.borrow().session.crop();
        imp.zoom_view.set_crop(crop.map(|crop| {
            (
                f64::from(crop.x),
                f64::from(crop.y),
                f64::from(crop.width),
                f64::from(crop.height),
            )
        }));
    }

    fn sync_crop_panel(&self) {
        let imp = self.imp();
        let drawing = imp.drawing.borrow();
        let Some(crop) = &drawing.crop_editor else {
            return;
        };
        let (width, height) = crop.size();
        let text = gettext("{width} × {height} px")
            .replace("{width}", &width.to_string())
            .replace("{height}", &height.to_string());
        let index = CropRatio::ALL
            .iter()
            .position(|ratio| *ratio == crop.ratio())
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0);
        drop(drawing);
        imp.crop_size_row.set_subtitle(&text);
        imp.updating_controls.set(true);
        imp.crop_ratio_row.set_selected(index);
        imp.updating_controls.set(false);
    }

    fn reload_crop_editor(&self) {
        let imp = self.imp();
        let mut guard = imp.drawing.borrow_mut();
        let drawing = &mut *guard;
        let current = drawing.session.crop();
        if let Some(crop) = drawing.crop_editor.as_mut() {
            crop.load(current);
        }
    }

    fn finish_cropping(&self, apply: bool) {
        let imp = self.imp();
        {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            if let Some(crop) = drawing.crop_editor.as_mut() {
                if apply {
                    drawing.session.set_crop(crop.crop());
                } else {
                    crop.discard();
                }
            }
        }
        self.select_tool(EditorTool::Select);
        self.after_edit();
    }

    fn select_tool(&self, tool: EditorTool) {
        let imp = self.imp();
        if let Some((_, button)) = imp
            .tool_buttons
            .borrow()
            .iter()
            .find(|(candidate, _)| *candidate == tool)
        {
            button.set_active(true);
        }
    }

    fn change_crop_ratio(&self) {
        let imp = self.imp();
        if imp.updating_controls.get() {
            return;
        }
        let Some(ratio) = usize::try_from(imp.crop_ratio_row.selected())
            .ok()
            .and_then(|index| CropRatio::ALL.get(index).copied())
        else {
            return;
        };
        if let Some(crop) = imp.drawing.borrow_mut().crop_editor.as_mut() {
            crop.set_ratio(ratio);
        }
        self.sync_crop_panel();
        imp.zoom_view.queue_draw();
    }

    #[template_callback]
    fn on_crop_apply_clicked(&self) {
        self.finish_cropping(true);
    }

    #[template_callback]
    fn on_crop_cancel_clicked(&self) {
        self.finish_cropping(false);
    }

    #[template_callback]
    fn on_crop_reset_clicked(&self) {
        if let Some(crop) = self.imp().drawing.borrow_mut().crop_editor.as_mut() {
            crop.reset();
        }
        self.sync_crop_panel();
        self.imp().zoom_view.queue_draw();
    }

    fn after_edit(&self) {
        self.update_panel();
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
            gdk::Key::Return | gdk::Key::KP_Enter if self.is_cropping() => {
                self.finish_cropping(true);
                true
            }
            gdk::Key::Escape if self.is_cropping() => {
                self.finish_cropping(false);
                true
            }
            gdk::Key::Escape => self.escape(),
            _ => false,
        };
        if handled {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    }

    fn is_cropping(&self) -> bool {
        self.imp().drawing.borrow().crop_editor.is_some()
    }

    fn undo(&self) -> bool {
        let done = self.imp().drawing.borrow_mut().session.undo();
        if done {
            self.after_history_move();
        }
        done
    }

    fn redo(&self) -> bool {
        let done = self.imp().drawing.borrow_mut().session.redo();
        if done {
            self.after_history_move();
        }
        done
    }

    fn after_history_move(&self) {
        if self.imp().drawing.borrow().crop_editor.is_some() {
            self.reload_crop_editor();
        } else {
            self.show_session_crop();
        }
        self.after_edit();
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

    fn notifier(&self) -> Notify {
        let editor = self.downgrade();
        std::rc::Rc::new(move |event| {
            if let Some(editor) = editor.upgrade() {
                editor.option_event(event);
            }
        })
    }

    fn target_kind(&self) -> Option<&'static str> {
        if self.imp().active_tool.get() == EditorTool::Crop {
            return None;
        }
        let drawing = self.imp().drawing.borrow();
        if let Some(shape) = drawing.session.selected_shape() {
            return shape.definition().map(|definition| definition.kind);
        }
        match self.current_tool() {
            Tool::Draw(definition) => Some(definition.kind),
            _ => None,
        }
    }

    fn target_options(&self, definition: &'static ToolDefinition) -> Options {
        let mut drawing = self.imp().drawing.borrow_mut();
        match drawing.session.selected_shape() {
            Some(shape) if shape.kind == definition.kind => complete_options(definition, shape),
            _ => remembered_options(&mut drawing, definition),
        }
    }

    fn update_panel(&self) {
        let imp = self.imp();
        let kind = self.target_kind();
        let definition = kind.and_then(pigoune_core::annotations::definition);
        if imp.panel_kind.get() != kind {
            for row in imp.option_rows.take() {
                imp.style_group.remove(&row.widget);
            }
            imp.panel_kind.set(kind);
            if let Some(definition) = definition {
                let rows = editor_options::build_rows(definition, &self.notifier());
                for row in &rows {
                    imp.style_group.add(&row.widget);
                }
                imp.option_rows.replace(rows);
            }
        }
        let cropping = imp.active_tool.get() == EditorTool::Crop;
        imp.crop_group.set_visible(cropping);
        imp.style_group
            .set_visible(definition.is_some() && !cropping);
        if cropping {
            self.sync_crop_panel();
        }
        let Some(definition) = definition else {
            return;
        };
        if let Some(tool) = kind.and_then(EditorTool::from_kind) {
            imp.style_group.set_title(&tool.label());
        }
        let options = self.target_options(definition);
        imp.updating_controls.set(true);
        editor_options::show_values(&imp.option_rows.borrow(), &options);
        imp.updating_controls.set(false);
    }

    fn option_event(&self, event: OptionEvent) {
        let imp = self.imp();
        if imp.updating_controls.get() {
            return;
        }
        match event {
            OptionEvent::SliderPressed => {
                let mut drawing = imp.drawing.borrow_mut();
                if let Some(id) = drawing.session.selected()
                    && drawing.session.begin_change(id)
                {
                    imp.slider_gesture.set(true);
                }
            }
            OptionEvent::SliderReleased => {
                if imp.slider_gesture.replace(false) {
                    imp.drawing.borrow_mut().session.end_change();
                    self.after_edit();
                }
            }
            OptionEvent::Changed(id, value) => self.apply_option(id, &value),
        }
    }

    fn apply_option(&self, id: &str, value: &Value) {
        let imp = self.imp();
        let Some(kind) = imp.panel_kind.get() else {
            return;
        };
        let Some(definition) = pigoune_core::annotations::definition(kind) else {
            return;
        };
        {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            drawing
                .tool_options
                .entry(definition.kind)
                .or_insert_with(|| Options::defaults(definition.options))
                .set(id, value.clone());
            let selected_kind = drawing
                .session
                .selected_shape()
                .map(|shape| shape.kind.clone());
            if selected_kind.as_deref() == Some(kind) {
                if imp.slider_gesture.get() {
                    drawing.session.preview_option(id, value.clone());
                } else {
                    drawing.session.set_option_on_selected(id, value.clone());
                }
            }
        }
        let options = self.target_options(definition);
        editor_options::update_visibility(&imp.option_rows.borrow(), &options);
        self.refresh();
    }

    fn tool_changed(&self) {
        let imp = self.imp();
        let cropping = imp.active_tool.get() == EditorTool::Crop;
        let was_cropping = imp.drawing.borrow().crop_editor.is_some();
        {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            drawing.interaction.cancel(&mut drawing.session);
            if !matches!(self.current_tool(), Tool::Select) {
                drawing.session.select(None);
            }
        }
        if cropping && !was_cropping {
            self.start_cropping();
        } else if !cropping && was_cropping {
            self.stop_cropping();
        }
        imp.zoom_view.set_cursor_override(None);
        self.show_active_tool();
        self.update_panel();
        imp.zoom_view.queue_draw();
    }

    fn show_active_tool(&self) {
        let imp = self.imp();
        let tool = imp.active_tool.get();
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

fn remembered_options(drawing: &mut imp::Drawing, definition: &'static ToolDefinition) -> Options {
    drawing
        .tool_options
        .entry(definition.kind)
        .or_insert_with(|| Options::defaults(definition.options))
        .clone()
}

fn complete_options(definition: &ToolDefinition, shape: &Shape) -> Options {
    let mut options = Options::defaults(definition.options);
    for (id, value) in shape.options.as_map() {
        options.set(id, value.clone());
    }
    options
}
