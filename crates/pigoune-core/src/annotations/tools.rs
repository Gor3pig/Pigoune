use super::Color;
use super::options::{DefaultValue, OptionKind, OptionSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryKind {
    TwoPoints,
    Box,
    Polyline,
    Anchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outline {
    Rectangle,
    Ellipse,
    Segment,
    Region,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolDefinition {
    pub kind: &'static str,
    pub geometry: GeometryKind,
    pub outline: Outline,
    pub hides_content: bool,
    pub options: &'static [OptionSpec],
}

impl ToolDefinition {
    #[must_use]
    pub fn spec(&self, id: &str) -> Option<&'static OptionSpec> {
        self.options.iter().find(|spec| spec.id == id)
    }
}

const RED: Color = Color {
    r: 0xe0,
    g: 0x1b,
    b: 0x24,
    a: 255,
};

pub const COLOR: OptionSpec = OptionSpec {
    id: "color",
    kind: OptionKind::Color,
    default: DefaultValue::Color(RED),
    shown_if: None,
};

pub const OPACITY: OptionSpec = OptionSpec {
    id: "opacity",
    kind: OptionKind::Opacity,
    default: DefaultValue::Number(1.0),
    shown_if: None,
};

pub const WIDTH: OptionSpec = OptionSpec {
    id: "width",
    kind: OptionKind::Number {
        min: 1.0,
        max: 60.0,
        step: 1.0,
    },
    default: DefaultValue::Number(6.0),
    shown_if: None,
};

pub const DASH_CHOICES: [&str; 3] = ["solid", "dashed", "dotted"];

pub const DASH: OptionSpec = OptionSpec {
    id: "dash",
    kind: OptionKind::Choice(&DASH_CHOICES),
    default: DefaultValue::Choice("solid"),
    shown_if: None,
};

pub const HEAD_CHOICES: [&str; 4] = ["triangle", "open", "double", "none"];

pub const HEAD: OptionSpec = OptionSpec {
    id: "head",
    kind: OptionKind::Choice(&HEAD_CHOICES),
    default: DefaultValue::Choice("triangle"),
    shown_if: None,
};

pub const HEAD_SIZE_CHOICES: [&str; 3] = ["small", "medium", "large"];

pub const HEAD_SIZE: OptionSpec = OptionSpec {
    id: "head_size",
    kind: OptionKind::Choice(&HEAD_SIZE_CHOICES),
    default: DefaultValue::Choice("medium"),
    shown_if: None,
};

pub const FILL: OptionSpec = OptionSpec {
    id: "fill",
    kind: OptionKind::Toggle,
    default: DefaultValue::Toggle(false),
    shown_if: None,
};

pub const FILL_OPACITY: OptionSpec = OptionSpec {
    id: "fill_opacity",
    kind: OptionKind::Opacity,
    default: DefaultValue::Number(0.25),
    shown_if: Some("fill"),
};

pub const CORNER_RADIUS: OptionSpec = OptionSpec {
    id: "corner_radius",
    kind: OptionKind::Number {
        min: 0.0,
        max: 200.0,
        step: 1.0,
    },
    default: DefaultValue::Number(0.0),
    shown_if: None,
};

pub const RECTANGLE: ToolDefinition = ToolDefinition {
    kind: "rectangle",
    geometry: GeometryKind::Box,
    outline: Outline::Rectangle,
    hides_content: false,
    options: &[
        COLOR,
        OPACITY,
        WIDTH,
        DASH,
        CORNER_RADIUS,
        FILL,
        FILL_OPACITY,
    ],
};

pub const ELLIPSE: ToolDefinition = ToolDefinition {
    kind: "ellipse",
    geometry: GeometryKind::Box,
    outline: Outline::Ellipse,
    hides_content: false,
    options: &[COLOR, OPACITY, WIDTH, DASH, FILL, FILL_OPACITY],
};

pub const LINE: ToolDefinition = ToolDefinition {
    kind: "line",
    geometry: GeometryKind::TwoPoints,
    outline: Outline::Segment,
    hides_content: false,
    options: &[COLOR, OPACITY, WIDTH, DASH],
};

pub const ARROW: ToolDefinition = ToolDefinition {
    kind: "arrow",
    geometry: GeometryKind::TwoPoints,
    outline: Outline::Segment,
    hides_content: false,
    options: &[COLOR, OPACITY, WIDTH, DASH, HEAD, HEAD_SIZE],
};

pub const RADIUS: OptionSpec = OptionSpec {
    id: "radius",
    kind: OptionKind::Number {
        min: 1.0,
        max: 60.0,
        step: 1.0,
    },
    default: DefaultValue::Number(12.0),
    shown_if: None,
};

pub const CELL: OptionSpec = OptionSpec {
    id: "cell",
    kind: OptionKind::Number {
        min: 2.0,
        max: 80.0,
        step: 1.0,
    },
    default: DefaultValue::Number(12.0),
    shown_if: None,
};

pub const BLUR: ToolDefinition = ToolDefinition {
    kind: "blur",
    geometry: GeometryKind::Box,
    outline: Outline::Region,
    hides_content: true,
    options: &[RADIUS],
};

pub const PIXELATE: ToolDefinition = ToolDefinition {
    kind: "pixelate",
    geometry: GeometryKind::Box,
    outline: Outline::Region,
    hides_content: true,
    options: &[CELL],
};

pub const DEFINITIONS: [&ToolDefinition; 6] =
    [&RECTANGLE, &ELLIPSE, &LINE, &ARROW, &BLUR, &PIXELATE];

#[must_use]
pub fn definition(kind: &str) -> Option<&'static ToolDefinition> {
    DEFINITIONS
        .iter()
        .copied()
        .find(|definition| definition.kind == kind)
}

#[must_use]
pub fn definitions() -> &'static [&'static ToolDefinition] {
    &DEFINITIONS
}
