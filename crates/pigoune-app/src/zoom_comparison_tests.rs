use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::compare_hold::{Region, WHOLE};
use crate::zoom_view::PigouneZoomView;

const SIDE: i32 = 20;

fn texture(left: [u8; 4], right: [u8; 4]) -> gdk::Texture {
    let mut bytes = Vec::new();
    for _ in 0..SIDE {
        for x in 0..SIDE {
            bytes.extend_from_slice(if x < SIDE / 2 { &left } else { &right });
        }
    }
    gdk::MemoryTexture::new(
        SIDE,
        SIDE,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(bytes),
        usize::try_from(SIDE * 4).expect("stride"),
    )
    .upcast()
}

fn settle() {
    let context = glib::MainContext::default();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(100) {
        context.iteration(false);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "small test coordinates"
)]
fn pixel_at(display: &gsk::Renderer, view: &PigouneZoomView, x: f32, y: f32) -> [u8; 3] {
    let paintable = gtk::WidgetPaintable::new(Some(view));
    let node = (0..30)
        .find_map(|_| {
            view.queue_draw();
            settle();
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(&snapshot, f64::from(view.width()), f64::from(view.height()));
            snapshot.to_node()
        })
        .expect("the widget is drawn");
    let area = graphene::Rect::new(0.0, 0.0, view.width() as f32, view.height() as f32);
    let picture = display.render_texture(&node, Some(&area));
    let mut downloader = gdk::TextureDownloader::new(&picture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let index = y as usize * stride + x as usize * 4;
    [bytes[index], bytes[index + 1], bytes[index + 2]]
}

pub fn the_original_fades_in_inside_the_frame_of_the_annotated_image(renderer: &gsk::Renderer) {
    let window = gtk::Window::new();
    window.set_default_size(300, 300);
    let view = PigouneZoomView::default();
    window.set_child(Some(&view));
    let red = [220, 0, 0, 255];
    let blue = [0, 0, 220, 255];
    let green = [0, 200, 0, 255];
    let yellow = [230, 230, 0, 255];
    view.show_image(Some(&texture(red, red)), 100, 100, false);
    window.present();
    settle();
    let bounds = view.image_bounds();
    let (cx, cy) = (
        bounds.x() + bounds.width() / 2.0,
        bounds.y() + bounds.height() / 2.0,
    );
    let plain = pixel_at(renderer, &view, cx, cy);
    assert_eq!(plain, [220, 0, 0]);
    let zoom = view.zoom();

    view.set_comparison(Some((texture(blue, blue), WHOLE)));
    assert_eq!(
        pixel_at(renderer, &view, cx, cy),
        plain,
        "no progress, no change"
    );
    view.set_comparison_progress(1.0);
    assert_eq!(
        pixel_at(renderer, &view, cx, cy),
        [0, 0, 220],
        "the original shows"
    );
    view.set_comparison_progress(0.5);
    let middle = pixel_at(renderer, &view, cx, cy);
    assert!(
        middle[0] > 80 && middle[2] > 80,
        "a fade between both: {middle:?}"
    );
    view.set_comparison_progress(0.0);
    assert_eq!(
        pixel_at(renderer, &view, cx, cy),
        plain,
        "released: the annotated image"
    );
    assert_eq!(view.image_bounds(), bounds, "the frame never moves");
    assert!(
        (view.zoom() - zoom).abs() < f64::EPSILON,
        "the zoom never changes"
    );

    let half = Region {
        x: 0.5,
        y: 0.0,
        width: 0.5,
        height: 1.0,
    };
    view.set_comparison(Some((texture(green, yellow), half)));
    view.set_comparison_progress(1.0);
    for (x, y) in [
        (bounds.x() + 3.0, bounds.y() + 3.0),
        (cx, cy),
        (
            bounds.x() + bounds.width() - 3.0,
            bounds.y() + bounds.height() - 3.0,
        ),
    ] {
        assert_eq!(
            pixel_at(renderer, &view, x, y),
            [230, 230, 0],
            "a cropped original shows only the cropped region, in the same frame"
        );
    }
    view.set_comparison_progress(0.0);
    assert_eq!(view.image_bounds(), bounds);
    window.close();
}
