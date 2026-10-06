use gettextrs::gettext;
use gtk::prelude::*;
use pigoune_core::Dimensions;

#[must_use]
pub fn screen_of(widget: &impl IsA<gtk::Widget>) -> Option<Dimensions> {
    let root = widget.as_ref().root()?;
    let surface = root.surface()?;
    let monitor = widget.as_ref().display().monitor_at_surface(&surface)?;
    let geometry = monitor.geometry();
    let scale = monitor.scale();
    Dimensions::new(
        in_pixels(geometry.width(), scale),
        in_pixels(geometry.height(), scale),
    )
}

#[must_use]
pub fn monitor_scale_of(widget: &impl IsA<gtk::Widget>) -> f64 {
    let scale = widget
        .as_ref()
        .root()
        .and_then(|root| root.surface())
        .and_then(|surface| widget.as_ref().display().monitor_at_surface(&surface))
        .map_or(1.0, |monitor| monitor.scale());
    if scale > 0.0 { scale } else { 1.0 }
}

#[must_use]
pub fn at_least(screen: Dimensions) -> String {
    gettext("{width} × {height} or larger")
        .replace("{width}", &screen.width().to_string())
        .replace("{height}", &screen.height().to_string())
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the side is rounded and kept between zero and the largest side"
)]
fn in_pixels(logical: i32, scale: f64) -> u32 {
    (f64::from(logical) * scale)
        .round()
        .clamp(0.0, f64::from(u32::MAX)) as u32
}
