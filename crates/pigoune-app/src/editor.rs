use std::cell::{Cell, RefCell};
use std::path::Path;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib, graphene, pango};
use pigoune_core::AssetId;
use pigoune_core::annotations::{
    AnnotationDocument, Crop, CropEditor, CropRatio, CursorKind, EditSession, Geometry, Handle,
    Interaction, LayerMove, Options, Point, Rect, Shape, Snapping, Tool, ToolDefinition, Value,
};

use crate::annotation_export;
use crate::annotation_render;
use crate::asset_preview::zoom_text;
use crate::capture_draft;
use crate::color_sampler;
use crate::editor_options::{self, Notify, OptionEvent, OptionRow};
use crate::editor_tool::EditorTool;
use crate::image_conversion;
use crate::mask_lock;
use crate::zoom_view::PointerEvent;

type ClosedCallback = Box<dyn Fn()>;
type NoticeCallback = Box<dyn Fn(&str)>;
type SaveCallback = Box<dyn Fn(&EditedCapture)>;

pub struct EditedCapture {
    pub replaces: Option<AssetId>,
    pub rendered: Vec<u8>,
    pub original: Vec<u8>,
    pub document: AnnotationDocument,
}

const TOOLS_PER_ROW: usize = 3;
const HANDLE_REACH_PIXELS: f64 = 9.0;
const SNAP_PIXELS: f64 = 6.0;
const NUDGE_SMALL: f32 = 1.0;
const NUDGE_LARGE: f32 = 10.0;
const DUPLICATE_OFFSET: f32 = 12.0;
const TEXT_VIEW_OFFSET: (f32, f32) = (4.0, 2.0);
const TEXT_PROVIDER_PRIORITY: u32 = 800;
const DRAFT_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);

#[derive(Clone, Copy)]
pub struct TextEdit {
    pub id: Option<u32>,
    pub anchor: Point,
}

mod imp {
    use super::{
        AssetId, Cell, ClosedCallback, LayerMove, NoticeCallback, OptionRow, Point, RefCell,
        SaveCallback, Shape, TextEdit,
    };
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
        pub on_notice: RefCell<Option<NoticeCallback>>,
        pub on_save: RefCell<Option<SaveCallback>>,
        pub draft_pending: Cell<bool>,
        pub replaced_asset: Cell<Option<AssetId>>,
        pub picking_color: Cell<Option<&'static str>>,
        pub picking_pressed: Cell<bool>,
        #[template_child]
        pub text_view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub mask_banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub layer_buttons: TemplateChild<gtk::Box>,
        pub layer_move_buttons: RefCell<Vec<(LayerMove, gtk::Button)>>,
        pub locked_masks: RefCell<Vec<(u32, Shape)>>,
        pub text_edit: Cell<Option<TextEdit>>,
        pub text_provider: gtk::CssProvider,
        pub text_view_origin: Cell<[u32; 3]>,
        pub click_to_edit: Cell<Option<(u32, Point)>>,
        pub pixel_cache: crate::annotation_render::PixelCache,
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

    pub fn connect_save(&self, callback: impl Fn(&EditedCapture) + 'static) {
        self.imp().on_save.replace(Some(Box::new(callback)));
    }

    pub fn connect_notice(&self, callback: impl Fn(&str) + 'static) {
        self.imp().on_notice.replace(Some(Box::new(callback)));
    }

    pub fn resume(&self, image: &Path, document: AnnotationDocument) -> Result<(), glib::Error> {
        self.open_with(image, Some(EditSession::restored(document)), None)
    }

    pub fn open_existing(
        &self,
        asset: AssetId,
        original: &Path,
        document: AnnotationDocument,
    ) -> Result<(), glib::Error> {
        self.open_with(original, Some(EditSession::new(document)), Some(asset))
    }

    pub fn flush_draft(&self) {
        let imp = self.imp();
        if imp.zoom_view.texture().is_none() || imp.replaced_asset.get().is_some() {
            return;
        }
        let drawing = imp.drawing.borrow();
        if drawing.session.is_modified() {
            let _ = capture_draft::Draft::standard().store_document(drawing.session.document());
        }
    }

    fn keep_original_in_draft(&self) {
        if self.imp().replaced_asset.get().is_some() {
            return;
        }
        let Some(texture) = self.imp().zoom_view.texture() else {
            return;
        };
        glib::spawn_future_local(async move {
            if let Ok(png) = image_conversion::png_bytes(&texture).await {
                let _ = capture_draft::Draft::standard().store_original(&png);
            }
        });
    }

    fn schedule_draft(&self) {
        let imp = self.imp();
        if imp.draft_pending.replace(true) {
            return;
        }
        glib::timeout_add_local_once(
            DRAFT_DELAY,
            glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move || {
                    editor.imp().draft_pending.set(false);
                    editor.flush_draft();
                }
            ),
        );
    }

    pub fn open_texture(&self, texture: &gdk::Texture) {
        capture_draft::Draft::standard().clear();
        self.show_texture(texture, None, None);
        self.keep_original_in_draft();
    }

    fn open_with(
        &self,
        image: &Path,
        session: Option<EditSession>,
        replaced: Option<AssetId>,
    ) -> Result<(), glib::Error> {
        let texture = gdk::Texture::from_filename(image)?;
        self.show_texture(&texture, session, replaced);
        Ok(())
    }

    fn show_texture(
        &self,
        texture: &gdk::Texture,
        session: Option<EditSession>,
        replaced: Option<AssetId>,
    ) {
        let width = u32::try_from(texture.width()).unwrap_or(1);
        let height = u32::try_from(texture.height()).unwrap_or(1);
        let imp = self.imp();
        self.cancel_text();
        imp.pixel_cache.borrow_mut().clear();
        imp.locked_masks.take();
        let mut drawing = imp::Drawing::default();
        if let Some(session) = session {
            drawing.session = session;
        }
        imp.replaced_asset.set(replaced);
        let context = self.pango_context();
        drawing
            .session
            .set_text_measurer(move |shape| annotation_render::measure_text(&context, shape));
        imp.drawing.replace(drawing);
        imp.image_size.set((width, height));
        imp.zoom_view
            .show_image(Some(texture), width, height, false);
        self.show_session_crop();
        self.update_panel();
        self.refresh();
    }

    pub fn close(&self) {
        let imp = self.imp();
        self.cancel_text();
        imp.drawing.borrow_mut().crop_editor = None;
        imp.zoom_view.show_image(None, 1, 1, false);
    }

    #[template_callback]
    fn on_back_clicked(&self) {
        self.commit_text();
        if !self.imp().drawing.borrow().session.is_modified() {
            self.leave();
            return;
        }
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Discard the Changes?")),
            Some(&gettext(
                "The annotations and changes made to this capture will be lost.",
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("discard", &gettext("_Discard"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.connect_response(
            Some("discard"),
            glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move |_, _| editor.leave()
            ),
        );
        dialog.present(Some(self));
    }

    fn leave(&self) {
        if let Some(on_closed) = self.imp().on_closed.borrow().as_ref() {
            on_closed();
        }
    }

    #[template_callback]
    fn on_undo_clicked(&self) {
        self.commit_text();
        self.undo();
    }

    #[template_callback]
    fn on_redo_clicked(&self) {
        self.commit_text();
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
        self.install_outputs();
        self.fill_layer_buttons();
        imp.text_view.buffer().connect_changed(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| editor.sync_text_view()
        ));
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &imp.text_provider,
                TEXT_PROVIDER_PRIORITY,
            );
        }
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
            button.set_tooltip_text(Some(&format!(
                "{} ({})",
                tool.label(),
                tool.shortcut().to_ascii_uppercase()
            )));
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

    fn fill_layer_buttons(&self) {
        let imp = self.imp();
        let layers = [
            (
                LayerMove::Front,
                "go-top-symbolic",
                gettext("Bring to Front"),
            ),
            (
                LayerMove::Forward,
                "go-up-symbolic",
                gettext("Bring Forward"),
            ),
            (
                LayerMove::Backward,
                "go-down-symbolic",
                gettext("Send Backward"),
            ),
            (
                LayerMove::Back,
                "go-bottom-symbolic",
                gettext("Send to Back"),
            ),
        ];
        let mut buttons = Vec::new();
        for (movement, icon, label) in layers {
            let button = gtk::Button::from_icon_name(icon);
            button.set_tooltip_text(Some(&label));
            button.update_property(&[gtk::accessible::Property::Label(&label)]);
            button.set_sensitive(false);
            button.connect_clicked(glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move |_| {
                    editor.move_layer(movement);
                }
            ));
            imp.layer_buttons.append(&button);
            buttons.push((movement, button));
        }
        imp.layer_move_buttons.replace(buttons);
    }

    fn move_layer(&self, movement: LayerMove) -> bool {
        self.commit_text();
        let moved = self
            .imp()
            .drawing
            .borrow_mut()
            .session
            .move_selected(movement);
        if moved {
            self.after_edit();
        }
        moved
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
            (EditorTool::Text, _) => Tool::Place,
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
        let imp = self.imp();
        let drawing = imp.drawing.borrow();
        let scale = point_to_f32(zoom);
        let editing = imp.text_edit.get();
        if editing.is_some() {
            self.follow_text_origin([whole.x(), whole.y(), scale]);
        }
        snapshot.save();
        snapshot.push_clip(visible);
        snapshot.translate(&graphene::Point::new(whole.x(), whole.y()));
        snapshot.scale(scale, scale);
        let context = self.pango_context();
        let texture = imp.zoom_view.texture();
        let renderer = self.native().and_then(|native| native.renderer());
        let backdrop = texture
            .as_ref()
            .zip(renderer.as_ref())
            .map(|(texture, renderer)| annotation_render::Backdrop {
                texture,
                renderer,
                cache: &imp.pixel_cache,
            });
        annotation_render::append_document(
            snapshot,
            drawing.session.document(),
            &context,
            backdrop.as_ref(),
            editing.and_then(|edit| edit.id),
        );
        if let Some(pending) = drawing.interaction.pending() {
            let ordinal = drawing.session.document().next_ordinal(&pending.kind);
            annotation_render::append_shape(
                snapshot,
                pending,
                ordinal,
                &context,
                backdrop
                    .as_ref()
                    .map(|backdrop| annotation_render::Scene {
                        backdrop,
                        document: drawing.session.document(),
                        upto: drawing.session.document().objects.len(),
                    })
                    .as_ref(),
            );
        }
        snapshot.pop();
        snapshot.restore();
        annotation_render::append_guides(
            snapshot,
            drawing.interaction.guides(),
            whole,
            scale,
            visible_area(imp.image_size.get(), drawing.session.crop()),
        );
        if let Some(crop) = &drawing.crop_editor {
            annotation_render::append_crop(snapshot, crop, whole, scale);
        } else if drawing.interaction.pending().is_none()
            && editing.is_none()
            && let Some(shape) = drawing.session.selected_shape()
        {
            annotation_render::append_selection(snapshot, shape, whole, scale, &context);
        }
    }

    fn start_picking(&self, id: &'static str) {
        self.commit_text();
        let imp = self.imp();
        imp.picking_color.set(Some(id));
        imp.zoom_view.set_cursor_override(Some("crosshair"));
    }

    fn handle_picking_pointer(&self, event: PointerEvent) -> bool {
        let imp = self.imp();
        match event {
            PointerEvent::Press(point) => {
                imp.picking_pressed.set(true);
                if let Some(id) = imp.picking_color.take() {
                    imp.zoom_view.set_cursor_override(None);
                    let sampled = self
                        .with_backdrop(|backdrop| {
                            color_sampler::pixel_color(
                                backdrop.renderer,
                                backdrop.texture,
                                point_to_f32(point.x),
                                point_to_f32(point.y),
                            )
                        })
                        .flatten();
                    if let Some(color) = sampled {
                        self.apply_option(id, &editor_options::color_value(color));
                        self.update_panel();
                    }
                }
                true
            }
            PointerEvent::Release(_) => {
                imp.picking_pressed.set(false);
                true
            }
            PointerEvent::Drag(_) | PointerEvent::DoublePress(_) | PointerEvent::Cancel => true,
            PointerEvent::Hover(_) => {
                imp.zoom_view.set_cursor_override(Some("crosshair"));
                false
            }
        }
    }

    fn handle_pointer(&self, event: PointerEvent) -> bool {
        let imp = self.imp();
        if imp.picking_color.get().is_some() || imp.picking_pressed.get() {
            return self.handle_picking_pointer(event);
        }
        if imp.active_tool.get() == EditorTool::Crop {
            return self.handle_crop_pointer(event);
        }
        if imp.active_tool.get() == EditorTool::Text {
            return self.handle_text_pointer(event);
        }
        self.handle_object_pointer(event)
    }

    fn handle_object_pointer(&self, event: PointerEvent) -> bool {
        let imp = self.imp();
        let tool = self.current_tool();
        let reach = self.reach();
        if matches!(event, PointerEvent::Press(_)) {
            self.commit_text();
        }
        self.sync_pointer_modifiers();
        if matches!(event, PointerEvent::Press(_)) && imp.text_edit.get().is_none() {
            imp.zoom_view.grab_focus();
        }
        let claimed = {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            match event {
                PointerEvent::Press(point) => {
                    let options = match tool {
                        Tool::Draw(definition) => remembered_options(drawing, definition),
                        _ => Options::new(),
                    };
                    let before = drawing.session.selected();
                    let claimed = drawing.interaction.press(
                        &mut drawing.session,
                        tool,
                        &options,
                        image_point(point),
                        reach,
                    );
                    let after = drawing.session.selected();
                    let clicked_text = after.filter(|id| {
                        (Some(*id) == before || tool == Tool::Place)
                            && drawing
                                .session
                                .shape_of(*id)
                                .is_some_and(|shape| shape.kind == "text")
                    });
                    imp.click_to_edit
                        .set(clicked_text.map(|id| (id, image_point(point))));
                    claimed
                }
                PointerEvent::Drag(point) => {
                    if let Some((_, start)) = imp.click_to_edit.get()
                        && distance(start, image_point(point)) > reach * 0.3
                    {
                        imp.click_to_edit.set(None);
                    }
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
                PointerEvent::Cancel => {
                    drawing.interaction.cancel(&mut drawing.session);
                    true
                }
                PointerEvent::DoublePress(point) => {
                    let straightened = drawing.interaction.straighten(
                        &mut drawing.session,
                        image_point(point),
                        reach,
                    );
                    imp.click_to_edit.set(None);
                    straightened
                }
                PointerEvent::Hover(point) => {
                    let kind =
                        Interaction::cursor(&drawing.session, tool, image_point(point), reach);
                    drop(guard);
                    let name = if tool == Tool::Place && kind == CursorKind::Crosshair {
                        Some("text")
                    } else {
                        cursor_name(kind)
                    };
                    imp.zoom_view.set_cursor_override(name);
                    return false;
                }
            }
        };
        match event {
            PointerEvent::Drag(_) => imp.zoom_view.queue_draw(),
            _ => self.after_edit(),
        }
        if matches!(event, PointerEvent::Release(_))
            && let Some((id, _)) = imp.click_to_edit.take()
        {
            self.edit_text(id);
        }
        claimed
    }

    fn sync_pointer_modifiers(&self) {
        let imp = self.imp();
        let mut drawing = imp.drawing.borrow_mut();
        drawing
            .interaction
            .set_constrained(imp.zoom_view.shift_held());
        let area = visible_area(imp.image_size.get(), drawing.session.crop());
        let snapping = (!imp.zoom_view.alt_held()).then(|| Snapping {
            tolerance: point_to_f32(SNAP_PIXELS / imp.zoom_view.zoom().max(0.01)),
            area,
        });
        drawing.interaction.set_snapping(snapping);
    }

    fn handle_text_pointer(&self, event: PointerEvent) -> bool {
        let imp = self.imp();
        match event {
            PointerEvent::Press(point) => {
                if !self.handle_object_pointer(event) {
                    imp.drawing.borrow_mut().session.select(None);
                    self.begin_text_edit(None, image_point(point));
                }
                true
            }
            PointerEvent::Hover(_) => self.handle_object_pointer(event),
            _ => {
                if imp.drawing.borrow().interaction.is_active() {
                    return self.handle_object_pointer(event);
                }
                true
            }
        }
    }

    fn edit_text(&self, id: u32) {
        let anchor = {
            let drawing = self.imp().drawing.borrow();
            drawing
                .session
                .shape_of(id)
                .and_then(|shape| match shape.geometry {
                    Geometry::Anchor(at) => Some(at),
                    _ => None,
                })
        };
        if let Some(anchor) = anchor {
            self.begin_text_edit(Some(id), anchor);
        }
    }

    fn begin_text_edit(&self, id: Option<u32>, anchor: Point) {
        let imp = self.imp();
        self.commit_text();
        let content = {
            let mut drawing = imp.drawing.borrow_mut();
            drawing.session.select(id);
            id.and_then(|id| drawing.session.shape_of(id))
                .map(|shape| shape.text("content").to_owned())
                .unwrap_or_default()
        };
        let buffer = imp.text_view.buffer();
        buffer.set_text(&content);
        imp.text_edit.set(Some(TextEdit { id, anchor }));
        imp.text_view_origin.set([0; 3]);
        imp.text_view.set_visible(true);
        self.sync_text_view();
        imp.text_view.grab_focus();
        buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
        self.after_edit();
    }

    fn typed_text(&self) -> String {
        let buffer = self.imp().text_view.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    }

    fn commit_text(&self) {
        let imp = self.imp();
        let Some(edit) = imp.text_edit.take() else {
            return;
        };
        let text = self.typed_text();
        imp.text_view.set_visible(false);
        {
            let mut guard = imp.drawing.borrow_mut();
            let drawing = &mut *guard;
            let empty = text.trim().is_empty();
            match edit.id {
                Some(id) => {
                    drawing.session.select(Some(id));
                    if empty {
                        drawing.session.remove_selected();
                    } else if drawing
                        .session
                        .shape_of(id)
                        .is_some_and(|shape| shape.text("content") != text)
                    {
                        drawing
                            .session
                            .set_option_on_selected("content", Value::from(text));
                    }
                }
                None if !empty => {
                    if let Some(definition) = pigoune_core::annotations::definition("text") {
                        let options = remembered_options(drawing, definition);
                        let shape = Shape::new("text", Geometry::Anchor(edit.anchor), options)
                            .with_option("content", Value::from(text));
                        let id = drawing.session.add(shape);
                        drawing.session.select(Some(id));
                    }
                }
                None => {}
            }
        }
        self.after_edit();
    }

    fn cancel_text(&self) {
        let imp = self.imp();
        if imp.text_edit.take().is_some() {
            imp.text_view.set_visible(false);
            self.after_edit();
        }
    }

    fn edited_shape(&self) -> Option<Shape> {
        let imp = self.imp();
        let edit = imp.text_edit.get()?;
        let mut guard = imp.drawing.borrow_mut();
        let drawing = &mut *guard;
        if let Some(id) = edit.id {
            return drawing.session.shape_of(id).cloned();
        }
        let definition = pigoune_core::annotations::definition("text")?;
        Some(Shape::new(
            "text",
            Geometry::Anchor(edit.anchor),
            remembered_options(drawing, definition),
        ))
    }

    fn follow_text_origin(&self, origin: [f32; 3]) {
        let imp = self.imp();
        let bits = origin.map(f32::to_bits);
        if imp.text_view_origin.get() == bits {
            return;
        }
        imp.text_view_origin.set(bits);
        glib::idle_add_local_once(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move || editor.sync_text_view()
        ));
    }

    fn sync_text_view(&self) {
        let imp = self.imp();
        let (Some(edit), Some(shape)) = (imp.text_edit.get(), self.edited_shape()) else {
            return;
        };
        let [left, top, zoom] = imp.text_view_origin.get().map(f32::from_bits);
        let x = left + edit.anchor.x * zoom - TEXT_VIEW_OFFSET.0;
        let y = top + edit.anchor.y * zoom - TEXT_VIEW_OFFSET.1;
        imp.text_view.set_margin_start(rounded_margin(x));
        imp.text_view.set_margin_top(rounded_margin(y));
        imp.text_provider
            .load_from_string(&annotation_render::text_view_css(&shape, zoom));
        let typed = shape.with_option("content", Value::from(self.typed_text()));
        let (width, height) = annotation_render::measure_text(&self.pango_context(), &typed);
        imp.text_view.set_width_request(rounded_margin(
            width * zoom + 2.0 * TEXT_VIEW_OFFSET.0 + 4.0,
        ));
        imp.text_view
            .set_height_request(rounded_margin(height * zoom + 2.0 * TEXT_VIEW_OFFSET.1));
    }

    fn edit_selected_text(&self) -> bool {
        let id = {
            let drawing = self.imp().drawing.borrow();
            drawing.session.selected().filter(|id| {
                drawing
                    .session
                    .shape_of(*id)
                    .is_some_and(|s| s.kind == "text")
            })
        };
        id.is_some_and(|id| {
            self.edit_text(id);
            true
        })
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
                PointerEvent::DoublePress(_) => true,
                PointerEvent::Cancel => {
                    crop.cancel_gesture();
                    true
                }
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
        for (movement, button) in imp.layer_move_buttons.borrow().iter() {
            button.set_sensitive(drawing.session.can_move_selected(*movement));
        }
        let unlocked = drawing
            .session
            .document()
            .masks()
            .iter()
            .any(|mask| !imp.locked_masks.borrow().contains(mask));
        drop(drawing);
        imp.mask_banner.set_revealed(unlocked);
        imp.zoom_view.queue_draw();
        if imp.drawing.borrow().session.is_modified() {
            self.schedule_draft();
        }
    }

    #[template_callback]
    fn on_lock_masks_clicked(&self) {
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Lock the Masks?")),
            Some(&gettext(
                "The blur and the pixelation will be applied to the original image for good, and this cannot be undone. A pixelation with large cells removes the details, but a light blur may still be partly recoverable.",
            )),
        );
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("lock", &gettext("_Lock"));
        dialog.set_response_appearance("lock", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.connect_response(
            Some("lock"),
            glib::clone!(
                #[weak(rename_to = editor)]
                self,
                move |_, _| editor.lock_masks()
            ),
        );
        dialog.present(Some(self));
    }

    fn notice(&self, text: &str) {
        if let Some(on_notice) = self.imp().on_notice.borrow().as_ref() {
            on_notice(text);
        }
    }

    fn with_backdrop<R>(&self, work: impl FnOnce(&annotation_render::Backdrop) -> R) -> Option<R> {
        let imp = self.imp();
        let texture = imp.zoom_view.texture()?;
        let renderer = self.native()?.renderer()?;
        let backdrop = annotation_render::Backdrop {
            texture: &texture,
            renderer: &renderer,
            cache: &imp.pixel_cache,
        };
        Some(work(&backdrop))
    }

    fn final_texture(&self) -> Option<gdk::Texture> {
        self.commit_text();
        let drawing = self.imp().drawing.borrow();
        let context = self.pango_context();
        self.with_backdrop(|backdrop| {
            annotation_export::final_texture(backdrop, drawing.session.document(), &context)
        })
        .flatten()
    }

    fn install_outputs(&self) {
        let imp = self.imp();
        let group = gio::SimpleActionGroup::new();
        let copy = gio::SimpleAction::new("copy", None);
        copy.connect_activate(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, _| editor.copy_image()
        ));
        group.add_action(&copy);
        let export = gio::SimpleAction::new("export", None);
        export.connect_activate(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_, _| {
                glib::spawn_future_local(glib::clone!(
                    #[weak]
                    editor,
                    async move { editor.export_image().await }
                ));
            }
        ));
        group.add_action(&export);
        self.insert_action_group("editor", Some(&group));
        let menu = gio::Menu::new();
        menu.append(Some(&gettext("Copy to Clipboard")), Some("editor.copy"));
        menu.append(Some(&gettext("Export as PNG…")), Some("editor.export"));
        imp.output_button.set_menu_model(Some(&menu));
        imp.output_button.set_sensitive(true);
        imp.save_button.set_sensitive(true);
        imp.save_button.connect_clicked(glib::clone!(
            #[weak(rename_to = editor)]
            self,
            move |_| {
                glib::spawn_future_local(glib::clone!(
                    #[weak]
                    editor,
                    async move { editor.request_save().await }
                ));
            }
        ));
    }

    fn show_output_failure(&self, heading: &str) {
        let dialog = adw::AlertDialog::new(
            Some(heading),
            Some(&gettext("The image could not be created or saved.")),
        );
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(self));
    }

    fn copy_image(&self) {
        let Some(texture) = self.final_texture() else {
            self.show_output_failure(&gettext("Could Not Copy the Image"));
            return;
        };
        self.clipboard().set_texture(&texture);
        self.notice(&gettext("Image copied to the clipboard"));
    }

    async fn request_save(&self) {
        let imp = self.imp();
        let failure = gettext("Could Not Save the Capture");
        let (Some(rendered), Some(original)) = (self.final_texture(), imp.zoom_view.texture())
        else {
            self.show_output_failure(&failure);
            return;
        };
        let document = imp.drawing.borrow().session.document().clone();
        let (Ok(rendered), Ok(original)) = (
            image_conversion::png_bytes(&rendered).await,
            image_conversion::png_bytes(&original).await,
        ) else {
            self.show_output_failure(&failure);
            return;
        };
        if let Some(on_save) = imp.on_save.borrow().as_ref() {
            on_save(&EditedCapture {
                replaces: imp.replaced_asset.get(),
                rendered,
                original,
                document,
            });
        }
    }

    async fn export_image(&self) {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Export Image As"))
            .accept_label(gettext("_Export"))
            .initial_name(format!("{}.png", gettext("Capture")))
            .modal(true)
            .build();
        let window = self.root().and_downcast::<gtk::Window>();
        let Ok(file) = dialog.save_future(window.as_ref()).await else {
            return;
        };
        let Some(texture) = self.final_texture() else {
            self.show_output_failure(&gettext("Could Not Export the Image"));
            return;
        };
        let written = match image_conversion::png_bytes(&texture).await {
            Ok(bytes) => file
                .replace_contents_future(bytes, None, false, gio::FileCreateFlags::NONE)
                .await
                .is_ok(),
            Err(_) => false,
        };
        if written {
            self.notice(&gettext("Image exported"));
        } else {
            self.show_output_failure(&gettext("Could Not Export the Image"));
        }
    }

    fn lock_masks(&self) {
        let imp = self.imp();
        self.commit_text();
        let baked = {
            let drawing = imp.drawing.borrow();
            let texture = imp.zoom_view.texture();
            let renderer = self.native().and_then(|native| native.renderer());
            texture
                .as_ref()
                .zip(renderer.as_ref())
                .and_then(|(texture, renderer)| {
                    let backdrop = annotation_render::Backdrop {
                        texture,
                        renderer,
                        cache: &imp.pixel_cache,
                    };
                    mask_lock::bake(&backdrop, drawing.session.document(), &self.pango_context())
                })
        };
        let Some(baked) = baked else {
            let dialog = adw::AlertDialog::new(
                Some(&gettext("Could Not Lock the Masks")),
                Some(&gettext("The original image was left unchanged.")),
            );
            dialog.add_response("close", &gettext("_Close"));
            dialog.present(Some(self));
            return;
        };
        imp.zoom_view.replace_texture(&baked);
        self.keep_original_in_draft();
        imp.pixel_cache.borrow_mut().clear();
        imp.locked_masks
            .replace(imp.drawing.borrow().session.document().masks());
        self.refresh();
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
        if self.imp().text_edit.get().is_some() {
            return self.handle_text_key(key, control);
        }
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let handled = match key {
            gdk::Key::z | gdk::Key::Z if control && shift => self.redo(),
            gdk::Key::z | gdk::Key::Z if control => self.undo(),
            gdk::Key::y | gdk::Key::Y if control => self.redo(),
            gdk::Key::bracketright | gdk::Key::braceright if control && shift => {
                self.move_layer(LayerMove::Front)
            }
            gdk::Key::bracketleft | gdk::Key::braceleft if control && shift => {
                self.move_layer(LayerMove::Back)
            }
            gdk::Key::bracketright if control => self.move_layer(LayerMove::Forward),
            gdk::Key::bracketleft if control => self.move_layer(LayerMove::Backward),
            gdk::Key::d | gdk::Key::D if control && shift => self.duplicate_selection(),
            gdk::Key::Left | gdk::Key::Right | gdk::Key::Up | gdk::Key::Down
                if !self.is_cropping() && !self.panel_has_focus() =>
            {
                self.nudge_selection(key, shift)
            }
            gdk::Key::Delete | gdk::Key::BackSpace => self.delete_selection(),
            gdk::Key::Return | gdk::Key::KP_Enter if self.is_cropping() => {
                self.finish_cropping(true);
                true
            }
            gdk::Key::Escape if self.is_cropping() => {
                self.finish_cropping(false);
                true
            }
            gdk::Key::Return | gdk::Key::KP_Enter => self.edit_selected_text(),
            gdk::Key::Escape => self.escape(),
            _ => self.choose_tool_by_letter(key, modifiers),
        };
        if handled {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    }

    fn choose_tool_by_letter(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        let other_modifiers = gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK
            | gdk::ModifierType::META_MASK;
        if modifiers.intersects(other_modifiers) {
            return false;
        }
        let Some(tool) = key.to_unicode().and_then(EditorTool::for_shortcut) else {
            return false;
        };
        let buttons = self.imp().tool_buttons.borrow();
        let Some((_, button)) = buttons.iter().find(|(candidate, _)| *candidate == tool) else {
            return false;
        };
        button.set_active(true);
        true
    }

    fn handle_text_key(&self, key: gdk::Key, control: bool) -> glib::Propagation {
        match key {
            gdk::Key::Escape => self.cancel_text(),
            gdk::Key::Return | gdk::Key::KP_Enter if control => self.commit_text(),
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
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

    fn panel_has_focus(&self) -> bool {
        self.root()
            .and_then(|root| root.focus())
            .is_some_and(|focus| focus.is_ancestor(&*self.imp().panel))
    }

    fn nudge_selection(&self, key: gdk::Key, large: bool) -> bool {
        let step = if large { NUDGE_LARGE } else { NUDGE_SMALL };
        let (dx, dy) = match key {
            gdk::Key::Left => (-step, 0.0),
            gdk::Key::Right => (step, 0.0),
            gdk::Key::Up => (0.0, -step),
            _ => (0.0, step),
        };
        let done = self
            .imp()
            .drawing
            .borrow_mut()
            .session
            .nudge_selected(dx, dy);
        if done {
            self.after_edit();
        }
        done
    }

    fn duplicate_selection(&self) -> bool {
        let done = self
            .imp()
            .drawing
            .borrow_mut()
            .session
            .duplicate_selected(DUPLICATE_OFFSET)
            .is_some();
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
        if imp.picking_color.take().is_some() {
            imp.zoom_view.set_cursor_override(None);
            return true;
        }
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
        if self.imp().active_tool.get() == EditorTool::Text {
            return Some("text");
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
            OptionEvent::PickColor(id) => self.start_picking(id),
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
        if imp.text_edit.get().is_some() {
            self.sync_text_view();
            if !imp.slider_gesture.get() {
                imp.text_view.grab_focus();
            }
        }
    }

    fn tool_changed(&self) {
        let imp = self.imp();
        self.commit_text();
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

fn visible_area(image: (u32, u32), crop: Option<Crop>) -> Rect {
    let (x, y, width, height) = crop.map_or((0, 0, image.0, image.1), |crop| {
        (crop.x, crop.y, crop.width, crop.height)
    });
    Rect {
        x: pixels_to_f32(x),
        y: pixels_to_f32(y),
        width: pixels_to_f32(width),
        height: pixels_to_f32(height),
    }
}

fn pixels_to_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

fn distance(from: Point, to: Point) -> f32 {
    (to.x - from.x).hypot(to.y - from.y)
}

fn rounded_margin(value: f32) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "view coordinates fit easily in i32"
    )]
    let margin = value.round().max(0.0) as i32;
    margin
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
            Handle::Curve => "grab",
            Handle::Move => "move",
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
