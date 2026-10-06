use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};

pub use crate::desktop_panels::Desktop;
use crate::desktop_panels::{Edge, Item, Paint, Panel};

const GAP: f64 = 6.0;
const PADDING: f64 = 10.0;
const PILL_GAP: f64 = 4.0;
const PILL_PADDING: f64 = 8.0;
const PILL_HEIGHT_SHARE: f64 = 0.6;
const PILL_OPACITY: f32 = 0.2;
const DOCK_MARGIN: f64 = 6.0;
const DOCK_RADIUS: f64 = 14.0;
const TIME_SIZE: f64 = 12.0;
const DATE_SIZE: f64 = 9.0;

pub struct Bars<'a> {
    pub widget: &'a gtk::Widget,
    pub screen: graphene::Rect,
    pub points: f64,
}

struct Line {
    x: f64,
    middle: f64,
    height: f64,
    foreground: gdk::RGBA,
}

impl Bars<'_> {
    pub fn draw(&self, snapshot: &gtk::Snapshot, desktop: Desktop) {
        for panel in desktop.panels() {
            self.draw_panel(snapshot, &panel);
        }
    }

    fn size(&self, points: f64) -> f64 {
        points * self.points
    }

    fn draw_panel(&self, snapshot: &gtk::Snapshot, panel: &Panel) {
        let height = self.size(panel.height);
        let padding = self.size(PADDING);
        let gap = self.size(GAP);
        let widths: Vec<f64> = panel.items.iter().map(|item| self.width_of(item)).collect();
        let gaps = gap * f64::from(u32::try_from(widths.len().saturating_sub(1)).unwrap_or(0));
        let content = widths.iter().sum::<f64>() + gaps;
        let screen_left = f64::from(self.screen.x());
        let screen_width = f64::from(self.screen.width());
        let screen_top = f64::from(self.screen.y());
        let screen_bottom = screen_top + f64::from(self.screen.height());
        let (red, green, blue) = panel.foreground;
        let foreground = gdk::RGBA::new(red, green, blue, 1.0);
        let background = paint(panel.background, &foreground);
        let (left, width, top) = if panel.floating {
            let width = content + padding * 2.0;
            let top = screen_bottom - self.size(DOCK_MARGIN) - height;
            let left = screen_left + (screen_width - width) / 2.0;
            fill_corners(
                snapshot,
                &rect(left, top, width, height),
                self.size(DOCK_RADIUS),
                &background,
            );
            (left, width, top)
        } else {
            let top = match panel.edge {
                Edge::Top => screen_top,
                Edge::Bottom => screen_bottom - height,
            };
            snapshot.append_color(&background, &rect(screen_left, top, screen_width, height));
            (screen_left, screen_width, top)
        };
        let spacers = panel
            .items
            .iter()
            .filter(|item| matches!(item, Item::Spacer))
            .count();
        let free = (width - padding * 2.0 - content).max(0.0)
            / f64::from(u32::try_from(spacers.max(1)).unwrap_or(1));
        let mut line = Line {
            x: left + padding,
            middle: top + height / 2.0,
            height,
            foreground,
        };
        for (item, item_width) in panel.items.iter().zip(widths) {
            self.draw_item(snapshot, item, &line);
            line.x += item_width + gap;
            if matches!(item, Item::Spacer) {
                line.x += free;
            }
        }
    }

    fn width_of(&self, item: &Item) -> f64 {
        match item {
            Item::Block { width, .. } => self.size(*width),
            Item::Text { text, size, bold } => pixel_size(&self.layout(text, *size, *bold)).0,
            Item::TwoLines { top, bottom } => pixel_size(&self.layout(top, TIME_SIZE, false))
                .0
                .max(pixel_size(&self.layout(bottom, DATE_SIZE, false)).0),
            Item::Pill(children) => self.pill_width(children),
            Item::Space(points) => self.size(*points),
            Item::Spacer => 0.0,
        }
    }

    fn pill_width(&self, children: &[Item]) -> f64 {
        let gaps = self.size(PILL_GAP)
            * f64::from(u32::try_from(children.len().saturating_sub(1)).unwrap_or(0));
        children
            .iter()
            .map(|child| self.width_of(child))
            .sum::<f64>()
            + gaps
            + self.size(PILL_PADDING) * 2.0
    }

    fn draw_item(&self, snapshot: &gtk::Snapshot, item: &Item, line: &Line) {
        match item {
            Item::Block {
                width,
                height,
                radius,
                paint: item_paint,
            } => {
                let (width, height) = (self.size(*width), self.size(*height));
                fill_corners(
                    snapshot,
                    &rect(line.x, line.middle - height / 2.0, width, height),
                    self.size(*radius),
                    &paint(*item_paint, &line.foreground),
                );
            }
            Item::Text { text, size, bold } => {
                let layout = self.layout(text, *size, *bold);
                let (_, text_height) = pixel_size(&layout);
                draw_layout(
                    snapshot,
                    &layout,
                    line.x,
                    line.middle - text_height / 2.0,
                    &line.foreground,
                );
            }
            Item::TwoLines { top, bottom } => self.draw_two_lines(snapshot, top, bottom, line),
            Item::Pill(children) => self.draw_pill(snapshot, children, line),
            Item::Space(_) | Item::Spacer => {}
        }
    }

    fn draw_two_lines(&self, snapshot: &gtk::Snapshot, top: &str, bottom: &str, line: &Line) {
        let first = self.layout(top, TIME_SIZE, false);
        let second = self.layout(bottom, DATE_SIZE, false);
        let (first_width, first_height) = pixel_size(&first);
        let (second_width, second_height) = pixel_size(&second);
        let width = first_width.max(second_width);
        let text_top = line.middle - f64::midpoint(first_height, second_height);
        draw_layout(
            snapshot,
            &first,
            line.x + (width - first_width) / 2.0,
            text_top,
            &line.foreground,
        );
        draw_layout(
            snapshot,
            &second,
            line.x + (width - second_width) / 2.0,
            text_top + first_height,
            &line.foreground,
        );
    }

    fn draw_pill(&self, snapshot: &gtk::Snapshot, children: &[Item], line: &Line) {
        let height = line.height * PILL_HEIGHT_SHARE;
        fill_corners(
            snapshot,
            &rect(
                line.x,
                line.middle - height / 2.0,
                self.pill_width(children),
                height,
            ),
            height / 2.0,
            &line.foreground.with_alpha(PILL_OPACITY),
        );
        let mut inner = Line {
            x: line.x + self.size(PILL_PADDING),
            middle: line.middle,
            height,
            foreground: line.foreground,
        };
        for child in children {
            self.draw_item(snapshot, child, &inner);
            inner.x += self.width_of(child) + self.size(PILL_GAP);
        }
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

fn paint(paint: Paint, foreground: &gdk::RGBA) -> gdk::RGBA {
    match paint {
        Paint::Foreground(alpha) => foreground.with_alpha(alpha),
        Paint::Color(red, green, blue, alpha) => gdk::RGBA::new(red, green, blue, alpha),
    }
}

fn pixel_size(layout: &pango::Layout) -> (f64, f64) {
    let (width, height) = layout.pixel_size();
    (f64::from(width), f64::from(height))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "drawing coordinates fit easily in f32"
)]
fn draw_layout(
    snapshot: &gtk::Snapshot,
    layout: &pango::Layout,
    x: f64,
    y: f64,
    color: &gdk::RGBA,
) {
    snapshot.save();
    snapshot.translate(&graphene::Point::new(x as f32, y as f32));
    snapshot.append_layout(layout, color);
    snapshot.restore();
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "drawing coordinates fit easily in f32"
)]
fn rect(x: f64, y: f64, width: f64, height: f64) -> graphene::Rect {
    graphene::Rect::new(x as f32, y as f32, width as f32, height as f32)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "corner radii fit easily in f32"
)]
fn fill_corners(snapshot: &gtk::Snapshot, area: &graphene::Rect, radius: f64, color: &gdk::RGBA) {
    let radius = radius.min(f64::from(area.height()) / 2.0) as f32;
    let corner = graphene::Size::new(radius, radius);
    snapshot.push_rounded_clip(&gsk::RoundedRect::new(
        *area, corner, corner, corner, corner,
    ));
    snapshot.append_color(color, area);
    snapshot.pop();
}
