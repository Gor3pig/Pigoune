use gettextrs::pgettext;
use gtk::glib;

const WHITE: Paint = Paint::Foreground(1.0);
const FAINT: Paint = Paint::Foreground(0.35);
const BRIGHT: Paint = Paint::Foreground(0.6);
const STATUS_DOTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
    Cinnamon,
    Xfce,
    Mate,
    Cosmic,
    Budgie,
}

impl Desktop {
    pub const ALL: [Self; 7] = [
        Self::Gnome,
        Self::Kde,
        Self::Cinnamon,
        Self::Xfce,
        Self::Mate,
        Self::Cosmic,
        Self::Budgie,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE Plasma",
            Self::Cinnamon => "Cinnamon",
            Self::Xfce => "Xfce",
            Self::Mate => "MATE",
            Self::Cosmic => "COSMIC",
            Self::Budgie => "Budgie",
        }
    }

    pub fn detected() -> Self {
        let current = glib::getenv("XDG_CURRENT_DESKTOP")
            .and_then(|value| value.into_string().ok())
            .unwrap_or_default();
        desktop_named(&current)
    }

    fn session_names(self) -> &'static [&'static str] {
        match self {
            Self::Gnome => &["GNOME"],
            Self::Kde => &["KDE", "PLASMA"],
            Self::Cinnamon => &["X-CINNAMON", "CINNAMON"],
            Self::Xfce => &["XFCE"],
            Self::Mate => &["MATE"],
            Self::Cosmic => &["COSMIC"],
            Self::Budgie => &["BUDGIE"],
        }
    }

    pub fn panels(self) -> Vec<Panel> {
        match self {
            Self::Gnome => gnome(),
            Self::Kde => kde(),
            Self::Cinnamon => cinnamon(),
            Self::Xfce => xfce(),
            Self::Mate => mate(),
            Self::Cosmic => cosmic(),
            Self::Budgie => budgie(),
        }
    }
}

fn desktop_named(current: &str) -> Desktop {
    let names: Vec<String> = current.split(':').map(str::to_uppercase).collect();
    Desktop::ALL
        .into_iter()
        .rev()
        .find(|desktop| {
            desktop
                .session_names()
                .iter()
                .any(|known| names.iter().any(|name| name == known))
        })
        .unwrap_or(Desktop::Gnome)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paint {
    Foreground(f32),
    Color(f32, f32, f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Block {
        width: f64,
        height: f64,
        radius: f64,
        paint: Paint,
    },
    Text {
        text: String,
        size: f64,
        bold: bool,
    },
    TwoLines {
        top: String,
        bottom: String,
    },
    Pill(Vec<Item>),
    Space(f64),
    Spacer,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    pub edge: Edge,
    pub height: f64,
    pub background: Paint,
    pub foreground: (f32, f32, f32),
    pub floating: bool,
    pub items: Vec<Item>,
}

fn square(size: f64, paint: Paint) -> Item {
    Item::Block {
        width: size,
        height: size,
        radius: size / 6.0,
        paint,
    }
}

fn disc(size: f64, paint: Paint) -> Item {
    Item::Block {
        width: size,
        height: size,
        radius: size / 2.0,
        paint,
    }
}

fn text(text: String, size: f64, bold: bool) -> Item {
    Item::Text { text, size, bold }
}

fn dots(size: f64) -> Vec<Item> {
    vec![disc(size, WHITE); STATUS_DOTS]
}

fn squares(count: usize, size: f64, highlighted: Option<usize>) -> Vec<Item> {
    (0..count)
        .map(|index| {
            let paint = if Some(index) == highlighted {
                BRIGHT
            } else {
                FAINT
            };
            square(size, paint)
        })
        .collect()
}

fn bar(edge: Edge, height: f64, background: Paint, items: Vec<Item>) -> Panel {
    Panel {
        edge,
        height,
        background,
        foreground: (1.0, 1.0, 1.0),
        floating: false,
        items,
    }
}

fn dock(height: f64, background: Paint, items: Vec<Item>) -> Panel {
    Panel {
        floating: true,
        ..bar(Edge::Bottom, height, background, items)
    }
}

fn gray(level: f32, alpha: f32) -> Paint {
    Paint::Color(level, level, level, alpha)
}

fn clock(format: &str) -> String {
    let Ok(now) = glib::DateTime::now_local() else {
        return String::new();
    };
    let part = |code: &str| {
        now.format(code)
            .map(|value| value.to_string())
            .unwrap_or_default()
    };
    format
        .replace("{weekday}", &part("%a"))
        .replace("{day}", &now.day_of_month().to_string())
        .replace("{month}", &part("%b"))
        .replace("{time}", &part("%H:%M"))
        .replace("{date}", &part("%x"))
}

fn gnome() -> Vec<Panel> {
    let mut items = vec![
        Item::Pill(vec![
            Item::Block {
                width: 14.0,
                height: 6.0,
                radius: 3.0,
                paint: WHITE,
            },
            disc(6.0, WHITE),
            disc(6.0, WHITE),
        ]),
        Item::Spacer,
        text(clock("{weekday} {day} {month}  {time}"), 13.5, true),
        Item::Spacer,
    ];
    items.extend(dots(8.0));
    vec![bar(Edge::Top, 32.0, gray(0.0, 1.0), items)]
}

fn kde() -> Vec<Panel> {
    let accent = Paint::Color(0.239, 0.682, 0.914, 1.0);
    let mut items = vec![disc(26.0, accent), Item::Space(4.0)];
    items.extend(squares(3, 32.0, Some(0)));
    items.push(Item::Spacer);
    items.extend(dots(8.0));
    items.push(Item::Space(6.0));
    items.push(Item::TwoLines {
        top: clock("{time}"),
        bottom: clock("{date}"),
    });
    vec![bar(
        Edge::Bottom,
        44.0,
        Paint::Color(0.125, 0.137, 0.149, 0.95),
        items,
    )]
}

fn cinnamon() -> Vec<Panel> {
    let mut items = vec![
        square(24.0, Paint::Color(0.545, 0.694, 0.345, 1.0)),
        Item::Space(4.0),
    ];
    items.extend(squares(3, 22.0, None));
    items.push(Item::Space(10.0));
    items.extend(squares(2, 22.0, Some(0)));
    items.push(Item::Spacer);
    items.extend(dots(8.0));
    items.push(Item::Space(4.0));
    items.push(text(clock("{time}"), 13.0, false));
    vec![bar(Edge::Bottom, 40.0, gray(0.17, 1.0), items)]
}

fn xfce() -> Vec<Panel> {
    let mut top = vec![
        square(14.0, FAINT),
        text(pgettext("desktop bar", "Applications"), 12.0, true),
        Item::Spacer,
        Item::Pill(vec![square(12.0, BRIGHT), square(12.0, FAINT)]),
    ];
    top.extend(dots(7.0));
    top.push(text(clock("{time}"), 12.0, false));
    vec![
        bar(Edge::Top, 28.0, gray(0.21, 1.0), top),
        dock(48.0, gray(0.21, 0.9), squares(5, 32.0, None)),
    ]
}

fn mate() -> Vec<Panel> {
    let light = gray(0.93, 1.0);
    let ink = (0.18, 0.2, 0.21);
    let mut top = vec![
        text(pgettext("desktop bar", "Applications"), 12.0, false),
        Item::Space(4.0),
        text(pgettext("desktop bar", "Places"), 12.0, false),
        Item::Space(4.0),
        text(pgettext("desktop bar", "System"), 12.0, false),
        Item::Spacer,
    ];
    top.extend(dots(7.0));
    top.push(text(clock("{weekday} {day} {month}, {time}"), 12.0, false));
    let bottom = vec![
        square(14.0, gray(0.72, 1.0)),
        Item::Block {
            width: 140.0,
            height: 16.0,
            radius: 2.0,
            paint: gray(0.83, 1.0),
        },
        Item::Spacer,
        square(14.0, Paint::Color(0.54, 0.71, 0.35, 1.0)),
        square(14.0, gray(0.83, 1.0)),
    ];
    vec![
        Panel {
            foreground: ink,
            ..bar(Edge::Top, 24.0, light, top)
        },
        Panel {
            foreground: ink,
            ..bar(Edge::Bottom, 24.0, light, bottom)
        },
    ]
}

fn cosmic() -> Vec<Panel> {
    let mut top = vec![
        Item::Pill(vec![text(
            pgettext("desktop bar", "Workspaces"),
            12.0,
            true,
        )]),
        Item::Pill(vec![text(
            pgettext("desktop bar", "Applications"),
            12.0,
            true,
        )]),
        Item::Spacer,
        text(clock("{day} {month} {time}"), 13.0, true),
        Item::Spacer,
    ];
    top.extend(dots(8.0));
    vec![
        bar(Edge::Top, 36.0, gray(0.106, 0.92), top),
        dock(56.0, gray(0.106, 0.88), squares(6, 36.0, None)),
    ]
}

fn budgie() -> Vec<Panel> {
    let mut items = vec![
        square(20.0, Paint::Color(0.32, 0.58, 0.89, 1.0)),
        Item::Space(4.0),
    ];
    items.extend(squares(3, 22.0, Some(0)));
    items.push(Item::Spacer);
    items.extend(dots(8.0));
    items.push(Item::Space(4.0));
    items.push(text(clock("{time}"), 13.0, false));
    vec![bar(Edge::Top, 32.0, gray(0.114, 1.0), items)]
}

#[cfg(test)]
mod tests {
    use super::{Desktop, Edge, desktop_named};

    #[test]
    fn the_desktop_in_use_is_recognized() {
        assert_eq!(desktop_named("KDE"), Desktop::Kde);
        assert_eq!(desktop_named("ubuntu:GNOME"), Desktop::Gnome);
        assert_eq!(desktop_named("X-Cinnamon"), Desktop::Cinnamon);
        assert_eq!(desktop_named("XFCE"), Desktop::Xfce);
        assert_eq!(desktop_named("MATE"), Desktop::Mate);
        assert_eq!(desktop_named("COSMIC"), Desktop::Cosmic);
        assert_eq!(desktop_named("Budgie:GNOME"), Desktop::Budgie);
        assert_eq!(desktop_named(""), Desktop::Gnome);
        assert_eq!(desktop_named("Hyprland"), Desktop::Gnome);
    }

    #[test]
    fn every_desktop_has_its_own_bars() {
        for desktop in Desktop::ALL {
            assert!(!desktop.panels().is_empty(), "{}", desktop.name());
        }
        let edges = |desktop: Desktop| -> Vec<Edge> {
            desktop.panels().iter().map(|panel| panel.edge).collect()
        };
        assert_eq!(edges(Desktop::Gnome), [Edge::Top]);
        assert_eq!(edges(Desktop::Kde), [Edge::Bottom]);
        assert_eq!(edges(Desktop::Mate), [Edge::Top, Edge::Bottom]);
    }
}
