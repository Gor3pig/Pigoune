use pigoune_core::annotations::{
    AnnotationDocument, Crop, CursorKind, EditSession, Geometry, Handle, HitMode, Interaction,
    LayerMove, Options, Point, Rect, Shape, Tool, definition, definitions, handles,
    number_to_value, shape_bounds,
};

const REACH: f32 = 9.0;
const REGION_KINDS: [&str; 2] = ["blur", "pixelate"];

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-2
}

fn drawing(kind: &str) -> Tool {
    Tool::Draw(definition(kind).expect("tool"))
}

fn boxed(kind: &str, x: f32, y: f32, width: f32, height: f32) -> Shape {
    let sheet = definition(kind).expect("tool");
    Shape::new(
        kind,
        Geometry::Box(Rect {
            x,
            y,
            width,
            height,
        }),
        Options::defaults(sheet.options),
    )
}

fn drag_with(session: &mut EditSession, tool: Tool, reach: f32, from: Point, to: Point) -> bool {
    let mut interaction = Interaction::new();
    let options = match tool {
        Tool::Draw(sheet) => Options::defaults(sheet.options),
        _ => Options::new(),
    };
    let claimed = interaction.press(session, tool, &options, from, reach);
    interaction.drag(
        session,
        point(f32::midpoint(from.x, to.x), f32::midpoint(from.y, to.y)),
    );
    interaction.release(session, to);
    claimed
}

fn drag(session: &mut EditSession, tool: Tool, from: Point, to: Point) {
    drag_with(session, tool, REACH, from, to);
}

fn bounds(session: &EditSession, id: u32) -> Rect {
    shape_bounds(session.shape_of(id).expect("shape")).expect("bounds")
}

fn at(rect: Rect) -> (f32, f32, f32, f32) {
    (rect.x, rect.y, rect.width, rect.height)
}

fn created(kind: &str) -> (EditSession, u32) {
    let mut session = EditSession::new(AnnotationDocument::default());
    drag(
        &mut session,
        drawing(kind),
        point(100.0, 100.0),
        point(300.0, 220.0),
    );
    let id = session.selected().expect("selected after creation");
    (session, id)
}

#[test]
fn scenarios_one_and_two_a_region_stays_manipulable_for_its_whole_life() {
    for kind in REGION_KINDS {
        let (mut session, id) = created(kind);
        assert_eq!(
            at(bounds(&session, id)),
            (100.0, 100.0, 200.0, 120.0),
            "{kind}"
        );

        drag(
            &mut session,
            drawing(kind),
            point(200.0, 160.0),
            point(250.0, 200.0),
        );
        assert_eq!(
            at(bounds(&session, id)),
            (150.0, 140.0, 200.0, 120.0),
            "{kind}"
        );
        assert_eq!(session.count(), 1, "{kind}");

        drag(
            &mut session,
            drawing(kind),
            point(350.0, 260.0),
            point(450.0, 300.0),
        );
        assert_eq!(
            at(bounds(&session, id)),
            (150.0, 140.0, 300.0, 160.0),
            "{kind}"
        );

        session.select(None);
        drag(
            &mut session,
            Tool::Select,
            point(300.0, 200.0),
            point(310.0, 210.0),
        );
        assert_eq!(session.selected(), Some(id), "{kind}");
        assert_eq!(
            at(bounds(&session, id)),
            (160.0, 150.0, 300.0, 160.0),
            "{kind}"
        );
        assert_eq!(session.count(), 1, "{kind}");
    }
}

#[test]
fn scenarios_three_and_four_a_region_can_be_drawn_inside_another_without_moving_it() {
    for kind in REGION_KINDS {
        let mut session = EditSession::new(AnnotationDocument::default());
        let outer = session.add(boxed(kind, 100.0, 100.0, 300.0, 200.0));

        drag(
            &mut session,
            drawing(kind),
            point(150.0, 150.0),
            point(220.0, 220.0),
        );

        assert_eq!(session.count(), 2, "{kind}");
        assert_eq!(
            at(bounds(&session, outer)),
            (100.0, 100.0, 300.0, 200.0),
            "{kind}"
        );
        let inner = session.selected().expect("the new region is selected");
        assert_ne!(inner, outer);
        assert_eq!(
            at(bounds(&session, inner)),
            (150.0, 150.0, 70.0, 70.0),
            "{kind}"
        );
    }
}

#[test]
fn scenario_five_overlapping_regions_and_a_rectangle_pick_by_stacking_order() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let blur = session.add(boxed("blur", 100.0, 100.0, 200.0, 200.0));
    let pixelate = session.add(boxed("pixelate", 200.0, 100.0, 200.0, 200.0));
    let rectangle = session.add(boxed("rectangle", 150.0, 150.0, 200.0, 200.0));
    session.select(None);

    assert_eq!(
        session.pick(point(250.0, 200.0), REACH, HitMode::Body),
        Some(pixelate)
    );
    assert_eq!(
        session.pick(point(120.0, 200.0), REACH, HitMode::Body),
        Some(blur)
    );
    assert_eq!(
        session.pick(point(150.0, 300.0), REACH, HitMode::Body),
        Some(rectangle)
    );

    session.select(Some(pixelate));
    assert!(session.move_selected(LayerMove::Back));
    session.select(None);
    assert_eq!(
        session.pick(point(250.0, 200.0), REACH, HitMode::Body),
        Some(blur)
    );

    for (id, from) in [
        (blur, point(250.0, 200.0)),
        (rectangle, point(150.0, 300.0)),
    ] {
        let before = bounds(&session, id);
        session.select(None);
        drag(
            &mut session,
            Tool::Select,
            from,
            point(from.x + 20.0, from.y + 10.0),
        );
        let after = bounds(&session, id);
        assert!(near(after.x, before.x + 20.0) && near(after.y, before.y + 10.0));
    }
    assert_eq!(session.count(), 3);
}

#[test]
fn scenario_six_after_a_crop_regions_keep_their_coordinates() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let visible = session.add(boxed("blur", 100.0, 100.0, 150.0, 100.0));
    let hidden = session.add(boxed("pixelate", 600.0, 100.0, 150.0, 100.0));
    session.select(None);
    session.set_crop(Some(Crop {
        x: 50,
        y: 50,
        width: 400,
        height: 300,
    }));

    assert_eq!(
        session.pick(point(650.0, 150.0), REACH, HitMode::Body),
        None
    );
    drag(
        &mut session,
        Tool::Select,
        point(175.0, 150.0),
        point(195.0, 170.0),
    );
    assert_eq!(at(bounds(&session, visible)), (120.0, 120.0, 150.0, 100.0));
    assert_eq!(at(bounds(&session, hidden)), (600.0, 100.0, 150.0, 100.0));

    drag(
        &mut session,
        Tool::Select,
        point(270.0, 220.0),
        point(300.0, 260.0),
    );
    assert_eq!(at(bounds(&session, visible)), (120.0, 120.0, 180.0, 140.0));
}

#[test]
fn scenario_seven_grabbing_works_at_every_zoom() {
    for reach in [36.0_f32, 9.0, 1.125] {
        for kind in REGION_KINDS {
            let mut session = EditSession::new(AnnotationDocument::default());
            let id = session.add(boxed(kind, 100.0, 100.0, 400.0, 300.0));
            session.select(None);

            drag_with(
                &mut session,
                Tool::Select,
                reach,
                point(300.0, 250.0),
                point(310.0, 250.0),
            );
            assert!(near(bounds(&session, id).x, 110.0), "{kind} at {reach}");

            session.select(None);
            drag_with(
                &mut session,
                drawing(kind),
                reach,
                point(110.0 + reach * 0.5, 250.0),
                point(110.0 + reach * 0.5 + 5.0, 250.0),
            );
            assert_eq!(session.count(), 1, "the border grabs: {kind} at {reach}");
            assert!(near(bounds(&session, id).x, 115.0), "{kind} at {reach}");

            session.select(None);
            drag_with(
                &mut session,
                drawing(kind),
                reach,
                point(115.0 + reach * 3.0, 250.0),
                point(115.0 + reach * 3.0 + 30.0, 330.0),
            );
            assert_eq!(session.count(), 2, "the inside draws: {kind} at {reach}");
        }
    }
}

#[test]
fn scenario_eight_every_step_of_the_history_is_exact() {
    for kind in REGION_KINDS {
        let (mut session, id) = created(kind);
        drag(
            &mut session,
            drawing(kind),
            point(200.0, 160.0),
            point(250.0, 200.0),
        );
        drag(
            &mut session,
            drawing(kind),
            point(350.0, 260.0),
            point(450.0, 300.0),
        );
        let moved_and_resized = at(bounds(&session, id));
        assert_eq!(moved_and_resized, (150.0, 140.0, 300.0, 160.0), "{kind}");

        assert!(session.undo());
        assert_eq!(
            at(bounds(&session, id)),
            (150.0, 140.0, 200.0, 120.0),
            "{kind}"
        );
        assert!(session.undo());
        assert_eq!(
            at(bounds(&session, id)),
            (100.0, 100.0, 200.0, 120.0),
            "{kind}"
        );
        assert!(session.redo());
        assert_eq!(
            at(bounds(&session, id)),
            (150.0, 140.0, 200.0, 120.0),
            "{kind}"
        );
        assert!(session.redo());
        assert_eq!(at(bounds(&session, id)), moved_and_resized, "{kind}");
        assert_eq!(session.count(), 1);
    }
}

#[test]
fn scenario_nine_a_saved_region_reopens_with_its_settings_and_moves_again() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let blur = session.add(boxed("blur", 100.0, 100.0, 200.0, 120.0));
    assert!(session.set_option_on_selected("radius", number_to_value(21.0)));
    let pixelate = session.add(boxed("pixelate", 400.0, 100.0, 200.0, 120.0));
    assert!(session.set_option_on_selected("cell", number_to_value(24.0)));
    drag(
        &mut session,
        drawing("pixelate"),
        point(500.0, 160.0),
        point(520.0, 180.0),
    );
    let saved = session.document().to_json().expect("serializable");

    let mut reopened =
        EditSession::restored(AnnotationDocument::from_json(&saved).expect("readable"));
    assert!(near(
        reopened.shape_of(blur).expect("blur").number("radius"),
        21.0
    ));
    assert!(near(
        reopened
            .shape_of(pixelate)
            .expect("pixelate")
            .number("cell"),
        24.0
    ));
    assert_eq!(
        at(bounds(&reopened, pixelate)),
        (420.0, 120.0, 200.0, 120.0)
    );

    drag(
        &mut reopened,
        Tool::Select,
        point(200.0, 160.0),
        point(230.0, 190.0),
    );
    drag(
        &mut reopened,
        Tool::Select,
        point(520.0, 180.0),
        point(500.0, 170.0),
    );

    assert_eq!(at(bounds(&reopened, blur)), (130.0, 130.0, 200.0, 120.0));
    assert_eq!(
        at(bounds(&reopened, pixelate)),
        (400.0, 110.0, 200.0, 120.0)
    );
    assert_eq!(reopened.count(), 2);
}

#[test]
fn scenario_ten_switching_tools_never_makes_a_region_unreachable() {
    let mut session = EditSession::new(AnnotationDocument::default());
    drag(
        &mut session,
        drawing("blur"),
        point(100.0, 100.0),
        point(300.0, 220.0),
    );
    let blur = session.selected().expect("blur");
    session.select(None);
    drag(
        &mut session,
        drawing("rectangle"),
        point(400.0, 100.0),
        point(500.0, 200.0),
    );
    session.select(None);
    drag(
        &mut session,
        Tool::Select,
        point(200.0, 160.0),
        point(230.0, 180.0),
    );
    assert_eq!(session.selected(), Some(blur));
    assert_eq!(at(bounds(&session, blur)), (130.0, 120.0, 200.0, 120.0));

    session.select(None);
    drag(
        &mut session,
        drawing("pixelate"),
        point(100.0, 400.0),
        point(300.0, 520.0),
    );
    let pixelate = session.selected().expect("pixelate");
    session.select(None);
    drag(
        &mut session,
        Tool::Select,
        point(200.0, 460.0),
        point(260.0, 480.0),
    );

    assert_eq!(session.selected(), Some(pixelate));
    assert_eq!(at(bounds(&session, pixelate)), (160.0, 420.0, 200.0, 120.0));
    assert_eq!(session.count(), 3);
}

#[test]
fn scenario_eleven_a_locked_mask_is_still_an_editable_object_that_brings_the_banner_back() {
    for kind in REGION_KINDS {
        let (mut session, id) = created(kind);
        let locked = session.document().masks();
        assert_eq!(locked.len(), 1);

        drag(
            &mut session,
            drawing(kind),
            point(200.0, 160.0),
            point(240.0, 190.0),
        );

        assert_eq!(
            at(bounds(&session, id)),
            (140.0, 130.0, 200.0, 120.0),
            "{kind}"
        );
        assert_ne!(
            session.document().masks(),
            locked,
            "a moved mask is no longer locked: {kind}"
        );
        assert!(session.undo());
        assert_eq!(session.document().masks(), locked, "{kind}");
    }
}

#[test]
fn scenario_twelve_an_interrupted_gesture_leaves_the_exact_previous_state() {
    for kind in REGION_KINDS {
        let (mut session, id) = created(kind);
        let before = session.document().to_json().expect("serializable");
        let mut interaction = Interaction::new();
        let options = Options::new();

        assert!(interaction.press(
            &mut session,
            drawing(kind),
            &options,
            point(200.0, 160.0),
            REACH
        ));
        interaction.drag(&mut session, point(500.0, 500.0));
        assert!(interaction.is_active());
        interaction.cancel(&mut session);

        assert!(!interaction.is_active());
        assert_eq!(
            session.document().to_json().expect("serializable"),
            before,
            "{kind}"
        );
        assert!(session.can_undo());

        assert!(interaction.press(
            &mut session,
            drawing(kind),
            &options,
            point(350.0, 260.0),
            REACH
        ));
        interaction.drag(&mut session, point(400.0, 300.0));
        interaction.cancel(&mut session);
        assert_eq!(
            session.document().to_json().expect("serializable"),
            before,
            "{kind}"
        );

        assert!(interaction.press(
            &mut session,
            drawing(kind),
            &options,
            point(500.0, 400.0),
            REACH
        ));
        interaction.drag(&mut session, point(600.0, 480.0));
        interaction.cancel(&mut session);
        assert_eq!(session.count(), 1, "{kind}");

        drag(
            &mut session,
            Tool::Select,
            point(200.0, 160.0),
            point(210.0, 170.0),
        );
        assert_eq!(
            at(bounds(&session, id)),
            (110.0, 110.0, 200.0, 120.0),
            "{kind}"
        );
    }
}

#[test]
fn every_region_tool_gets_the_move_handle_and_nothing_else_does() {
    for sheet in definitions() {
        let shape = Shape::new(
            sheet.kind,
            Geometry::Box(Rect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 60.0,
            }),
            Options::defaults(sheet.options),
        );
        let has_move = handles(&shape)
            .iter()
            .any(|(handle, _)| *handle == Handle::Move);
        let is_region = matches!(sheet.kind, "blur" | "pixelate" | "spotlight");
        assert_eq!(has_move, is_region, "{}", sheet.kind);
    }
}

#[test]
fn the_move_handle_sits_at_the_center_and_shows_the_move_cursor() {
    let (session, id) = created("blur");
    let grip = handles(session.shape_of(id).expect("blur"))
        .into_iter()
        .find(|(handle, _)| *handle == Handle::Move)
        .map(|(_, at)| at)
        .expect("move handle");

    assert!(near(grip.x, 200.0) && near(grip.y, 160.0));
    assert_eq!(
        Interaction::cursor(&session, drawing("blur"), grip, REACH),
        CursorKind::Resize(Handle::Move)
    );
}

#[test]
fn a_corner_handle_wins_over_the_move_handle_of_a_tiny_region() {
    let mut session = EditSession::new(AnnotationDocument::default());
    let id = session.add(boxed("blur", 100.0, 100.0, 10.0, 10.0));

    drag(
        &mut session,
        drawing("blur"),
        point(110.0, 110.0),
        point(140.0, 150.0),
    );

    assert_eq!(session.count(), 1);
    assert_eq!(at(bounds(&session, id)), (100.0, 100.0, 40.0, 50.0));
}
