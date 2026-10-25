use std::cell::RefCell;
use std::collections::HashMap;

use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};
use pigoune_core::annotations::{
    AnnotationDocument, Color, Crop, EditSession, Geometry, LayerMove, Options, Point, Rect, Shape,
    Value, color_to_value, definition, number_to_value, translated,
};

use crate::annotation_export;
use crate::annotation_render::{self, Backdrop, PixelCache};
use crate::color_sampler;

const WIDTH: i32 = 200;
const HEIGHT: i32 = 120;
const RED: [u8; 4] = [224, 27, 36, 255];

struct Picture {
    width: i32,
    height: i32,
    data: Vec<u8>,
}

impl Picture {
    fn at(&self, x: i32, y: i32) -> [u8; 4] {
        let index = usize::try_from((y * self.width + x) * 4).expect("inside the picture");
        [
            self.data[index],
            self.data[index + 1],
            self.data[index + 2],
            self.data[index + 3],
        ]
    }
}

struct Rig {
    renderer: gsk::Renderer,
    context: pango::Context,
    texture: gdk::Texture,
    cache: PixelCache,
}

fn source(x: i32, y: i32) -> [u8; 4] {
    let checker = (x / 4 + y / 4) % 2 == 0;
    [
        u8::try_from(x * 255 / (WIDTH - 1)).expect("byte"),
        u8::try_from(y * 255 / (HEIGHT - 1)).expect("byte"),
        if checker { 255 } else { 0 },
        255,
    ]
}

fn source_texture() -> gdk::Texture {
    let mut bytes = Vec::new();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            bytes.extend_from_slice(&source(x, y));
        }
    }
    gdk::MemoryTexture::new(
        WIDTH,
        HEIGHT,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(bytes),
        usize::try_from(WIDTH * 4).expect("stride"),
    )
    .upcast()
}

fn rig() -> Option<Rig> {
    if gtk::init().is_err() {
        assert!(
            std::env::var_os("PIGOUNE_REQUIRE_RENDER_TESTS").is_none(),
            "no display for the rendering tests"
        );
        return None;
    }
    crate::fonts::register();
    let renderer = gsk::CairoRenderer::new();
    renderer
        .realize_for_display(&gdk::Display::default().expect("display"))
        .expect("the renderer is realized");
    Some(Rig {
        renderer: renderer.upcast(),
        context: gtk::Label::new(None).pango_context(),
        texture: source_texture(),
        cache: RefCell::new(HashMap::new()),
    })
}

fn picture(texture: &gdk::Texture) -> Picture {
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let row = usize::try_from(texture.width()).expect("width") * 4;
    let mut data = Vec::new();
    for line in bytes.chunks(stride) {
        data.extend_from_slice(&line[..row]);
    }
    Picture {
        width: texture.width(),
        height: texture.height(),
        data,
    }
}

impl Rig {
    fn backdrop(&self) -> Backdrop<'_> {
        Backdrop {
            texture: &self.texture,
            renderer: &self.renderer,
            cache: &self.cache,
        }
    }

    fn export(&self, document: &AnnotationDocument) -> Picture {
        self.cache.borrow_mut().clear();
        let texture = annotation_export::final_texture(&self.backdrop(), document, &self.context)
            .expect("the final image is rendered");
        picture(&texture)
    }

    fn preview(&self, document: &AnnotationDocument) -> Picture {
        self.cache.borrow_mut().clear();
        let snapshot = gtk::Snapshot::new();
        snapshot.append_scaled_texture(
            &self.texture,
            gsk::ScalingFilter::Nearest,
            &graphene::Rect::new(0.0, 0.0, 200.0, 120.0),
        );
        annotation_render::append_document(
            &snapshot,
            document,
            &self.context,
            Some(&self.backdrop()),
            None,
        );
        let node = snapshot.to_node().expect("node");
        let texture = self
            .renderer
            .render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, 200.0, 120.0)));
        picture(&texture)
    }
}

fn shape(kind: &str, geometry: Geometry, options: &[(&str, Value)]) -> Shape {
    let sheet = definition(kind).expect("tool");
    let mut shape = Shape::new(kind, geometry, Options::defaults(sheet.options));
    for (id, value) in options {
        shape = shape.with_option(id, value.clone());
    }
    shape
}

fn boxed(kind: &str, x: f32, y: f32, width: f32, height: f32, options: &[(&str, Value)]) -> Shape {
    shape(
        kind,
        Geometry::Box(Rect {
            x,
            y,
            width,
            height,
        }),
        options,
    )
}

fn segment(kind: &str, from: (f32, f32), to: (f32, f32), options: &[(&str, Value)]) -> Shape {
    shape(
        kind,
        Geometry::TwoPoints {
            from: Point {
                x: from.0,
                y: from.1,
            },
            to: Point { x: to.0, y: to.1 },
        },
        options,
    )
}

fn stroke(kind: &str, points: &[(f32, f32)], options: &[(&str, Value)]) -> Shape {
    shape(
        kind,
        Geometry::Polyline(points.iter().map(|(x, y)| Point { x: *x, y: *y }).collect()),
        options,
    )
}

fn document(shapes: Vec<Shape>) -> AnnotationDocument {
    let mut session = EditSession::new(AnnotationDocument::default());
    for shape in shapes {
        session.add(shape);
    }
    session.document().clone()
}

fn paint(r: u8, g: u8, b: u8) -> Value {
    color_to_value(Color { r, g, b, a: 255 })
}

fn near(left: [u8; 4], right: [u8; 4], tolerance: u8) -> bool {
    left.iter()
        .zip(right)
        .all(|(a, b)| a.abs_diff(b) <= tolerance)
}

fn assert_unchanged(picture: &Picture, from: (i32, i32), to: (i32, i32), what: &str) {
    for y in from.1..to.1 {
        for x in from.0..to.0 {
            assert_eq!(picture.at(x, y), source(x, y), "{what}: pixel ({x}, {y})");
        }
    }
}

fn contrast(picture: &Picture, from: (i32, i32), to: (i32, i32)) -> u32 {
    let mut total = 0;
    for y in from.1..to.1 {
        for x in from.0..to.0 - 1 {
            total += u32::from(picture.at(x, y)[2].abs_diff(picture.at(x + 1, y)[2]));
        }
    }
    total
}

fn untouched_image_is_reproduced_exactly(rig: &Rig) {
    let result = rig.export(&AnnotationDocument::default());
    assert_eq!((result.width, result.height), (WIDTH, HEIGHT));
    assert_unchanged(&result, (0, 0), (WIDTH, HEIGHT), "no annotation");
}

fn outlines_and_lines_are_drawn_where_they_belong(rig: &Rig) {
    let result = rig.export(&document(vec![
        boxed("rectangle", 40.0, 30.0, 100.0, 60.0, &[]),
        boxed(
            "ellipse",
            150.0,
            20.0,
            40.0,
            80.0,
            &[("width", number_to_value(4.0))],
        ),
        segment("line", (10.0, 10.0), (190.0, 10.0), &[]),
    ]));
    assert!(near(result.at(40, 60), RED, 2), "rectangle edge");
    assert_eq!(
        result.at(90, 60),
        source(90, 60),
        "rectangle inside stays hollow"
    );
    assert_eq!(result.at(5, 60), source(5, 60), "outside the rectangle");
    assert!(near(result.at(190, 60), RED, 2), "ellipse right end");
    assert_eq!(result.at(170, 60), source(170, 60), "ellipse inside");
    assert!(near(result.at(100, 10), RED, 2), "line");
    assert_eq!(result.at(100, 25), source(100, 25), "beside the line");
}

fn arrows_follow_their_curve(rig: &Rig) {
    let straight = rig.export(&document(vec![segment(
        "arrow",
        (20.0, 100.0),
        (180.0, 100.0),
        &[("width", number_to_value(10.0))],
    )]));
    assert!(near(straight.at(100, 100), RED, 2), "straight shaft");
    assert!(near(straight.at(176, 100), RED, 2), "arrow head");
    assert_eq!(
        straight.at(100, 80),
        source(100, 80),
        "above a straight arrow"
    );

    let curved = rig.export(&document(vec![segment(
        "arrow",
        (20.0, 100.0),
        (180.0, 100.0),
        &[
            ("width", number_to_value(10.0)),
            ("curve", number_to_value(-50.0)),
        ],
    )]));
    let column: Vec<[u8; 4]> = (70..92).map(|y| curved.at(100, y)).collect();
    assert!(
        near(curved.at(100, 80), RED, 2),
        "middle of the curve {column:?}"
    );
    assert_eq!(curved.at(100, 100), source(100, 100), "the chord is empty");
}

fn uniform_rig(rig: &Rig, level: u8) -> Rig {
    let mut bytes = Vec::new();
    for _ in 0..WIDTH * HEIGHT {
        bytes.extend_from_slice(&[level, level, level, 255]);
    }
    Rig {
        renderer: rig.renderer.clone(),
        context: rig.context.clone(),
        texture: gdk::MemoryTexture::new(
            WIDTH,
            HEIGHT,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(bytes),
            usize::try_from(WIDTH * 4).expect("stride"),
        )
        .upcast(),
        cache: RefCell::new(HashMap::new()),
    }
}

fn coverage(picture: &Picture, x: i32, y: i32, background: u8) -> Option<f32> {
    let pixel = picture.at(x, y);
    let (channel, spread) = (0..3)
        .map(|channel| (channel, i32::from(RED[channel]) - i32::from(background)))
        .max_by_key(|(_, spread)| spread.abs())
        .expect("a channel");
    (spread.abs() >= 60).then(|| {
        let seen = f32::from(i16::from(pixel[channel]) - i16::from(background));
        seen / f32::from(i16::try_from(spread).expect("small"))
    })
}

fn translucent_arrows_are_one_homogeneous_shape(rig: &Rig) {
    for background in [245_u8, 30] {
        let plain = uniform_rig(rig, background);
        for (shaft, head, width, curve) in [
            ("tapered", "triangle", 4.0, 0.0),
            ("tapered", "triangle", 14.0, 0.0),
            ("tapered", "triangle", 10.0, -40.0),
            ("tapered", "open", 8.0, 0.0),
            ("tapered", "none", 8.0, 0.0),
            ("tapered", "double", 8.0, 0.0),
            ("even", "triangle", 4.0, 0.0),
            ("even", "triangle", 14.0, -40.0),
            ("even", "open", 8.0, 0.0),
            ("even", "double", 8.0, 25.0),
            ("even", "none", 8.0, 0.0),
        ] {
            for opacity in [1.0_f32, 0.8, 0.5, 0.2] {
                let arrow = segment(
                    "arrow",
                    (20.0, 60.0),
                    (180.0, 60.0),
                    &[
                        ("shaft", Value::from(shaft)),
                        ("head", Value::from(head)),
                        ("width", number_to_value(width)),
                        ("curve", number_to_value(curve)),
                        ("opacity", number_to_value(opacity)),
                    ],
                );
                let drawn = document(vec![arrow]);
                let what = format!(
                    "{shaft} {head} width {width} curve {curve} opacity {opacity} on {background}"
                );
                let result = plain.export(&drawn);
                let mut strongest = 0.0_f32;
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        if let Some(value) = coverage(&result, x, y, background) {
                            strongest = strongest.max(value);
                        }
                    }
                }
                assert!(
                    strongest <= opacity + 0.04,
                    "{what}: a pixel is {strongest:.2} opaque, the whole arrow must be at most {opacity}"
                );
                if curve == 0.0 {
                    let last = if head == "open" { 130 } else { 170 };
                    for x in 88..last {
                        let value = coverage(&result, x, 60, background).expect("on the axis");
                        assert!(
                            (value - opacity).abs() <= 0.07,
                            "{what}: the axis at x={x} is {value:.2} opaque instead of {opacity}"
                        );
                    }
                }
                assert_eq!(
                    plain.preview(&drawn).data,
                    result.data,
                    "{what}: preview and export differ"
                );
            }
        }
    }
}

fn text_steps_and_strokes_are_drawn(rig: &Rig) {
    let text = rig.export(&document(vec![shape(
        "text",
        Geometry::Anchor(Point { x: 20.0, y: 20.0 }),
        &[
            ("content", Value::from("Hi")),
            ("font_size", number_to_value(32.0)),
        ],
    )]));
    let mut changed = 0;
    for y in 20..60 {
        for x in 20..80 {
            if text.at(x, y) != source(x, y) {
                changed += 1;
            }
        }
    }
    assert!(
        changed > 40,
        "the text paints pixels in its box ({changed})"
    );
    assert_unchanged(&text, (120, 80), (190, 110), "far from the text");

    let badge = rig.export(&document(vec![shape(
        "step",
        Geometry::Anchor(Point { x: 100.0, y: 60.0 }),
        &[],
    )]));
    assert!(near(badge.at(111, 60), RED, 2), "inside the badge disc");
    assert_eq!(badge.at(130, 60), source(130, 60), "outside the badge");

    let pen = rig.export(&document(vec![stroke(
        "pen",
        &[(20.0, 100.0), (100.0, 100.0), (180.0, 100.0)],
        &[],
    )]));
    assert!(near(pen.at(60, 100), RED, 2), "pen stroke");
    assert_eq!(pen.at(60, 80), source(60, 80), "beside the stroke");
}

fn translucent_effects_blend_with_the_image(rig: &Rig) {
    let highlight = rig.export(&document(vec![stroke(
        "highlighter",
        &[(20.0, 60.0), (180.0, 60.0)],
        &[],
    )]));
    let under = source(100, 60);
    let yellow = [0xf6, 0xd3, 0x2d, 255];
    let result = highlight.at(100, 60);
    assert_ne!(result, under, "the highlighter changes the pixel");
    assert_ne!(result, yellow, "the highlighter is translucent");
    for channel in 0..3 {
        let (low, high) = (
            under[channel].min(yellow[channel]),
            under[channel].max(yellow[channel]),
        );
        assert!(
            (low..=high).contains(&result[channel]),
            "channel {channel} stays between the image and the ink"
        );
    }
    assert_unchanged(&highlight, (20, 90), (180, 110), "below the highlighter");

    let faded = rig.export(&document(vec![boxed(
        "rectangle",
        40.0,
        30.0,
        100.0,
        60.0,
        &[("opacity", number_to_value(0.5))],
    )]));
    let blended = faded.at(40, 60);
    assert_ne!(blended, RED);
    assert_ne!(blended, source(40, 60));
}

fn the_spotlight_dims_everything_around_it(rig: &Rig) {
    let result = rig.export(&document(vec![boxed(
        "spotlight",
        60.0,
        30.0,
        80.0,
        60.0,
        &[],
    )]));
    assert_unchanged(&result, (70, 40), (130, 80), "inside the spotlight");
    for (x, y) in [(5, 5), (190, 110), (30, 100)] {
        let dimmed = result.at(x, y);
        let original = source(x, y);
        assert!(
            (0..3).all(|channel| dimmed[channel] <= original[channel])
                && (0..3).any(|channel| dimmed[channel] < original[channel]),
            "outside pixel ({x}, {y}) is darker"
        );
    }
}

fn the_rounded_spotlight_dims_its_corners_only(rig: &Rig) {
    let result = rig.export(&document(vec![boxed(
        "spotlight",
        60.0,
        30.0,
        80.0,
        60.0,
        &[("spot", Value::from("rounded"))],
    )]));
    assert_unchanged(&result, (90, 50), (110, 70), "inside the rounded spotlight");
    assert_unchanged(&result, (100, 33), (101, 34), "the middle of the top edge");
    let corner = result.at(61, 31);
    assert!(
        (0..3).any(|channel| corner[channel] < source(61, 31)[channel]),
        "the corner is dimmed"
    );
}

fn a_turned_rectangle_is_drawn_turned_around_its_centre(rig: &Rig) {
    let solid = |rotation: f32| {
        rig.export(&document(vec![boxed(
            "rectangle",
            60.0,
            50.0,
            80.0,
            20.0,
            &[
                ("fill", Value::from(true)),
                ("fill_opacity", number_to_value(1.0)),
                ("rotation", number_to_value(rotation)),
            ],
        )]))
    };
    let flat = solid(0.0);
    assert!(
        near(flat.at(65, 60), RED, 2),
        "the flat bar covers its left end"
    );
    assert_eq!(flat.at(100, 25), source(100, 25), "and not the top");

    let upright = solid(90.0);
    assert!(
        near(upright.at(100, 25), RED, 2),
        "the turned bar reaches the top"
    );
    assert!(near(upright.at(100, 95), RED, 2), "and the bottom");
    assert_eq!(
        upright.at(65, 60),
        source(65, 60),
        "and leaves its old left end"
    );
    assert!(
        near(upright.at(100, 60), RED, 2),
        "the centre stays covered"
    );
}

fn a_turned_blur_and_a_turned_pixelation_cover_exactly_their_turned_area(rig: &Rig) {
    let region = |kind: &str, options: &[(&str, Value)]| {
        let mut all = vec![("rotation", number_to_value(90.0))];
        all.extend(options.iter().cloned());
        rig.export(&document(vec![boxed(kind, 70.0, 40.0, 60.0, 20.0, &all)]))
    };

    let blur = region("blur", &[("radius", number_to_value(8.0))]);
    let white = blur.at(100, 25)[2];
    let black = blur.at(104, 25)[2];
    assert!(
        white.abs_diff(black) < 4,
        "the turned area is blurred ({white} against {black})"
    );
    assert_eq!(
        blur.at(72, 50),
        source(72, 50),
        "the old position is left alone"
    );
    assert_eq!(blur.at(5, 5), source(5, 5), "far from the blur");
    assert_ne!(blur.at(100, 25), source(100, 25));
    for channel in 0..2 {
        assert!(
            blur.at(100, 25)[channel].abs_diff(source(100, 25)[channel]) < 6,
            "the blur shows the image under it, not a turned copy (channel {channel})"
        );
    }

    let cells = region("pixelate", &[("cell", number_to_value(10.0))]);
    assert_eq!(cells.at(91, 21), cells.at(98, 28), "one cell is uniform");
    assert_ne!(cells.at(91, 21), source(91, 21), "and is not the source");
    for channel in 0..2 {
        assert!(
            cells.at(95, 25)[channel].abs_diff(source(95, 25)[channel]) < 10,
            "the cell shows the image under it (channel {channel})"
        );
    }
    assert_eq!(
        cells.at(72, 50),
        source(72, 50),
        "the old position is left alone"
    );
    assert_eq!(cells.at(5, 5), source(5, 5), "far from the pixelation");
}

fn a_turned_blur_on_the_image_edge_stays_opaque(rig: &Rig) {
    let result = rig.export(&document(vec![boxed(
        "blur",
        0.0,
        40.0,
        120.0,
        40.0,
        &[
            ("radius", number_to_value(60.0)),
            ("rotation", number_to_value(90.0)),
        ],
    )]));
    let white = result.at(60, 60)[2];
    let black = result.at(64, 60)[2];
    assert!(
        white.abs_diff(black) < 4,
        "no sharp pixel shows through a turned blur ({white} against {black})"
    );
}

fn turned_points_draw_a_line_and_a_stroke_where_they_were_turned(rig: &Rig) {
    let line = segment(
        "line",
        (60.0, 60.0),
        (140.0, 60.0),
        &[("width", number_to_value(8.0))],
    );
    let upright =
        pigoune_core::annotations::turned_points(&line, Point { x: 100.0, y: 60.0 }, 90.0);
    let flat = rig.export(&document(vec![line]));
    assert!(
        near(flat.at(65, 60), RED, 2),
        "the flat line covers its left end"
    );
    assert_eq!(flat.at(100, 25), source(100, 25), "and not the top");
    let turned = rig.export(&document(vec![upright]));
    assert!(
        near(turned.at(100, 25), RED, 2),
        "the turned line reaches the top"
    );
    assert_eq!(
        turned.at(65, 60),
        source(65, 60),
        "and leaves its old left end"
    );
}

fn a_bubble_is_drawn_with_a_filled_body_an_outline_and_a_tail(rig: &Rig) {
    let picture = rig.export(&document(vec![boxed(
        "bubble",
        20.0,
        20.0,
        100.0,
        60.0,
        &[
            ("tail_x", number_to_value(0.4)),
            ("tail_y", number_to_value(1.0)),
        ],
    )]));
    let white = [255, 255, 255, 255];
    let ink = [0x24, 0x1f, 0x31, 255];
    assert!(near(picture.at(70, 50), white, 2), "the body is filled");
    assert!(near(picture.at(20, 50), ink, 40), "the outline is dark");
    assert!(near(picture.at(99, 94), white, 2), "the tail is filled too");
    assert_eq!(picture.at(30, 100), source(30, 100), "nothing elsewhere");
    assert_eq!(picture.at(150, 50), source(150, 50), "nothing on the right");

    let cloud = boxed(
        "bubble",
        20.0,
        10.0,
        100.0,
        60.0,
        &[
            ("form", Value::from("cloud")),
            ("width", number_to_value(1.0)),
            ("tail_x", number_to_value(0.4)),
            ("tail_y", number_to_value(0.9)),
        ],
    );
    let circles = pigoune_core::annotations::bubble_parts(&cloud).expect("parts");
    let last = &circles[3];
    let count = f32::from(u16::try_from(last.len()).expect("small"));
    let (x, y) = (
        last.iter().map(|p| p.x).sum::<f32>() / count,
        last.iter().map(|p| p.y).sum::<f32>() / count,
    );
    let picture = rig.export(&document(vec![cloud]));
    assert!(near(picture.at(70, 40), white, 2), "the cloud is filled");
    #[expect(clippy::cast_possible_truncation, reason = "inside the test picture")]
    let (x, y) = (x.round() as i32, y.round() as i32);
    assert!(near(picture.at(x, y), white, 2), "so is its last circle");

    let said = |content: &str| {
        rig.export(&document(vec![boxed(
            "bubble",
            20.0,
            10.0,
            100.0,
            60.0,
            &[
                ("content", Value::from(content)),
                ("font_size", number_to_value(24.0)),
                ("width", number_to_value(1.0)),
            ],
        )]))
    };
    let dark_in = |picture: &Picture, from: i32, to: i32| {
        (from..to)
            .flat_map(|x| (22..58).map(move |y| (x, y)))
            .filter(|(x, y)| picture.at(*x, *y)[0] < 120)
            .count()
    };
    assert_eq!(
        dark_in(&said(""), 36, 104),
        0,
        "an empty bubble has no letters"
    );
    let words = said("HH");
    assert!(
        dark_in(&words, 36, 104) > 30,
        "the letters are drawn in dark ink"
    );
    assert!(
        dark_in(&words, 36, 70) > 0 && dark_in(&words, 70, 104) > 0,
        "and centred on the bubble"
    );
}

fn the_cartoon_lettering_is_really_another_font(rig: &Rig) {
    let measured = |lettering: &str| {
        let bubble = boxed(
            "bubble",
            0.0,
            0.0,
            100.0,
            60.0,
            &[
                ("content", Value::from("Hello world")),
                ("lettering", Value::from(lettering)),
                ("font_size", number_to_value(24.0)),
            ],
        );
        annotation_render::bubble_text_height(&rig.context, &bubble, "Hello world")
    };
    let family = |lettering: &str| {
        annotation_render::text_view_css(
            &boxed(
                "bubble",
                0.0,
                0.0,
                100.0,
                60.0,
                &[("lettering", Value::from(lettering))],
            ),
            1.0,
        )
    };
    assert!(family("cartoon").contains(crate::fonts::CARTOON_FAMILY));
    assert!(!family("normal").contains(crate::fonts::CARTOON_FAMILY));
    let layout = pango::Layout::new(&rig.context);
    layout.set_text("Hello world");
    let mut font = pango::FontDescription::new();
    font.set_family(crate::fonts::CARTOON_FAMILY);
    font.set_absolute_size(24.0 * f64::from(pango::SCALE));
    layout.set_font_description(Some(&font));
    let mut plain = pango::FontDescription::new();
    plain.set_family("Sans");
    plain.set_absolute_size(24.0 * f64::from(pango::SCALE));
    let other = pango::Layout::new(&rig.context);
    other.set_text("Hello world");
    other.set_font_description(Some(&plain));
    assert_ne!(
        layout.pixel_size().0,
        other.pixel_size().0,
        "the cartoon family is registered and differs from the system one"
    );
    assert!(measured("cartoon") > 0.0 && measured("normal") > 0.0);
}

fn layers_follow_their_order(rig: &Rig) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let filled = [
        ("fill", Value::from(true)),
        ("fill_opacity", number_to_value(1.0)),
    ];
    let mut back = filled.to_vec();
    back.push(("color", paint(224, 27, 36)));
    let mut front = filled.to_vec();
    front.push(("color", paint(28, 113, 216)));
    let red = session.add(boxed("rectangle", 20.0, 20.0, 80.0, 60.0, &back));
    session.add(boxed("rectangle", 60.0, 40.0, 80.0, 60.0, &front));
    let blue = [28, 113, 216, 255];

    let first = rig.export(session.document());
    assert!(
        near(first.at(80, 60), blue, 2),
        "the later object is on top"
    );
    assert!(near(first.at(30, 30), RED, 2), "red only");

    session.select(Some(red));
    assert!(session.move_selected(LayerMove::Front));
    let second = rig.export(session.document());
    assert!(near(second.at(80, 60), RED, 2), "reordered: red is on top");
    assert!(near(second.at(120, 90), blue, 2), "blue only");
}

fn a_crop_keeps_the_region_and_its_coordinates(rig: &Rig) {
    let mut cropped = document(vec![boxed("rectangle", 40.0, 30.0, 40.0, 20.0, &[])]);
    cropped.crop = Some(Crop {
        x: 20,
        y: 10,
        width: 100,
        height: 60,
    });
    let result = rig.export(&cropped);
    assert_eq!((result.width, result.height), (100, 60));
    assert_eq!(
        result.at(0, 0),
        source(20, 10),
        "the crop starts at its origin"
    );
    assert_eq!(
        result.at(99, 59),
        source(119, 69),
        "the crop ends at its corner"
    );
    assert!(
        near(result.at(20, 30), RED, 2),
        "the rectangle follows the crop"
    );
}

fn blur_covers_only_its_region_and_follows_it(rig: &Rig) {
    let blur = boxed(
        "blur",
        40.0,
        20.0,
        120.0,
        80.0,
        &[("radius", number_to_value(8.0))],
    );
    let before = rig.export(&document(vec![blur.clone()]));
    assert_unchanged(&before, (0, 0), (WIDTH, 18), "above the blur");
    assert_unchanged(&before, (0, 102), (WIDTH, HEIGHT), "below the blur");
    assert_unchanged(&before, (0, 0), (38, HEIGHT), "left of the blur");
    assert_unchanged(&before, (162, 0), (WIDTH, HEIGHT), "right of the blur");
    let sharp = contrast(&before, (45, 30), (155, 90));
    let original = contrast(
        &rig.export(&AnnotationDocument::default()),
        (45, 30),
        (155, 90),
    );
    assert!(
        sharp * 4 < original,
        "the inside is blurred ({sharp} against {original})"
    );

    let moved = rig.export(&document(vec![translated(&blur, 30.0, 0.0)]));
    assert_unchanged(
        &moved,
        (30, 0),
        (68, HEIGHT),
        "the old position is restored",
    );
    assert!(
        contrast(&moved, (75, 30), (185, 90)) * 4 < original,
        "the new position is blurred"
    );
    assert_unchanged(&moved, (192, 0), (WIDTH, HEIGHT), "right of the moved blur");
}

fn a_blur_stays_opaque_near_the_edges_of_the_image(rig: &Rig) {
    let original = contrast(
        &rig.export(&AnnotationDocument::default()),
        (45, 30),
        (155, 90),
    );
    for radius in [8.0, 60.0, 150.0] {
        let inside = rig.export(&document(vec![boxed(
            "blur",
            40.0,
            20.0,
            120.0,
            80.0,
            &[("radius", number_to_value(radius))],
        )]));
        let white = inside.at(100, 60)[2];
        let black = inside.at(104, 60)[2];
        assert!(
            white.abs_diff(black) < 4,
            "no sharp pixel shows through a blur of radius {radius} ({white} against {black})"
        );
        let on_the_edge = rig.export(&document(vec![boxed(
            "blur",
            0.0,
            0.0,
            80.0,
            120.0,
            &[("radius", number_to_value(radius))],
        )]));
        assert!(
            contrast(&on_the_edge, (2, 4), (78, 116)) * 8 < original,
            "a blur on the image edge is as soft as elsewhere (radius {radius})"
        );
    }
}

fn pixelation_makes_uniform_cells_and_follows_its_region(rig: &Rig) {
    let cell = number_to_value(10.0);
    let pixelate = boxed("pixelate", 40.0, 20.0, 120.0, 80.0, &[("cell", cell)]);
    let before = rig.export(&document(vec![pixelate.clone()]));
    assert_unchanged(&before, (0, 0), (WIDTH, 18), "above the pixelation");
    assert_unchanged(&before, (0, 0), (38, HEIGHT), "left of the pixelation");
    for (column, row) in [(0, 0), (3, 2), (11, 7)] {
        let (left, top) = (40 + column * 10, 20 + row * 10);
        let first = before.at(left, top);
        for y in top..top + 10 {
            for x in left..left + 10 {
                assert_eq!(
                    before.at(x, y),
                    first,
                    "cell ({column}, {row}) is uniform at ({x}, {y})"
                );
            }
        }
        assert_ne!(
            first,
            source(left, top),
            "cell ({column}, {row}) is not the original pixel"
        );
    }

    let moved = rig.export(&document(vec![translated(&pixelate, 30.0, 0.0)]));
    assert_unchanged(&moved, (40, 22), (68, 98), "the old position is restored");
    let uniform = moved.at(70, 20);
    for y in 20..30 {
        for x in 70..80 {
            assert_eq!(moved.at(x, y), uniform, "the moved cell is uniform");
        }
    }
}

fn an_annotation_after_a_mask_stays_sharp(rig: &Rig) {
    let result = rig.export(&document(vec![
        boxed(
            "blur",
            40.0,
            20.0,
            120.0,
            80.0,
            &[("radius", number_to_value(8.0))],
        ),
        stroke(
            "pen",
            &[(50.0, 60.0), (150.0, 60.0)],
            &[("width", number_to_value(6.0))],
        ),
    ]));
    assert!(
        near(result.at(100, 60), RED, 2),
        "the stroke over the blur is not blurred"
    );

    let under = rig.export(&document(vec![
        stroke(
            "pen",
            &[(50.0, 60.0), (150.0, 60.0)],
            &[("width", number_to_value(6.0))],
        ),
        boxed(
            "blur",
            40.0,
            20.0,
            120.0,
            80.0,
            &[("radius", number_to_value(8.0))],
        ),
    ]));
    assert_ne!(
        under.at(100, 60),
        RED,
        "a mask hides what was drawn before it"
    );
}

fn hand_drawn_shapes() -> Vec<Shape> {
    let look = |level: &str| ("roughness", Value::from(level));
    vec![
        boxed("rectangle", 20.0, 15.0, 70.0, 40.0, &[look("sketchy")]),
        boxed("ellipse", 110.0, 15.0, 70.0, 40.0, &[look("very_sketchy")]),
        segment("line", (20.0, 75.0), (90.0, 95.0), &[look("sketchy")]),
        segment(
            "arrow",
            (110.0, 75.0),
            (180.0, 100.0),
            &[look("very_sketchy")],
        ),
    ]
}

fn the_precise_look_is_the_look_of_every_older_shape(rig: &Rig) {
    for kind in ["rectangle", "ellipse"] {
        let plain = rig.export(&document(vec![boxed(kind, 30.0, 20.0, 100.0, 60.0, &[])]));
        let precise = rig.export(&document(vec![boxed(
            kind,
            30.0,
            20.0,
            100.0,
            60.0,
            &[("roughness", Value::from("precise"))],
        )]));
        let mut old = boxed(kind, 30.0, 20.0, 100.0, 60.0, &[]);
        old.options = Options::new();
        let without_option = rig.export(&document(vec![old]));
        assert_eq!(plain.data, precise.data, "{kind} precise");
        assert_eq!(plain.data, without_option.data, "{kind} without the option");
    }
}

fn a_hand_drawn_shape_is_always_drawn_the_same_way(rig: &Rig) {
    let sketched = document(hand_drawn_shapes());
    let first = rig.export(&sketched);
    assert_eq!(first.data, rig.export(&sketched).data, "same document");
    let saved = sketched.to_json().expect("saved");
    let reopened = AnnotationDocument::from_json(&saved).expect("read");
    assert_eq!(
        first.data,
        rig.export(&reopened).data,
        "after saving and reading"
    );
    assert_eq!(first.data, rig.preview(&sketched).data, "the preview");
}

fn a_hand_drawn_shape_differs_from_the_precise_one_but_stays_near_it(rig: &Rig) {
    let precise_shapes: Vec<Shape> = hand_drawn_shapes()
        .into_iter()
        .map(|shape| shape.with_option("roughness", Value::from("precise")))
        .collect();
    let precise = rig.export(&document(precise_shapes));
    let sketched = rig.export(&document(hand_drawn_shapes()));
    assert_ne!(precise.data, sketched.data);
    assert_unchanged(&sketched, (0, 0), (WIDTH, 5), "far above everything");
    assert_unchanged(&sketched, (0, 112), (WIDTH, HEIGHT), "far below everything");
    assert_unchanged(&sketched, (0, 0), (8, HEIGHT), "far left of everything");
    assert_unchanged(
        &sketched,
        (194, 0),
        (WIDTH, HEIGHT),
        "far right of everything",
    );
}

fn a_duplicate_looks_exactly_like_its_original(rig: &Rig) {
    let flat = uniform_rig(rig, 255);
    let mut session = EditSession::new(AnnotationDocument::default());
    let original = session.add(boxed(
        "ellipse",
        10.0,
        10.0,
        60.0,
        40.0,
        &[("roughness", Value::from("very_sketchy"))],
    ));
    session.duplicate_object(original, 60.0).expect("copy");
    let result = flat.export(session.document());
    let mut inked = 0;
    for y in 0..57 {
        for x in 0..80 {
            assert_eq!(
                result.at(x, y),
                result.at(x + 60, y + 60),
                "pixel ({x}, {y})"
            );
            inked += usize::from(result.at(x, y) != [255, 255, 255, 255]);
        }
    }
    assert!(inked > 100, "the ellipse is drawn");
}

fn a_hand_drawn_look_changes_with_the_seed_of_the_shape(rig: &Rig) {
    let flat = uniform_rig(rig, 255);
    let ellipse = || {
        boxed(
            "ellipse",
            30.0,
            20.0,
            100.0,
            60.0,
            &[("roughness", Value::from("very_sketchy"))],
        )
    };
    let spacer = segment("line", (150.0, 105.0), (190.0, 110.0), &[]);
    let first = flat.export(&document(vec![ellipse()]));
    let second = flat.export(&document(vec![spacer, ellipse()]));
    let region = |picture: &Picture| {
        (10..100)
            .flat_map(|y| (10..150).map(move |x| picture.at(x, y)))
            .collect::<Vec<_>>()
    };
    assert_ne!(region(&first), region(&second));
}

fn a_dashed_pen_stroke_leaves_gaps_and_a_solid_one_does_not(rig: &Rig) {
    let flat = uniform_rig(rig, 255);
    let points = [(20.0, 60.0), (100.0, 60.0), (180.0, 60.0)];
    let row = |dash: &str| {
        let picture = flat.export(&document(vec![stroke(
            "pen",
            &points,
            &[("dash", Value::from(dash)), ("width", number_to_value(6.0))],
        )]));
        (30..170)
            .filter(|x| picture.at(*x, 60) == [255, 255, 255, 255])
            .count()
    };
    assert_eq!(row("solid"), 0, "a solid stroke has no gap");
    assert!(row("dashed") > 20, "a dashed stroke has gaps");
    assert!(row("dotted") > 20, "a dotted stroke has gaps");
}

fn the_export_matches_the_preview(rig: &Rig) {
    let everything = document(vec![
        boxed("rectangle", 10.0, 10.0, 60.0, 40.0, &[]),
        segment(
            "arrow",
            (20.0, 100.0),
            (180.0, 90.0),
            &[("curve", number_to_value(30.0))],
        ),
        boxed("blur", 80.0, 20.0, 60.0, 40.0, &[]),
        boxed("pixelate", 120.0, 60.0, 60.0, 40.0, &[]),
        boxed("spotlight", 5.0, 5.0, 150.0, 100.0, &[]),
    ]);
    let preview = rig.preview(&everything);
    let export = rig.export(&everything);
    assert_eq!(preview.data, export.data, "the final image is the preview");
}

fn hidden_objects_are_not_drawn_and_locked_ones_are(rig: &Rig) {
    let mut session = EditSession::new(AnnotationDocument::default());
    let red = [
        ("fill", Value::from(true)),
        ("fill_opacity", number_to_value(1.0)),
        ("color", paint(224, 27, 36)),
    ];
    let id = session.add(boxed("rectangle", 20.0, 20.0, 80.0, 60.0, &red));
    let plain = rig.export(&AnnotationDocument::default());
    let shown = rig.export(session.document());
    assert!(near(shown.at(60, 50), RED, 2), "the object is drawn");

    session.set_locked(id, true);
    assert_eq!(
        rig.export(session.document()).data,
        shown.data,
        "a locked object is drawn as usual"
    );
    session.set_hidden(id, true);
    assert_eq!(
        rig.export(session.document()).data,
        plain.data,
        "a hidden object is absent from the export"
    );
    assert_eq!(
        rig.preview(session.document()).data,
        rig.preview(&AnnotationDocument::default()).data,
        "a hidden object is absent from the preview"
    );
    session.undo();
    assert_eq!(rig.export(session.document()).data, shown.data);
}

fn the_color_sampler_reads_the_pixel_it_is_given(rig: &Rig) {
    for (x, y) in [(10, 10), (199, 0), (57, 91)] {
        let color = color_sampler::pixel_color(
            &rig.renderer,
            &rig.texture,
            f32::from(i16::try_from(x).expect("small")),
            f32::from(i16::try_from(y).expect("small")),
        )
        .expect("a pixel");
        let expected = source(x, y);
        assert_eq!(
            [color.r, color.g, color.b],
            [expected[0], expected[1], expected[2]]
        );
    }
}

fn the_png_export_carries_pixels_and_nothing_else(rig: &Rig) {
    let masked = document(vec![
        boxed("blur", 20.0, 20.0, 60.0, 60.0, &[]),
        boxed("pixelate", 100.0, 20.0, 60.0, 60.0, &[]),
    ]);
    rig.cache.borrow_mut().clear();
    let texture = annotation_export::final_texture(&rig.backdrop(), &masked, &rig.context)
        .expect("the final image is rendered");
    let bytes = glib::MainContext::default()
        .block_on(crate::image_conversion::png_bytes(&texture))
        .expect("the image is encoded");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let mut kinds: Vec<String> = Vec::new();
    let mut at = 8;
    while at + 8 <= bytes.len() {
        let length = usize::try_from(u32::from_be_bytes([
            bytes[at],
            bytes[at + 1],
            bytes[at + 2],
            bytes[at + 3],
        ]))
        .expect("length");
        let kind = String::from_utf8_lossy(&bytes[at + 4..at + 8]).into_owned();
        if kinds.last() != Some(&kind) {
            kinds.push(kind);
        }
        at += 12 + length;
    }
    assert_eq!(
        kinds,
        ["IHDR", "IDAT", "IEND"],
        "no metadata chunk and no extra layer"
    );
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    assert_eq!(
        (i32::try_from(width), i32::try_from(height)),
        (Ok(WIDTH), Ok(HEIGHT))
    );
}

#[test]
fn rendering_suite() {
    let Some(rig) = rig() else {
        return;
    };
    untouched_image_is_reproduced_exactly(&rig);
    outlines_and_lines_are_drawn_where_they_belong(&rig);
    arrows_follow_their_curve(&rig);
    translucent_arrows_are_one_homogeneous_shape(&rig);
    text_steps_and_strokes_are_drawn(&rig);
    translucent_effects_blend_with_the_image(&rig);
    the_spotlight_dims_everything_around_it(&rig);
    the_rounded_spotlight_dims_its_corners_only(&rig);
    a_turned_rectangle_is_drawn_turned_around_its_centre(&rig);
    a_turned_blur_and_a_turned_pixelation_cover_exactly_their_turned_area(&rig);
    a_turned_blur_on_the_image_edge_stays_opaque(&rig);
    turned_points_draw_a_line_and_a_stroke_where_they_were_turned(&rig);
    a_bubble_is_drawn_with_a_filled_body_an_outline_and_a_tail(&rig);
    the_cartoon_lettering_is_really_another_font(&rig);
    layers_follow_their_order(&rig);
    a_crop_keeps_the_region_and_its_coordinates(&rig);
    blur_covers_only_its_region_and_follows_it(&rig);
    a_blur_stays_opaque_near_the_edges_of_the_image(&rig);
    pixelation_makes_uniform_cells_and_follows_its_region(&rig);
    an_annotation_after_a_mask_stays_sharp(&rig);
    the_precise_look_is_the_look_of_every_older_shape(&rig);
    a_hand_drawn_shape_is_always_drawn_the_same_way(&rig);
    a_hand_drawn_shape_differs_from_the_precise_one_but_stays_near_it(&rig);
    a_duplicate_looks_exactly_like_its_original(&rig);
    a_hand_drawn_look_changes_with_the_seed_of_the_shape(&rig);
    a_dashed_pen_stroke_leaves_gaps_and_a_solid_one_does_not(&rig);
    the_export_matches_the_preview(&rig);
    hidden_objects_are_not_drawn_and_locked_ones_are(&rig);
    the_color_sampler_reads_the_pixel_it_is_given(&rig);
    the_png_export_carries_pixels_and_nothing_else(&rig);
    crate::editor::panel_tests::the_panels_only_change_what_is_visible();
    crate::editor::panel_tests::hiding_an_effect_shows_nothing_and_leaves_the_canvas_alone();
    crate::editor::panel_tests::the_sidebar_button_stays_where_it_is();
    crate::editor::panel_tests::the_layers_can_always_be_reached_after_folding_and_unfolding_the_panels();
    crate::zoom_comparison_tests::the_original_fades_in_inside_the_frame_of_the_annotated_image(
        &rig.renderer,
    );
}
