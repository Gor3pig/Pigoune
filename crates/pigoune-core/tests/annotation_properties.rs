use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, GeometryKind, Handle, Interaction, OptionKind,
    Options, Outline, Point, Shape, Tool, ToolDefinition, arrow_curve_control, arrow_head,
    curve_point, definition, definitions, handles, head_size_factor, number_to_value, shape_bounds,
    shape_from_anchor, shape_from_drag, shape_from_points, text_visual_bounds,
};
use serde_json::{Value, json};

const REACH: f32 = 6.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn measurer(shape: &Shape) -> (f32, f32) {
    let size = shape.number("font_size");
    let lines: Vec<&str> = shape.text("content").split('\n').collect();
    let longest = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1);
    (
        f32::from(u16::try_from(longest).unwrap_or(1)).max(1.0) * size * 0.6,
        f32::from(u16::try_from(lines.len()).unwrap_or(1)) * size * 1.3,
    )
}

fn sample(definition: &ToolDefinition) -> Shape {
    let options = Options::defaults(definition.options);
    match definition.geometry {
        GeometryKind::Box => shape_from_drag(
            definition,
            point(200.0, 200.0),
            point(500.0, 400.0),
            &options,
        ),
        GeometryKind::TwoPoints => shape_from_drag(
            definition,
            point(200.0, 200.0),
            point(500.0, 300.0),
            &options,
        ),
        GeometryKind::Polyline => {
            let path: Vec<Point> = (0..6)
                .map(|step| {
                    let x = 200.0 + 60.0 * f32::from(u8::try_from(step).unwrap_or(0));
                    point(x, if step % 2 == 0 { 200.0 } else { 260.0 })
                })
                .collect();
            shape_from_points(definition, &path, &options, false)
        }
        GeometryKind::Anchor => shape_from_anchor(definition, point(300.0, 300.0), &options),
    }
    .expect("a sample shape")
    .with_option("content", json!("Sample text"))
}

fn origin(shape: &Shape) -> Point {
    if let Geometry::Anchor(at) = &shape.geometry {
        return *at;
    }
    let bounds = shape_bounds(shape).expect("bounds");
    point(bounds.x, bounds.y)
}

fn grab_points(shape: &Shape) -> Vec<(&'static str, Point)> {
    let kind = shape.definition().expect("definition");
    match (&shape.geometry, kind.outline) {
        (Geometry::Box(rect), Outline::Rectangle) => {
            vec![("top edge", point(rect.x + rect.width / 2.0, rect.y))]
        }
        (Geometry::Box(rect), Outline::Ellipse) => {
            vec![("left tip", point(rect.x, rect.y + rect.height / 2.0))]
        }
        (Geometry::Box(rect), _) => vec![(
            "centre",
            point(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0),
        )],
        (Geometry::TwoPoints { from, to }, _) => {
            let middle = arrow_curve_control(shape).map_or_else(
                || point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
                |control| curve_point(*from, control, *to, 0.5),
            );
            let mut found = vec![("shaft", middle)];
            if shape.kind == "arrow" && shape.choice("head") != "none" {
                let head = arrow_head(
                    *from,
                    *to,
                    shape.number("width"),
                    head_size_factor(shape.choice("head_size")),
                );
                found.push((
                    "head",
                    point(
                        (head.tip.x + head.left.x + head.right.x) / 3.0,
                        (head.tip.y + head.left.y + head.right.y) / 3.0,
                    ),
                ));
            }
            found
        }
        (Geometry::Polyline(points), _) => vec![("stroke vertex", points[2])],
        (Geometry::Anchor(at), Outline::Disc) => vec![("disc centre", *at)],
        (Geometry::Anchor(_), _) => {
            let bounds = text_visual_bounds(shape, measurer(shape)).expect("text bounds");
            vec![
                (
                    "text centre",
                    point(
                        bounds.x + bounds.width / 2.0,
                        bounds.y + bounds.height / 2.0,
                    ),
                ),
                ("padded corner", point(bounds.x + 1.0, bounds.y + 1.0)),
            ]
        }
    }
}

fn values(spec: &pigoune_core::annotations::OptionSpec) -> Vec<Value> {
    match spec.kind {
        OptionKind::Color => vec![json!({"r": 20, "g": 90, "b": 200, "a": 255})],
        OptionKind::Opacity => vec![json!(0.2), json!(1.0)],
        OptionKind::Number { min, max, .. } => vec![
            number_to_value(min),
            number_to_value(f32::midpoint(min, max)),
            number_to_value(max),
        ],
        OptionKind::Choice(choices) => choices.iter().map(|choice| json!(*choice)).collect(),
        OptionKind::Toggle => vec![json!(true), json!(false)],
        OptionKind::Text => vec![json!("x"), json!("two\nlines")],
    }
}

fn session_with(shape: Shape) -> (EditSession, u32) {
    let mut session = EditSession::new(AnnotationDocument::default());
    session.set_text_measurer(measurer);
    let id = session.add(shape);
    (session, id)
}

fn drag(session: &mut EditSession, from: Point, to: Point) {
    let mut interaction = Interaction::new();
    interaction.press(session, Tool::Select, &Options::new(), from, REACH);
    interaction.drag(
        session,
        point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
    );
    interaction.release(session, to);
}

fn same(left: Point, right: Point) -> bool {
    (left.x - right.x).abs() < 0.01 && (left.y - right.y).abs() < 0.01
}

#[test]
fn every_annotation_stays_grabbable_and_movable_after_any_property_change() {
    let mut checked = 0;
    for definition in definitions() {
        for spec in definition.options {
            for value in values(spec) {
                let (mut session, id) = session_with(sample(definition));
                session.set_option_on_selected(spec.id, value.clone());
                let shape = session.shape_of(id).expect("shape").clone();
                for (label, grab) in grab_points(&shape) {
                    let context = format!("{} {} = {value} at {label}", definition.kind, spec.id);
                    session.select(None);
                    assert_eq!(session.pick(grab, REACH), Some(id), "pick: {context}");
                    let start = origin(&shape);
                    drag(&mut session, grab, point(grab.x + 20.0, grab.y + 10.0));
                    let moved = origin(session.shape_of(id).expect("shape"));
                    assert!(
                        same(moved, point(start.x + 20.0, start.y + 10.0)),
                        "move: {context}: {start:?} -> {moved:?}"
                    );
                    assert_eq!(session.selected(), Some(id), "selection: {context}");
                    assert!(session.undo(), "undo: {context}");
                    assert!(
                        same(origin(session.shape_of(id).expect("shape")), start),
                        "undo position: {context}"
                    );
                    assert!(session.redo(), "redo: {context}");
                    assert!(
                        same(origin(session.shape_of(id).expect("shape")), moved),
                        "redo position: {context}"
                    );
                    assert!(session.undo());
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 150, "only {checked} combinations were checked");
}

#[test]
fn a_text_bubble_can_be_grabbed_by_its_whole_visible_surface() {
    let (mut session, id) = session_with(sample(definition("text").expect("text")));
    session.set_option_on_selected("background", json!("bubble"));
    let shape = session.shape_of(id).expect("shape").clone();
    let bubble = text_visual_bounds(&shape, measurer(&shape)).expect("bounds");
    let plain = text_visual_bounds(
        &shape.with_option("background", json!("none")),
        measurer(&shape),
    )
    .expect("bounds");
    assert!(bubble.width > plain.width && bubble.height > plain.height);
    session.select(None);

    let in_the_padding = point(bubble.x + 2.0, bubble.y + 2.0);

    assert_eq!(session.pick(in_the_padding, 1.0), Some(id));
    session.select(Some(id));
    assert!(session.set_option_on_selected("background", json!("none")));
    session.select(None);
    assert_eq!(session.pick(in_the_padding, 1.0), None);
}

#[test]
fn a_line_an_arrow_and_a_stroke_are_not_grabbable_over_their_whole_bounding_box() {
    for kind in ["line", "arrow", "pen", "highlighter"] {
        let (mut session, id) = session_with(sample(definition(kind).expect("definition")));
        let bounds = shape_bounds(session.shape_of(id).expect("shape")).expect("bounds");
        session.select(None);
        for far in [
            point(bounds.x + 4.0, bounds.y + bounds.height - 2.0),
            point(bounds.x + bounds.width - 4.0, bounds.y + 2.0),
        ] {
            if session.pick(far, REACH).is_some() {
                continue;
            }
            session.select(Some(id));
            assert_eq!(session.pick(far, REACH), None, "{kind}: selected, {far:?}");
            session.select(None);
        }
        let middle = point(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0 + 40.0,
        );
        session.select(Some(id));
        let far_from_everything = point(bounds.x - 50.0, bounds.y - 50.0);
        assert_eq!(session.pick(far_from_everything, REACH), None, "{kind}");
        let _ = middle;
    }
}

#[test]
fn an_arrow_is_grabbed_by_its_head_wings_and_not_beside_the_head() {
    let (mut session, id) = session_with(sample(definition("arrow").expect("arrow")));
    session.set_option_on_selected("width", number_to_value(8.0));
    session.set_option_on_selected("head_size", json!("large"));
    let shape = session.shape_of(id).expect("shape").clone();
    let Geometry::TwoPoints { from, to } = shape.geometry else {
        panic!("two points")
    };
    let head = arrow_head(from, to, 8.0, head_size_factor("large"));
    session.select(None);

    let wing = point(
        f32::midpoint(head.tip.x, head.left.x),
        f32::midpoint(head.tip.y, head.left.y),
    );
    assert_eq!(session.pick(wing, 1.0), Some(id));
    let beside = point(
        head.left.x + 2.0 * (head.left.x - head.base.x),
        head.left.y + 2.0 * (head.left.y - head.base.y),
    );
    assert_eq!(session.pick(beside, 1.0), None);
}

#[test]
fn every_family_survives_the_whole_journey_from_creation_to_reopening() {
    for definition in definitions() {
        let (mut session, id) = session_with(sample(definition));
        let (toggled, value) = definition
            .options
            .iter()
            .filter(|spec| spec.id != "content")
            .find_map(|spec| {
                values(spec)
                    .into_iter()
                    .find(|value| *value != spec.default_value())
                    .map(|value| (spec, value))
            })
            .expect("every kind has an option that can change");
        session.select(None);
        let grab = grab_points(session.shape_of(id).expect("shape"))[0].1;
        session.select(Some(id));
        assert!(
            session.set_option_on_selected(toggled.id, value),
            "{}",
            definition.kind
        );
        let start = origin(session.shape_of(id).expect("shape"));

        session.select(None);
        drag(&mut session, grab, point(grab.x + 30.0, grab.y + 20.0));
        let moved = origin(session.shape_of(id).expect("shape"));
        assert!(
            same(moved, point(start.x + 30.0, start.y + 20.0)),
            "{}",
            definition.kind
        );

        let handle = handles(session.shape_of(id).expect("shape"))
            .into_iter()
            .find(|(handle, _)| !matches!(handle, Handle::Curve))
            .map(|(_, at)| at);
        if let Some(at) = handle {
            drag(&mut session, at, point(at.x + 12.0, at.y + 9.0));
        }
        let finished = session.document().clone();
        assert_eq!(session.selected(), Some(id), "{}", definition.kind);

        let mut steps = 0;
        while session.undo() {
            steps += 1;
        }
        assert!(steps >= 2, "{}: {steps} steps", definition.kind);
        assert_eq!(session.count(), 0);
        while session.redo() {}
        assert!(
            session.document().same_content_as(&finished),
            "{}",
            definition.kind
        );

        let saved = session.document().to_json().expect("serializable");
        let mut again =
            EditSession::restored(AnnotationDocument::from_json(&saved).expect("readable"));
        again.set_text_measurer(measurer);
        assert!(
            again.document().same_content_as(&finished),
            "{}",
            definition.kind
        );
        let reopened_origin = origin(again.shape_of(id).expect("shape"));
        let grab_again = grab_points(again.shape_of(id).expect("shape"))[0].1;
        again.select(None);
        drag(
            &mut again,
            grab_again,
            point(grab_again.x + 5.0, grab_again.y + 5.0),
        );
        assert!(
            same(
                origin(again.shape_of(id).expect("shape")),
                point(reopened_origin.x + 5.0, reopened_origin.y + 5.0)
            ),
            "{} after reopening",
            definition.kind
        );
    }
}
