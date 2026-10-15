use pigoune_core::annotations::{
    Crop, CropEditor, CropRatio, CursorKind, Handle, Point, Rect, SMALLEST_SIDE,
};

const REACH: f32 = 8.0;

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

fn drag(editor: &mut CropEditor, from: Point, to: Point) -> bool {
    let claimed = editor.press(from, REACH);
    if claimed {
        editor.drag(point(
            f32::midpoint(from.x, to.x),
            f32::midpoint(from.y, to.y),
        ));
        editor.release(to);
    }
    claimed
}

fn inside_image(frame: Rect, width: f32, height: f32) -> bool {
    frame.x >= -0.001
        && frame.y >= -0.001
        && frame.x + frame.width <= width + 0.001
        && frame.y + frame.height <= height + 0.001
}

#[test]
fn a_new_editor_covers_the_whole_image_and_means_no_crop() {
    let editor = CropEditor::new(1600, 900, None);

    assert_eq!(
        editor.frame(),
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1600.0,
            height: 900.0
        }
    );
    assert_eq!(editor.crop(), None);
    assert_eq!(editor.size(), (1600, 900));
    assert!(!editor.is_changed());
}

#[test]
fn an_existing_crop_is_loaded_and_resetting_removes_it() {
    let current = Crop {
        x: 100,
        y: 50,
        width: 800,
        height: 400,
    };
    let mut editor = CropEditor::new(1600, 900, Some(current));
    assert_eq!(editor.crop(), Some(current));
    assert!(!editor.is_changed());

    editor.reset();

    assert_eq!(editor.crop(), None);
    assert!(editor.is_changed());
    editor.discard();
    assert_eq!(editor.crop(), Some(current));
    assert!(!editor.is_changed());
}

#[test]
fn dragging_a_corner_shrinks_the_frame_to_a_crop() {
    let mut editor = CropEditor::new(1600, 900, None);

    assert!(drag(
        &mut editor,
        point(1600.0, 900.0),
        point(1200.0, 700.0)
    ));

    assert_eq!(
        editor.crop(),
        Some(Crop {
            x: 0,
            y: 0,
            width: 1200,
            height: 700
        })
    );
    assert!(editor.is_changed());
    assert_eq!(editor.size(), (1200, 700));
}

#[test]
fn every_handle_moves_only_its_edges() {
    let start = Crop {
        x: 400,
        y: 300,
        width: 800,
        height: 400,
    };
    let cases = [
        (
            Handle::TopLeft,
            point(400.0, 300.0),
            point(350.0, 250.0),
            (350, 250, 850, 450),
        ),
        (
            Handle::Top,
            point(800.0, 300.0),
            point(900.0, 250.0),
            (400, 250, 800, 450),
        ),
        (
            Handle::TopRight,
            point(1200.0, 300.0),
            point(1250.0, 250.0),
            (400, 250, 850, 450),
        ),
        (
            Handle::Right,
            point(1200.0, 500.0),
            point(1300.0, 600.0),
            (400, 300, 900, 400),
        ),
        (
            Handle::BottomRight,
            point(1200.0, 700.0),
            point(1300.0, 750.0),
            (400, 300, 900, 450),
        ),
        (
            Handle::Bottom,
            point(800.0, 700.0),
            point(1000.0, 780.0),
            (400, 300, 800, 480),
        ),
        (
            Handle::BottomLeft,
            point(400.0, 700.0),
            point(300.0, 760.0),
            (300, 300, 900, 460),
        ),
        (
            Handle::Left,
            point(400.0, 500.0),
            point(300.0, 650.0),
            (300, 300, 900, 400),
        ),
    ];
    for (handle, from, to, (x, y, width, height)) in cases {
        let mut editor = CropEditor::new(1600, 900, Some(start));
        assert_eq!(editor.cursor(from, REACH), CursorKind::Resize(handle));
        assert!(drag(&mut editor, from, to), "{handle:?}");
        assert_eq!(
            editor.crop(),
            Some(Crop {
                x,
                y,
                width,
                height
            }),
            "{handle:?}"
        );
    }
}

#[test]
fn the_frame_never_leaves_the_image() {
    let mut editor = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 200,
            y: 200,
            width: 600,
            height: 300,
        }),
    );

    drag(&mut editor, point(800.0, 500.0), point(5000.0, 5000.0));
    assert!(inside_image(editor.frame(), 1600.0, 900.0));
    drag(&mut editor, point(200.0, 200.0), point(-900.0, -900.0));
    assert!(inside_image(editor.frame(), 1600.0, 900.0));
}

#[test]
fn the_frame_cannot_become_smaller_than_the_minimum_nor_flip() {
    let mut editor = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 400,
            y: 300,
            width: 800,
            height: 400,
        }),
    );

    drag(&mut editor, point(1200.0, 700.0), point(100.0, 100.0));

    let frame = editor.frame();
    assert!(same(frame.width, SMALLEST_SIDE));
    assert!(same(frame.height, SMALLEST_SIDE));
    assert!(same(frame.x, 400.0) && same(frame.y, 300.0));
    let mut other = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 400,
            y: 300,
            width: 800,
            height: 400,
        }),
    );
    drag(&mut other, point(400.0, 300.0), point(2000.0, 2000.0));
    assert!(same(other.frame().width, SMALLEST_SIDE));
    assert!(same(other.frame().x, 1200.0 - SMALLEST_SIDE));
}

#[test]
fn dragging_inside_the_frame_moves_it_without_resizing() {
    let mut editor = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 400,
            y: 300,
            width: 800,
            height: 400,
        }),
    );

    assert_eq!(editor.cursor(point(800.0, 500.0), REACH), CursorKind::Move);
    assert!(drag(&mut editor, point(800.0, 500.0), point(900.0, 450.0)));

    assert_eq!(
        editor.crop(),
        Some(Crop {
            x: 500,
            y: 250,
            width: 800,
            height: 400
        })
    );
    drag(&mut editor, point(900.0, 450.0), point(9000.0, 9000.0));
    assert_eq!(editor.size(), (800, 400));
    assert!(inside_image(editor.frame(), 1600.0, 900.0));
    assert!(same(editor.frame().x, 800.0));
    assert!(same(editor.frame().y, 500.0));
}

#[test]
fn a_press_outside_the_frame_is_not_taken() {
    let mut editor = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 400,
            y: 300,
            width: 800,
            height: 400,
        }),
    );

    assert!(!editor.press(point(50.0, 50.0), REACH));
    assert!(!editor.is_active());
    assert_eq!(editor.cursor(point(50.0, 50.0), REACH), CursorKind::Default);
}

#[test]
fn cancelling_a_gesture_restores_the_frame_and_discard_the_committed_crop() {
    let committed = Crop {
        x: 400,
        y: 300,
        width: 800,
        height: 400,
    };
    let mut editor = CropEditor::new(1600, 900, Some(committed));
    let before = editor.frame();
    editor.press(point(800.0, 500.0), REACH);
    editor.drag(point(900.0, 600.0));
    assert!(editor.is_active());

    editor.cancel_gesture();

    assert_eq!(editor.frame(), before);
    assert!(!editor.is_active());
    drag(&mut editor, point(800.0, 500.0), point(900.0, 600.0));
    assert!(editor.is_changed());
    editor.discard();
    assert_eq!(editor.crop(), Some(committed));
}

#[test]
fn a_square_ratio_reshapes_the_frame_around_its_center() {
    let mut editor = CropEditor::new(1600, 900, None);

    editor.set_ratio(CropRatio::Square);

    let frame = editor.frame();
    assert!(same(frame.width, frame.height));
    assert!(same(frame.height, 900.0));
    assert!(same(frame.x, 350.0));
    assert!(inside_image(frame, 1600.0, 900.0));
    assert_eq!(editor.ratio(), CropRatio::Square);
}

#[test]
fn the_original_ratio_keeps_the_proportions_of_the_image() {
    let mut editor = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 100,
            y: 100,
            width: 500,
            height: 500,
        }),
    );

    editor.set_ratio(CropRatio::Original);

    let frame = editor.frame();
    assert!(same(frame.width / frame.height, 1600.0 / 900.0));
    assert!(inside_image(frame, 1600.0, 900.0));
}

#[test]
fn setting_the_free_ratio_leaves_the_frame_alone() {
    let mut editor = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 100,
            y: 100,
            width: 500,
            height: 400,
        }),
    );
    let before = editor.frame();
    editor.set_ratio(CropRatio::Free);
    assert_eq!(editor.frame(), before);
}

#[test]
fn a_corner_drag_keeps_the_chosen_ratio() {
    let mut editor = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 200,
            y: 200,
            width: 400,
            height: 300,
        }),
    );
    editor.set_ratio(CropRatio::FourThree);
    let frame = editor.frame();
    let corner = point(frame.x + frame.width, frame.y + frame.height);

    drag(
        &mut editor,
        corner,
        point(corner.x + 300.0, corner.y + 60.0),
    );

    let frame = editor.frame();
    assert!(same(frame.width / frame.height, 4.0 / 3.0));
    assert!(frame.width > 600.0);
}

#[test]
fn an_edge_drag_with_a_ratio_changes_both_sides_and_keeps_the_far_edge() {
    let mut editor = CropEditor::new(
        1600,
        900,
        Some(Crop {
            x: 200,
            y: 200,
            width: 400,
            height: 400,
        }),
    );
    editor.set_ratio(CropRatio::Square);
    let frame = editor.frame();
    let middle = point(frame.x + frame.width, frame.y + frame.height / 2.0);

    drag(&mut editor, middle, point(middle.x + 100.0, middle.y));

    let after = editor.frame();
    assert!(same(after.width / after.height, 1.0));
    assert!(same(after.x, frame.x));
    assert!(after.width > frame.width);
}

#[test]
fn whatever_the_drag_and_the_ratio_the_frame_stays_inside_the_image_with_its_ratio() {
    let mut seed: u32 = 11;
    let mut next = move |limit: u32| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) % limit
    };
    let float = |value: u32| f32::from(u16::try_from(value).expect("small"));
    for ratio in CropRatio::ALL {
        let mut editor = CropEditor::new(1600, 900, None);
        editor.set_ratio(ratio);
        for _ in 0..400 {
            let handles = editor.handle_points();
            let (_, at) = handles[usize::try_from(next(8)).expect("index")];
            let target = point(float(next(2400)) - 400.0, float(next(1500)) - 300.0);
            if next(4) == 0 {
                editor.press(point(editor.frame().x + 40.0, editor.frame().y + 40.0), 1.0);
            } else {
                editor.press(at, REACH);
            }
            editor.drag(target);
            editor.release(target);
            let frame = editor.frame();
            assert!(inside_image(frame, 1600.0, 900.0), "{ratio:?} {frame:?}");
            assert!(frame.width >= SMALLEST_SIDE - 0.01, "{ratio:?} {frame:?}");
            assert!(frame.height >= SMALLEST_SIDE - 0.01, "{ratio:?} {frame:?}");
            if let Some(wanted) = ratio.value((1600.0, 900.0)) {
                assert!(
                    (frame.width / frame.height - wanted).abs() < 0.02,
                    "{ratio:?} {frame:?}"
                );
            }
        }
    }
}

#[test]
fn a_crop_from_a_document_is_clamped_to_a_smaller_image() {
    let editor = CropEditor::new(
        500,
        400,
        Some(Crop {
            x: 400,
            y: 300,
            width: 800,
            height: 600,
        }),
    );

    let frame = editor.frame();
    assert!(inside_image(frame, 500.0, 400.0));
    assert!(same(frame.width, 500.0) && same(frame.height, 400.0));
}
