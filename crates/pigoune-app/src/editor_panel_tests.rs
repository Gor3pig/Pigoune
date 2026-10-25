use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use pigoune_core::AssetId;
use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, Options, Rect, Shape, definition,
};

use super::PigouneEditor;
use crate::editor_tool::EditorTool;

fn shape(kind: &str, x: f32) -> Shape {
    Shape::new(
        kind,
        Geometry::Box(Rect {
            x,
            y: 10.0,
            width: 40.0,
            height: 30.0,
        }),
        Options::defaults(definition(kind).expect("tool").options),
    )
}

fn texture() -> gdk::Texture {
    let bytes = glib::Bytes::from_owned(vec![255_u8; 100 * 60 * 4]);
    gdk::MemoryTexture::new(100, 60, gdk::MemoryFormat::R8g8b8a8, &bytes, 400).upcast()
}

fn document() -> AnnotationDocument {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(shape("rectangle", 5.0));
    let blur = session.add(shape("blur", 50.0));
    session.add(shape("ellipse", 20.0));
    session.set_hidden(blur, true);
    session.set_locked(1, true);
    session.document().clone()
}

struct State {
    tool: EditorTool,
    document: String,
    modified: bool,
    can_undo: bool,
    selected: Option<u32>,
}

fn state(editor: &PigouneEditor) -> State {
    let imp = editor.imp();
    let drawing = imp.drawing.borrow();
    State {
        tool: imp.active_tool.get(),
        document: drawing.session.document().to_json().expect("saved"),
        modified: drawing.session.is_modified(),
        can_undo: drawing.session.can_undo(),
        selected: drawing.session.selected(),
    }
}

fn assert_same(before: &State, after: &State, what: &str) {
    assert_eq!(before.tool, after.tool, "{what}: the active tool");
    assert_eq!(before.document, after.document, "{what}: the annotations");
    assert_eq!(before.modified, after.modified, "{what}: the unsaved mark");
    assert_eq!(before.can_undo, after.can_undo, "{what}: the history");
    assert_eq!(before.selected, after.selected, "{what}: the selection");
}

pub fn the_panels_only_change_what_is_visible() {
    let _ = adw::init();
    let _ = gtk::gio::resources_register_include!("pigoune.gresource");
    let editor: PigouneEditor = glib::Object::new();
    let asset = AssetId::parse("00000000-0000-7000-8000-000000000001").expect("id");
    editor.open_item(asset, &texture(), document());
    editor.choose_tool(EditorTool::Rectangle);
    let imp = editor.imp();
    let before = state(&editor);

    assert!(imp.tools_grid.is_visible() && !imp.tools_hint.is_visible());
    assert!(imp.layers_scroller.is_visible() && imp.properties_scroller.is_visible());
    assert!(
        !imp.tools_toggle.is_visible(),
        "no fold button in the wide layout"
    );
    assert_same(&before, &state(&editor), "everything shown");

    let header_of = |button: &gtk::Widget| {
        button
            .ancestor(adw::HeaderBar::static_type())
            .and_downcast::<adw::HeaderBar>()
    };
    assert_eq!(
        header_of(imp.save_button.upcast_ref()).as_ref(),
        Some(&*imp.right_header)
    );
    assert!(imp.split_view.shows_sidebar());
    assert!(editor.toggle_sidebar());
    assert!(!imp.split_view.shows_sidebar(), "the sidebar is hidden");
    assert!(!imp.sidebar_toggle.is_active());
    for button in [
        imp.save_button.upcast_ref::<gtk::Widget>(),
        imp.output_button.upcast_ref(),
        imp.capture_button.upcast_ref(),
    ] {
        assert_eq!(
            header_of(button).as_ref(),
            Some(&*imp.left_header),
            "save, copy and capture stay reachable"
        );
    }
    assert!(imp.left_header.shows_end_title_buttons());
    assert_same(&before, &state(&editor), "sidebar hidden");
    assert!(editor.toggle_sidebar());
    assert!(imp.split_view.shows_sidebar() && imp.sidebar_toggle.is_active());
    assert_eq!(
        header_of(imp.save_button.upcast_ref()).as_ref(),
        Some(&*imp.right_header)
    );
    assert!(!imp.left_header.shows_end_title_buttons());
    assert_same(&before, &state(&editor), "sidebar shown again");

    editor.set_compact(true);
    assert!(
        !editor.toggle_sidebar(),
        "a narrow window has no sidebar to toggle"
    );
    assert!(!imp.split_view.shows_sidebar());
    editor.set_compact(false);
    assert!(imp.split_view.shows_sidebar());
}

pub fn hiding_an_effect_shows_nothing_and_leaves_the_canvas_alone() {
    let _ = adw::init();
    let _ = gtk::gio::resources_register_include!("pigoune.gresource");
    let editor: PigouneEditor = glib::Object::new();
    let notices = std::rc::Rc::new(std::cell::Cell::new(0_usize));
    editor.connect_notice({
        let notices = notices.clone();
        move |_| notices.set(notices.get() + 1)
    });
    let asset = AssetId::parse("00000000-0000-7000-8000-000000000002").expect("id");
    editor.open_item(asset, &texture(), document());
    let imp = editor.imp();
    let canvas = || {
        let point = imp.zoom_view.view_to_image(10.0, 10.0);
        (
            imp.zoom_view.zoom().to_bits(),
            imp.zoom_view.width(),
            imp.zoom_view.height(),
            point.x.to_bits(),
            point.y.to_bits(),
        )
    };
    let before = canvas();
    let blur = imp
        .drawing
        .borrow()
        .session
        .document()
        .exposed_masks()
        .first()
        .copied()
        .expect("the blur is hidden");
    for hidden in [false, true, false, true] {
        imp.drawing.borrow_mut().session.set_hidden(blur, hidden);
        editor.after_edit();
        assert_eq!(canvas(), before, "the canvas does not move, resize or zoom");
    }
    assert_eq!(notices.get(), 0, "hiding an effect shows no notification");
}

fn settle(duration_ms: u64) {
    let start = std::time::Instant::now();
    let context = glib::MainContext::default();
    while start.elapsed() < std::time::Duration::from_millis(duration_ms) {
        context.iteration(false);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

pub fn the_sidebar_button_stays_where_it_is() {
    let _ = adw::init();
    let _ = gtk::gio::resources_register_include!("pigoune.gresource");
    let window = gtk::Window::new();
    window.set_default_size(1200, 800);
    let editor: PigouneEditor = glib::Object::new();
    window.set_child(Some(&editor));
    let asset = AssetId::parse("00000000-0000-7000-8000-000000000003").expect("id");
    editor.open_item(asset, &texture(), document());
    window.present();
    settle(600);
    let imp = editor.imp();
    let position = || {
        let bounds = imp
            .sidebar_toggle
            .compute_bounds(&window)
            .expect("the button is laid out");
        assert!(bounds.width() > 0.0, "the button has a size");
        (bounds.x().to_bits(), bounds.y().to_bits())
    };
    let shown = position();
    assert!(
        f32::from_bits(shown.0) < 400.0,
        "the button sits with the navigation controls, not at the edge of a changing area"
    );
    assert_eq!(
        imp.sidebar_toggle.tooltip_text().as_deref(),
        Some("Hide the Sidebar (F9)")
    );
    for _ in 0..3 {
        editor.set_sidebar_hidden(true);
        settle(600);
        assert_eq!(
            position(),
            shown,
            "the button does not move when the sidebar is hidden"
        );
        assert_eq!(
            imp.sidebar_toggle.tooltip_text().as_deref(),
            Some("Show the Sidebar (F9)")
        );
        editor.set_sidebar_hidden(false);
        settle(600);
        assert_eq!(
            position(),
            shown,
            "the button does not move when the sidebar is back"
        );
    }
    window.close();
}

pub fn the_tools_the_options_and_the_layers_are_all_in_view_in_a_wide_window() {
    let _ = adw::init();
    let _ = gtk::gio::resources_register_include!("pigoune.gresource");
    let window = gtk::Window::new();
    window.set_default_size(1200, 800);
    let editor: PigouneEditor = glib::Object::new();
    window.set_child(Some(&editor));
    let asset = AssetId::parse("00000000-0000-7000-8000-000000000004").expect("id");
    editor.open_item(asset, &texture(), document());
    editor.imp().drawing.borrow_mut().session.select(Some(1));
    editor.after_edit();
    window.present();
    settle(600);
    let imp = editor.imp();
    assert!(imp.layers_scroller.height() > 40, "the layers show");
    assert!(imp.properties_scroller.height() > 40, "the options show");
    assert!(
        imp.tools_grid.is_visible() && imp.tools_grid.height() < 140,
        "the tools are a small grid that stays in view"
    );
    for (tool, button) in imp.tool_buttons.borrow().iter() {
        let icon = button
            .child()
            .and_then(|content| content.first_child())
            .expect("icon");
        let bounds = icon.compute_bounds(button).expect("bounds");
        let off_x = (bounds.x() + bounds.width() / 2.0
            - f32::from(i16::try_from(button.width()).unwrap_or(0)) / 2.0)
            .abs();
        let off_y = (bounds.y() + bounds.height() / 2.0
            - f32::from(i16::try_from(button.height()).unwrap_or(0)) / 2.0)
            .abs();
        assert!(
            off_x < 1.0 && off_y < 1.0,
            "{tool:?}: the icon is off centre by {off_x} and {off_y} in a {}x{} button",
            button.width(),
            button.height()
        );
        assert!(
            button.tooltip_text().is_some_and(|text| text.contains('(')),
            "each tool keeps its name and shortcut as a tooltip"
        );
    }
    window.close();
}
