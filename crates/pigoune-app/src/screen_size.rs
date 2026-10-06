use gettextrs::gettext;
use gtk::gdk;
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

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenChoice {
    pub name: String,
    pub size: Dimensions,
    pub scale: f64,
}

#[must_use]
pub fn screens_of(widget: &impl IsA<gtk::Widget>) -> (Vec<ScreenChoice>, usize) {
    let display = widget.as_ref().display();
    let current = widget
        .as_ref()
        .root()
        .and_then(|root| root.surface())
        .and_then(|surface| display.monitor_at_surface(&surface));
    let monitors = display.monitors();
    let mut screens = Vec::new();
    let mut connectors = Vec::new();
    let mut shown = 0;
    for position in 0..monitors.n_items() {
        let Some(monitor) = monitors.item(position).and_downcast::<gdk::Monitor>() else {
            continue;
        };
        let geometry = monitor.geometry();
        let scale = valid_scale(monitor.scale());
        let Some(size) = Dimensions::new(
            in_pixels(geometry.width(), scale),
            in_pixels(geometry.height(), scale),
        ) else {
            continue;
        };
        if current.as_ref() == Some(&monitor) {
            shown = screens.len();
        }
        let name = monitor_name(
            monitor.description().as_deref(),
            monitor.connector().as_deref(),
            screens.len() + 1,
        );
        connectors.push(monitor.connector().map(|connector| connector.to_string()));
        screens.push(ScreenChoice { name, size, scale });
    }
    let names: Vec<String> = screens.iter().map(|screen| screen.name.clone()).collect();
    for (screen, name) in screens.iter_mut().zip(distinct_names(&names, &connectors)) {
        screen.name = name;
    }
    (screens, shown)
}

#[must_use]
pub fn distinct_names(names: &[String], connectors: &[Option<String>]) -> Vec<String> {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let shared = names.iter().filter(|other| *other == name).count() > 1;
            if !shared {
                return name.clone();
            }
            let detail = connectors
                .get(index)
                .cloned()
                .flatten()
                .filter(|connector| !connector.trim().is_empty())
                .unwrap_or_else(|| (index + 1).to_string());
            gettext("{name} ({detail})")
                .replace("{name}", name)
                .replace("{detail}", &detail)
        })
        .collect()
}

#[must_use]
pub fn monitor_name(description: Option<&str>, connector: Option<&str>, number: usize) -> String {
    description
        .or(connector)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map_or_else(
            || gettext("Screen {number}").replace("{number}", &number.to_string()),
            ToOwned::to_owned,
        )
}

#[must_use]
pub fn screen_label(screen: &ScreenChoice) -> String {
    gettext("{name} · {width} × {height}")
        .replace("{name}", &screen.name)
        .replace("{width}", &screen.size.width().to_string())
        .replace("{height}", &screen.size.height().to_string())
}

fn valid_scale(scale: f64) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::{distinct_names, monitor_name};

    #[test]
    fn a_screen_is_named_by_its_description_then_its_connector() {
        assert_eq!(
            monitor_name(Some("Built-in display"), Some("eDP-1"), 1),
            "Built-in display"
        );
        assert_eq!(monitor_name(None, Some("HDMI-1"), 2), "HDMI-1");
        assert_eq!(monitor_name(Some("  "), None, 3), "Screen 3");
    }

    #[test]
    fn identical_screens_are_told_apart_by_their_connector() {
        let names = ["Dell".to_owned(), "Dell".to_owned(), "LG".to_owned()];
        let connectors = [Some("DP-1".to_owned()), None, Some("HDMI-1".to_owned())];
        assert_eq!(
            distinct_names(&names, &connectors),
            ["Dell (DP-1)", "Dell (2)", "LG"]
        );
    }
}
