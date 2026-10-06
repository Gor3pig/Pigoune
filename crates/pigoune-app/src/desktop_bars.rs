use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};

const GNOME_BAR_HEIGHT: f64 = 32.0;
const GNOME_TEXT_SIZE: f64 = 13.5;
const GNOME_PADDING: f64 = 12.0;
const GNOME_WORKSPACES_WIDTH: f64 = 48.0;
const GNOME_WORKSPACES_HEIGHT: f64 = 16.0;
const GNOME_WORKSPACES_OPACITY: f32 = 0.2;
const GNOME_STATUS_DOT: f64 = 8.0;
const KDE_PANEL_HEIGHT: f64 = 44.0;
const KDE_PANEL_COLOR: (f32, f32, f32, f32) = (0.125, 0.137, 0.149, 0.95);
const KDE_ACCENT: (f32, f32, f32) = (0.239, 0.682, 0.914);
const KDE_PADDING: f64 = 8.0;
const KDE_LAUNCHER: f64 = 26.0;
const KDE_TASK: f64 = 32.0;
const KDE_TASKS: u32 = 3;
const KDE_TASK_OPACITY: f32 = 0.18;
const KDE_TRAY_DOT: f64 = 8.0;
const KDE_TIME_SIZE: f64 = 12.0;
const KDE_DATE_SIZE: f64 = 9.0;
const KDE_CLOCK_WIDTH: f64 = 72.0;
const STATUS_DOTS: u32 = 3;
const KDE_DESKTOP_NAMES: [&str; 2] = ["KDE", "PLASMA"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
}

impl Desktop {
    pub const ALL: [Self; 2] = [Self::Gnome, Self::Kde];

    pub fn name(self) -> &'static str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE Plasma",
        }
    }

    pub fn detected() -> Self {
        let current = glib::getenv("XDG_CURRENT_DESKTOP")
            .and_then(|value| value.into_string().ok())
            .unwrap_or_default();
        desktop_named(&current)
    }
}

fn desktop_named(current: &str) -> Desktop {
    let upper = current.to_uppercase();
    if upper
        .split(':')
        .any(|name| KDE_DESKTOP_NAMES.contains(&name))
    {
        Desktop::Kde
    } else {
        Desktop::Gnome
    }
}

pub struct Bars<'a> {
    pub widget: &'a gtk::Widget,
    pub screen: graphene::Rect,
    pub points: f64,
}

impl Bars<'_> {
    pub fn draw(&self, snapshot: &gtk::Snapshot, desktop: Desktop) {
        match desktop {
            Desktop::Gnome => self.draw_gnome(snapshot),
            Desktop::Kde => self.draw_kde(snapshot),
        }
    }

    fn size(&self, points: f64) -> f64 {
        points * self.points
    }

    fn left(&self) -> f64 {
        f64::from(self.screen.x())
    }

    fn right(&self) -> f64 {
        f64::from(self.screen.x() + self.screen.width())
    }

    fn draw_gnome(&self, snapshot: &gtk::Snapshot) {
        let height = self.size(GNOME_BAR_HEIGHT);
        let top = f64::from(self.screen.y());
        snapshot.append_color(
            &gdk::RGBA::BLACK,
            &rect(self.left(), top, f64::from(self.screen.width()), height),
        );
        let middle = top + height / 2.0;
        let padding = self.size(GNOME_PADDING);
        let workspaces = self.size(GNOME_WORKSPACES_HEIGHT);
        fill_rounded(
            snapshot,
            &rect(
                self.left() + padding,
                middle - workspaces / 2.0,
                self.size(GNOME_WORKSPACES_WIDTH),
                workspaces,
            ),
            &gdk::RGBA::WHITE.with_alpha(GNOME_WORKSPACES_OPACITY),
        );
        draw_dots(
            snapshot,
            self.right() - padding,
            middle,
            self.size(GNOME_STATUS_DOT),
            &gdk::RGBA::WHITE,
        );
        let layout = self.layout(&gnome_clock_text(), GNOME_TEXT_SIZE, true);
        let (width, text_height) = pixel_size(&layout);
        let center = self.left() + f64::from(self.screen.width()) / 2.0;
        draw_layout(
            snapshot,
            &layout,
            center - width / 2.0,
            middle - text_height / 2.0,
        );
    }

    fn draw_kde(&self, snapshot: &gtk::Snapshot) {
        let height = self.size(KDE_PANEL_HEIGHT);
        let top = f64::from(self.screen.y() + self.screen.height()) - height;
        let (red, green, blue, alpha) = KDE_PANEL_COLOR;
        snapshot.append_color(
            &gdk::RGBA::new(red, green, blue, alpha),
            &rect(self.left(), top, f64::from(self.screen.width()), height),
        );
        let middle = top + height / 2.0;
        let padding = self.size(KDE_PADDING);
        let launcher = self.size(KDE_LAUNCHER);
        let (red, green, blue) = KDE_ACCENT;
        let accent = gdk::RGBA::new(red, green, blue, 1.0);
        fill_rounded(
            snapshot,
            &rect(
                self.left() + padding,
                middle - launcher / 2.0,
                launcher,
                launcher,
            ),
            &accent,
        );
        let task = self.size(KDE_TASK);
        let mut x = self.left() + padding * 2.0 + launcher;
        for index in 0..KDE_TASKS {
            let color = if index == 0 {
                accent.with_alpha(KDE_TASK_OPACITY * 2.0)
            } else {
                gdk::RGBA::WHITE.with_alpha(KDE_TASK_OPACITY)
            };
            fill_corners(
                snapshot,
                &rect(x, middle - task / 2.0, task, task),
                padding / 2.0,
                &color,
            );
            x += task + padding / 2.0;
        }
        let clock_width = self.size(KDE_CLOCK_WIDTH);
        let clock_center = self.right() - padding - clock_width / 2.0;
        draw_dots(
            snapshot,
            self.right() - padding * 2.0 - clock_width,
            middle,
            self.size(KDE_TRAY_DOT),
            &gdk::RGBA::WHITE,
        );
        let now = glib::DateTime::now_local().ok();
        let time = self.layout(&formatted(now.as_ref(), "%H:%M"), KDE_TIME_SIZE, false);
        let date = self.layout(&formatted(now.as_ref(), "%x"), KDE_DATE_SIZE, false);
        let (time_width, time_height) = pixel_size(&time);
        let (date_width, date_height) = pixel_size(&date);
        let text_top = middle - f64::midpoint(time_height, date_height);
        draw_layout(snapshot, &time, clock_center - time_width / 2.0, text_top);
        draw_layout(
            snapshot,
            &date,
            clock_center - date_width / 2.0,
            text_top + time_height,
        );
    }

    fn layout(&self, text: &str, points: f64, bold: bool) -> pango::Layout {
        let layout = self.widget.create_pango_layout(Some(text));
        let mut font = pango::FontDescription::new();
        font.set_family("Sans");
        if bold {
            font.set_weight(pango::Weight::Bold);
        }
        font.set_absolute_size(self.size(points) * f64::from(pango::SCALE));
        layout.set_font_description(Some(&font));
        layout
    }
}

fn draw_dots(snapshot: &gtk::Snapshot, right: f64, middle: f64, dot: f64, color: &gdk::RGBA) {
    for index in 1..=STATUS_DOTS {
        let left = right - f64::from(index) * dot * 2.0 + dot;
        fill_rounded(snapshot, &rect(left, middle - dot / 2.0, dot, dot), color);
    }
}

fn gnome_clock_text() -> String {
    let now = glib::DateTime::now_local().ok();
    let day = now
        .as_ref()
        .map(|now| now.day_of_month().to_string())
        .unwrap_or_default();
    format!(
        "{} {} {}  {}",
        formatted(now.as_ref(), "%a"),
        day,
        formatted(now.as_ref(), "%b"),
        formatted(now.as_ref(), "%H:%M")
    )
}

fn formatted(now: Option<&glib::DateTime>, format: &str) -> String {
    now.and_then(|now| now.format(format).ok())
        .map(|text| text.to_string())
        .unwrap_or_default()
}

fn pixel_size(layout: &pango::Layout) -> (f64, f64) {
    let (width, height) = layout.pixel_size();
    (f64::from(width), f64::from(height))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "drawing coordinates fit easily in f32"
)]
fn draw_layout(snapshot: &gtk::Snapshot, layout: &pango::Layout, x: f64, y: f64) {
    snapshot.save();
    snapshot.translate(&graphene::Point::new(x as f32, y as f32));
    snapshot.append_layout(layout, &gdk::RGBA::WHITE);
    snapshot.restore();
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "drawing coordinates fit easily in f32"
)]
fn rect(x: f64, y: f64, width: f64, height: f64) -> graphene::Rect {
    graphene::Rect::new(x as f32, y as f32, width as f32, height as f32)
}

fn fill_rounded(snapshot: &gtk::Snapshot, area: &graphene::Rect, color: &gdk::RGBA) {
    fill_corners(snapshot, area, f64::from(area.height()) / 2.0, color);
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "corner radii fit easily in f32"
)]
fn fill_corners(snapshot: &gtk::Snapshot, area: &graphene::Rect, radius: f64, color: &gdk::RGBA) {
    let corner = graphene::Size::new(radius as f32, radius as f32);
    snapshot.push_rounded_clip(&gsk::RoundedRect::new(
        *area, corner, corner, corner, corner,
    ));
    snapshot.append_color(color, area);
    snapshot.pop();
}

#[cfg(test)]
mod tests {
    use super::{Desktop, desktop_named};

    #[test]
    fn the_desktop_in_use_is_recognized() {
        assert_eq!(desktop_named("KDE"), Desktop::Kde);
        assert_eq!(desktop_named("ubuntu:GNOME"), Desktop::Gnome);
        assert_eq!(desktop_named(""), Desktop::Gnome);
        assert_eq!(desktop_named("plasma"), Desktop::Kde);
    }
}
