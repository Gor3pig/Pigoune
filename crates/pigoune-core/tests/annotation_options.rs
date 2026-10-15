use std::collections::HashSet;

use pigoune_core::annotations::{
    ARROW, AnnotationDocument, Color, DASH, DefaultValue, Geometry, OptionKind, Options, Point,
    RECTANGLE, Rect, Shape, ToolDefinition, arrow_head, color_to_value, dash_pattern, definition,
    definitions, head_size_factor, number_to_value, value_to_color,
};
use serde_json::{Value, json};

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-4
}

fn rectangle(options: Options) -> Shape {
    Shape::new(
        "rectangle",
        Geometry::Box(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        options,
    )
}

fn arrow(options: Options) -> Shape {
    Shape::new(
        "arrow",
        Geometry::TwoPoints {
            from: Point { x: 0.0, y: 0.0 },
            to: Point { x: 50.0, y: 0.0 },
        },
        options,
    )
}

#[test]
fn every_tool_has_its_own_name_and_unique_options() {
    let kinds: HashSet<&str> = definitions().iter().map(|tool| tool.kind).collect();
    assert_eq!(kinds.len(), definitions().len());
    for tool in definitions() {
        let ids: HashSet<&str> = tool.options.iter().map(|spec| spec.id).collect();
        assert_eq!(ids.len(), tool.options.len(), "{}", tool.kind);
        assert_eq!(definition(tool.kind), Some(*tool));
    }
    assert!(definition("lasso").is_none());
}

#[test]
fn every_default_is_accepted_by_its_own_option() {
    for tool in definitions() {
        for spec in tool.options {
            assert!(
                spec.accepts(&spec.default_value()),
                "{} / {}",
                tool.kind,
                spec.id
            );
        }
    }
}

#[test]
fn an_option_that_depends_on_another_points_at_a_toggle_of_the_same_tool() {
    for tool in definitions() {
        for spec in tool.options {
            if let Some(controller) = spec.shown_if {
                let found = tool
                    .spec(controller)
                    .expect("the controlling option exists");
                assert_eq!(found.kind, OptionKind::Toggle, "{}", tool.kind);
            }
        }
    }
}

#[test]
fn the_tool_options_cover_the_planned_arrow_line_and_rectangle_features() {
    let ids = |tool: &ToolDefinition| -> Vec<&str> { tool.options.iter().map(|s| s.id).collect() };
    for needed in ["color", "opacity", "width", "dash", "head", "head_size"] {
        assert!(ids(&ARROW).contains(&needed), "{needed}");
    }
    for needed in ["corner_radius", "fill", "fill_opacity", "dash"] {
        assert!(ids(&RECTANGLE).contains(&needed), "{needed}");
    }
}

#[test]
fn a_missing_option_reads_as_the_default_and_a_set_one_wins() {
    let shape = arrow(Options::new());
    assert!(same(shape.number("width"), 6.0));
    assert_eq!(shape.choice("head"), "triangle");
    assert_eq!(shape.choice("head_size"), "medium");
    assert_eq!(shape.color("color").r, 0xe0);
    assert!(!rectangle(Options::new()).toggle("fill"));

    let shape = arrow(Options::new().with("width", number_to_value(12.0)));
    assert!(same(shape.number("width"), 12.0));
    let changed = shape.with_option("head", json!("double"));
    assert_eq!(changed.choice("head"), "double");
    assert_eq!(shape.choice("head"), "triangle");
}

#[test]
fn an_invalid_option_falls_back_to_the_default() {
    let shape = arrow(
        Options::new()
            .with("width", json!("wide"))
            .with("head", json!("rocket"))
            .with("color", json!("red"))
            .with("opacity", json!(7.5)),
    );
    assert!(same(shape.number("width"), 6.0));
    assert_eq!(shape.choice("head"), "triangle");
    assert_eq!(shape.color("color").r, 0xe0);
    assert!(same(shape.number("opacity"), 1.0));
}

#[test]
fn a_number_is_kept_inside_the_range_of_its_option() {
    let shape = rectangle(
        Options::new()
            .with("width", json!(9999))
            .with("corner_radius", json!(-5)),
    );
    assert!(same(shape.number("width"), 60.0));
    assert!(same(shape.number("corner_radius"), 0.0));
}

#[test]
fn the_stroke_color_follows_the_opacity_and_the_fill_only_exists_when_asked() {
    let shape = rectangle(
        Options::new()
            .with(
                "color",
                color_to_value(Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                }),
            )
            .with("opacity", json!(0.5)),
    );
    let stroke = shape.stroke_color();
    assert_eq!((stroke.r, stroke.g, stroke.b), (1, 2, 3));
    assert_eq!(stroke.a, 128);
    assert!(shape.fill_color().is_none());

    let filled = shape
        .with_option("fill", json!(true))
        .with_option("fill_opacity", json!(0.4));
    let fill = filled.fill_color().expect("a fill");
    assert_eq!((fill.r, fill.g, fill.b), (1, 2, 3));
    assert_eq!(fill.a, 102);
}

#[test]
fn a_color_travels_through_json_and_a_bad_one_is_refused() {
    let color = Color {
        r: 9,
        g: 8,
        b: 7,
        a: 6,
    };
    assert_eq!(value_to_color(&color_to_value(color)), Some(color));
    assert_eq!(value_to_color(&json!([1, 2, 3])), None);
    assert_eq!(value_to_color(&json!([1, 2, 3, 300])), None);
    assert_eq!(value_to_color(&json!("red")), None);
    assert_eq!(number_to_value(f32::NAN), Value::Null);
}

#[test]
fn defaults_list_every_option_of_a_tool() {
    let options = Options::defaults(ARROW.options);
    for spec in ARROW.options {
        assert!(options.get(spec.id).is_some(), "{}", spec.id);
    }
    assert_eq!(options.as_map().len(), ARROW.options.len());
    assert!(matches!(DASH.default, DefaultValue::Choice("solid")));
}

#[test]
fn an_old_document_without_options_reads_with_the_defaults() {
    let document = AnnotationDocument::from_json(
        r#"{"version":1,"objects":[{"id":1,"kind":"arrow",
            "from":{"x":0.0,"y":0.0},"to":{"x":40.0,"y":0.0}}]}"#,
    )
    .expect("read");
    let shape = document.objects[0].shape().expect("a known tool");
    assert!(same(shape.number("width"), 6.0));
    assert_eq!(shape.choice("head"), "triangle");
}

#[test]
fn an_option_added_by_a_newer_version_is_kept_with_the_others() {
    let stored = json!({"version":1,"objects":[{"id":1,"kind":"arrow",
        "from":{"x":0.0,"y":0.0},"to":{"x":40.0,"y":0.0},
        "options":{"width":9,"glow":{"radius":3}}}]});

    let document = AnnotationDocument::from_json(&stored.to_string()).expect("read");
    let written: Value = serde_json::from_str(&document.to_json().expect("written")).expect("json");

    assert_eq!(written["objects"][0]["options"]["glow"]["radius"], 3);
    assert_eq!(written["objects"][0]["options"]["width"], 9);
}

#[test]
fn the_head_grows_with_its_size_choice_and_unknown_choices_mean_medium() {
    assert!(head_size_factor("small") < head_size_factor("medium"));
    assert!(head_size_factor("medium") < head_size_factor("large"));
    assert!(same(
        head_size_factor("gigantic"),
        head_size_factor("medium")
    ));
    let from = Point { x: 0.0, y: 0.0 };
    let to = Point { x: 400.0, y: 0.0 };
    let small = arrow_head(from, to, 6.0, head_size_factor("small"));
    let large = arrow_head(from, to, 6.0, head_size_factor("large"));
    assert!(large.base.x < small.base.x);
}

#[test]
fn dashes_scale_with_the_width_and_solid_has_none() {
    assert_eq!(dash_pattern("solid", 6.0), None);
    assert_eq!(dash_pattern("unknown", 6.0), None);
    let thin = dash_pattern("dashed", 2.0).expect("a pattern");
    let thick = dash_pattern("dashed", 8.0).expect("a pattern");
    assert!(same(thick[0] / thin[0], 4.0));
    assert!(thin[0] > thin[1]);
    let dots = dash_pattern("dotted", 6.0).expect("a pattern");
    assert!(dots[0] < 1.0);
    assert!(dots[1] > 6.0);
}
