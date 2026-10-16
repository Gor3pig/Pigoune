use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, Interaction, Options, Point, RECTANGLE, Rect, Shape,
    TEXT, Tool, estimated_text_extent, hit, hit_with_extent, number_to_value, translated,
};
use serde_json::json;

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn text_at(x: f32, y: f32, content: &str) -> Shape {
    Shape::new(
        "text",
        Geometry::Anchor(point(x, y)),
        Options::defaults(TEXT.options).with("content", json!(content)),
    )
}

#[test]
fn the_defaults_are_a_plain_red_sans_text_without_background() {
    let plain = Shape::new("text", Geometry::Anchor(point(0.0, 0.0)), Options::new());

    assert_eq!(plain.text("content"), "");
    assert!(same(plain.number("font_size"), 32.0));
    assert!(!plain.toggle("bold") && !plain.toggle("italic"));
    assert_eq!(plain.choice("family"), "sans");
    assert_eq!(plain.choice("background"), "none");
    assert_eq!(plain.color("color").r, 0xe0);
    assert!(same(plain.number("opacity"), 1.0));
}

#[test]
fn the_content_is_a_text_option_and_only_a_string_is_a_valid_content() {
    let spec = TEXT.spec("content").expect("the content option");

    assert!(spec.accepts(&json!("Bonjour")));
    assert!(spec.accepts(&json!("")));
    assert!(!spec.accepts(&json!(3)));
    assert!(!spec.accepts(&json!(true)));
    let broken = Shape::new(
        "text",
        Geometry::Anchor(point(0.0, 0.0)),
        Options::new().with("content", json!(42)),
    );
    assert_eq!(broken.text("content"), "");
}

#[test]
fn the_text_keeps_every_character_through_the_document() {
    let content = "Étape \"1\"\nligne 2 \\ 🙂 <b> 日本語\té";
    let shape = text_at(40.5, 12.25, content)
        .with_option("bold", json!(true))
        .with_option("family", json!("mono"))
        .with_option("font_size", number_to_value(48.0));
    let mut session = EditSession::new(AnnotationDocument::default());
    session.add(shape.clone());

    let text = session.document().to_json().expect("written");
    let read = AnnotationDocument::from_json(&text).expect("read");

    assert_eq!(read.objects[0].shape(), Some(&shape));
    assert_eq!(
        read.objects[0].shape().expect("text").text("content"),
        content
    );
}

#[test]
fn the_estimated_box_grows_with_the_text_the_lines_and_the_size() {
    let short = estimated_text_extent(&text_at(0.0, 0.0, "ab"));
    let long = estimated_text_extent(&text_at(0.0, 0.0, "abcdefghij"));
    let two_lines = estimated_text_extent(&text_at(0.0, 0.0, "ab\nab"));
    let big = estimated_text_extent(
        &text_at(0.0, 0.0, "ab").with_option("font_size", number_to_value(64.0)),
    );

    assert!(long.0 > short.0);
    assert!(same(long.1, short.1));
    assert!(two_lines.1 > short.1 * 1.9);
    assert!(same(two_lines.0, short.0));
    assert!(big.0 > short.0 * 1.9 && big.1 > short.1 * 1.9);
}

#[test]
fn an_empty_text_still_has_a_small_box() {
    let (width, height) = estimated_text_extent(&text_at(0.0, 0.0, ""));

    assert!(width > 0.0 && height > 0.0);
}

#[test]
fn a_text_is_hit_inside_its_box_and_not_outside() {
    let shape = text_at(100.0, 100.0, "Cliquer ici");
    let (width, height) = estimated_text_extent(&shape);

    assert!(hit(
        &shape,
        point(100.0 + width / 2.0, 100.0 + height / 2.0),
        0.0
    ));
    assert!(hit(&shape, point(100.0, 100.0), 0.0));
    assert!(!hit(&shape, point(90.0, 100.0), 0.0));
    assert!(hit(&shape, point(96.0, 100.0), 6.0));
    assert!(!hit(&shape, point(100.0 + width + 30.0, 100.0), 0.0));
    assert!(!hit(&shape, point(100.0, 100.0 + height + 30.0), 0.0));
}

#[test]
fn a_measured_box_replaces_the_estimate() {
    let shape = text_at(100.0, 100.0, "Cliquer ici");

    assert!(hit_with_extent(
        &shape,
        point(400.0, 110.0),
        0.0,
        Some((400.0, 50.0))
    ));
    assert!(!hit_with_extent(
        &shape,
        point(400.0, 110.0),
        0.0,
        Some((100.0, 50.0))
    ));
}

#[test]
fn the_session_uses_the_measure_of_the_application_for_texts_only() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let text = session.add(text_at(100.0, 100.0, "ab"));
    let rectangle = session.add(Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 600.0,
            y: 100.0,
            width: 50.0,
            height: 50.0,
        }),
        Options::defaults(RECTANGLE.options).with("fill", json!(true)),
    ));
    assert_eq!(session.hit_test(point(300.0, 110.0), 0.0), None);

    session.set_text_measurer(|_| (500.0, 60.0));

    assert_eq!(session.hit_test(point(300.0, 110.0), 0.0), Some(text));
    assert_eq!(session.hit_test(point(620.0, 120.0), 0.0), Some(rectangle));
    assert_eq!(session.hit_test(point(300.0, 400.0), 0.0), None);
}

#[test]
fn the_measure_is_asked_for_the_text_being_tested() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let short = session.add(text_at(0.0, 0.0, "ab"));
    let long = session.add(text_at(0.0, 300.0, "abcdefghij"));
    session.set_text_measurer(|shape| {
        (
            f32::from(u8::try_from(shape.text("content").len()).unwrap_or(0)) * 20.0,
            40.0,
        )
    });

    assert_eq!(session.hit_test(point(30.0, 10.0), 0.0), Some(short));
    assert_eq!(session.hit_test(point(150.0, 310.0), 0.0), Some(long));
    assert_eq!(session.hit_test(point(150.0, 10.0), 0.0), None);
}

#[test]
fn editing_the_content_is_one_undoable_change() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text_at(10.0, 10.0, "avant"));

    assert!(session.set_option_on_selected("content", json!("après")));

    assert_eq!(session.shape_of(id).expect("text").text("content"), "après");
    assert!(!session.set_option_on_selected("content", json!("après")));
    assert!(session.undo());
    assert_eq!(session.shape_of(id).expect("text").text("content"), "avant");
    assert!(session.redo());
    assert_eq!(session.shape_of(id).expect("text").text("content"), "après");
}

#[test]
fn a_text_can_be_selected_and_moved_with_the_select_tool() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text_at(100.0, 100.0, "déplace-moi"));
    session.select(None);
    let mut interaction = Interaction::new();

    interaction.press(
        &mut session,
        Tool::Select,
        &Options::new(),
        point(110.0, 110.0),
        REACH,
    );
    interaction.drag(&mut session, point(140.0, 150.0));
    interaction.release(&mut session, point(150.0, 160.0));

    assert_eq!(session.selected(), Some(id));
    assert_eq!(
        session.shape_of(id).expect("text").geometry,
        Geometry::Anchor(point(140.0, 150.0))
    );
    assert_eq!(
        translated(&text_at(1.0, 2.0, "x"), 3.0, 3.0).geometry,
        Geometry::Anchor(point(4.0, 5.0))
    );
    assert!(session.undo());
    assert_eq!(
        session.shape_of(id).expect("text").geometry,
        Geometry::Anchor(point(100.0, 100.0))
    );
}

#[test]
fn the_style_options_of_a_text_go_through_the_history() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(text_at(10.0, 10.0, "style"));

    assert!(session.set_option_on_selected("bold", json!(true)));
    assert!(session.set_option_on_selected("background", json!("dark")));
    assert!(session.shape_of(id).expect("text").toggle("bold"));
    assert_eq!(
        session.shape_of(id).expect("text").choice("background"),
        "dark"
    );
    assert!(session.undo());
    assert_eq!(
        session.shape_of(id).expect("text").choice("background"),
        "none"
    );
    assert!(session.undo());
    assert!(!session.shape_of(id).expect("text").toggle("bold"));
}
