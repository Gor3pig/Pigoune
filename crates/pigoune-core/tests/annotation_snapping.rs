use pigoune_core::annotations::{
    AnnotationDocument, EditSession, Geometry, Guide, Interaction, Options, Point, RECTANGLE, Rect,
    Shape, Snapping, Tool, snap_move,
};

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

fn space(tolerance: f32) -> Snapping {
    Snapping {
        tolerance,
        image: (1000.0, 600.0),
    }
}

#[test]
fn nothing_close_enough_means_no_shift_and_no_guide() {
    let snap = snap_move(rect(100.0, 100.0, 50.0, 50.0), &[], space(5.0));

    assert_eq!(snap.shift, (0.0, 0.0));
    assert!(snap.guides.is_empty());
}

#[test]
fn a_side_close_to_the_edge_of_the_image_is_pulled_onto_it() {
    let snap = snap_move(rect(3.0, 200.0, 50.0, 50.0), &[], space(5.0));

    assert!(near(snap.shift.0, -3.0));
    assert!(near(snap.shift.1, 0.0));
    assert_eq!(
        snap.guides,
        vec![Guide {
            vertical: true,
            at: 0.0
        }]
    );
}

#[test]
fn the_right_side_and_the_bottom_snap_too() {
    let snap = snap_move(rect(948.0, 548.0, 50.0, 50.0), &[], space(5.0));

    assert!(near(snap.shift.0, 2.0));
    assert!(near(snap.shift.1, 2.0));
    assert_eq!(snap.guides.len(), 2);
}

#[test]
fn the_center_of_the_image_attracts_the_center_of_the_object() {
    let snap = snap_move(rect(474.0, 100.0, 50.0, 50.0), &[], space(5.0));

    assert!(near(snap.shift.0, 1.0));
    assert!(snap.guides.contains(&Guide {
        vertical: true,
        at: 500.0
    }));
}

#[test]
fn the_edges_and_the_center_of_another_object_attract_as_well() {
    let other = rect(300.0, 300.0, 100.0, 80.0);

    let aligned_left = snap_move(rect(297.0, 50.0, 40.0, 40.0), &[other], space(5.0));
    assert!(near(aligned_left.shift.0, 3.0));

    let aligned_center = snap_move(rect(100.0, 322.0, 40.0, 40.0), &[other], space(5.0));
    assert!(near(aligned_center.shift.1, -2.0));
    assert!(aligned_center.guides.contains(&Guide {
        vertical: false,
        at: 340.0
    }));
}

#[test]
fn the_nearest_line_wins_when_several_are_in_reach() {
    let near_one = rect(400.0, 100.0, 10.0, 10.0);
    let nearer = rect(404.0, 100.0, 10.0, 10.0);

    let snap = snap_move(
        rect(402.0, 400.0, 6.0, 6.0),
        &[near_one, nearer],
        space(5.0),
    );

    assert!(near(snap.shift.0.abs(), 0.0) || snap.shift.0.abs() <= 2.0);
}

#[test]
fn a_tolerance_of_zero_never_moves_anything_unless_it_is_exact() {
    let snap = snap_move(rect(1.0, 1.0, 5.0, 5.0), &[], space(0.0));

    assert_eq!(snap.shift, (0.0, 0.0));
}

fn session_with_two_rectangles() -> (EditSession, u32) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let options = Options::defaults(RECTANGLE.options);
    session.add(Shape::new(
        "rectangle",
        Geometry::Box(rect(300.0, 300.0, 100.0, 100.0)),
        options.clone(),
    ));
    let moving = session.add(Shape::new(
        "rectangle",
        Geometry::Box(rect(100.0, 100.0, 60.0, 60.0)),
        options,
    ));
    (session, moving)
}

fn dragged(snapping: Option<Snapping>, by: (f32, f32)) -> (Geometry, Vec<Guide>) {
    let (mut session, moving) = session_with_two_rectangles();
    let mut interaction = Interaction::new();
    interaction.set_snapping(snapping);
    let start = Point { x: 110.0, y: 100.0 };
    interaction.press(&mut session, Tool::Select, &Options::new(), start, 4.0);
    assert_eq!(session.selected(), Some(moving));
    let end = Point {
        x: start.x + by.0,
        y: start.y + by.1,
    };
    interaction.drag(&mut session, end);
    let guides = interaction.guides().to_vec();
    interaction.release(&mut session, end);
    let geometry = session.shape_of(moving).expect("shape").geometry.clone();
    (geometry, guides)
}

#[test]
fn dragging_an_object_near_another_aligns_it_and_shows_the_guides() {
    let (geometry, guides) = dragged(Some(space(8.0)), (203.0, 0.0));

    let Geometry::Box(moved) = geometry else {
        panic!("a box");
    };
    assert!(near(moved.x, 300.0));
    assert!(!guides.is_empty());
}

#[test]
fn without_snapping_the_drag_is_exact() {
    let (geometry, guides) = dragged(None, (203.0, 0.0));

    let Geometry::Box(moved) = geometry else {
        panic!("a box");
    };
    assert!(near(moved.x, 303.0));
    assert!(guides.is_empty());
}

#[test]
fn the_guides_disappear_when_the_drag_ends() {
    let (mut session, _) = session_with_two_rectangles();
    let mut interaction = Interaction::new();
    interaction.set_snapping(Some(space(8.0)));
    let start = Point { x: 110.0, y: 100.0 };
    interaction.press(&mut session, Tool::Select, &Options::new(), start, 4.0);
    interaction.drag(&mut session, Point { x: 313.0, y: 100.0 });
    assert!(!interaction.guides().is_empty());

    interaction.release(&mut session, Point { x: 313.0, y: 100.0 });

    assert!(interaction.guides().is_empty());
}
