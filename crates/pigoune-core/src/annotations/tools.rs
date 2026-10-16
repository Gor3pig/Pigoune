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
    Stroke,
    Disc,
    TextBox,
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

pub const SHADOW: OptionSpec = OptionSpec {
    id: "shadow",
    kind: OptionKind::Toggle,
    default: DefaultValue::Toggle(false),
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
        SHADOW,
    ],
};

pub const ELLIPSE: ToolDefinition = ToolDefinition {
    kind: "ellipse",
    geometry: GeometryKind::Box,
    outline: Outline::Ellipse,
    hides_content: false,
    options: &[COLOR, OPACITY, WIDTH, DASH, FILL, FILL_OPACITY, SHADOW],
};

pub const LINE: ToolDefinition = ToolDefinition {
    kind: "line",
    geometry: GeometryKind::TwoPoints,
    outline: Outline::Segment,
    hides_content: false,
    options: &[COLOR, OPACITY, WIDTH, DASH, SHADOW],
};

pub const CURVE: OptionSpec = OptionSpec {
    id: "curve",
    kind: OptionKind::Number {
        min: -100.0,
        max: 100.0,
        step: 1.0,
    },
    default: DefaultValue::Number(0.0),
    shown_if: None,
};

pub const SHAFT_CHOICES: [&str; 2] = ["even", "tapered"];

pub const SHAFT: OptionSpec = OptionSpec {
    id: "shaft",
    kind: OptionKind::Choice(&SHAFT_CHOICES),
    default: DefaultValue::Choice("even"),
    shown_if: None,
};

pub const ARROW: ToolDefinition = ToolDefinition {
    kind: "arrow",
    geometry: GeometryKind::TwoPoints,
    outline: Outline::Segment,
    hides_content: false,
    options: &[
        COLOR, OPACITY, WIDTH, DASH, HEAD, HEAD_SIZE, CURVE, SHAFT, SHADOW,
    ],
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

pub const SPOT_SHAPE_CHOICES: [&str; 2] = ["rectangle", "ellipse"];

pub const SPOT_SHAPE: OptionSpec = OptionSpec {
    id: "spot",
    kind: OptionKind::Choice(&SPOT_SHAPE_CHOICES),
    default: DefaultValue::Choice("rectangle"),
    shown_if: None,
};

pub const DIMMING: OptionSpec = OptionSpec {
    id: "dimming",
    kind: OptionKind::Opacity,
    default: DefaultValue::Number(0.6),
    shown_if: None,
};

pub const SPOTLIGHT: ToolDefinition = ToolDefinition {
    kind: "spotlight",
    geometry: GeometryKind::Box,
    outline: Outline::Region,
    hides_content: false,
    options: &[SPOT_SHAPE, DIMMING],
};

pub const SMOOTH: OptionSpec = OptionSpec {
    id: "smooth",
    kind: OptionKind::Toggle,
    default: DefaultValue::Toggle(true),
    shown_if: None,
};

const YELLOW: Color = Color {
    r: 0xf6,
    g: 0xd3,
    b: 0x2d,
    a: 255,
};

pub const HIGHLIGHT_COLOR: OptionSpec = OptionSpec {
    id: "color",
    kind: OptionKind::Color,
    default: DefaultValue::Color(YELLOW),
    shown_if: None,
};

pub const HIGHLIGHT_OPACITY: OptionSpec = OptionSpec {
    id: "opacity",
    kind: OptionKind::Opacity,
    default: DefaultValue::Number(0.4),
    shown_if: None,
};

pub const HIGHLIGHT_WIDTH: OptionSpec = OptionSpec {
    id: "width",
    kind: OptionKind::Number {
        min: 4.0,
        max: 80.0,
        step: 1.0,
    },
    default: DefaultValue::Number(24.0),
    shown_if: None,
};

pub const PEN: ToolDefinition = ToolDefinition {
    kind: "pen",
    geometry: GeometryKind::Polyline,
    outline: Outline::Stroke,
    hides_content: false,
    options: &[COLOR, OPACITY, WIDTH, SMOOTH, SHADOW],
};

pub const HIGHLIGHTER: ToolDefinition = ToolDefinition {
    kind: "highlighter",
    geometry: GeometryKind::Polyline,
    outline: Outline::Stroke,
    hides_content: false,
    options: &[HIGHLIGHT_COLOR, HIGHLIGHT_OPACITY, HIGHLIGHT_WIDTH, SMOOTH],
};

pub const BADGE_STYLE_CHOICES: [&str; 2] = ["filled", "outlined"];

pub const BADGE_SIZE: OptionSpec = OptionSpec {
    id: "size",
    kind: OptionKind::Number {
        min: 8.0,
        max: 60.0,
        step: 1.0,
    },
    default: DefaultValue::Number(16.0),
    shown_if: None,
};

pub const BADGE_STYLE: OptionSpec = OptionSpec {
    id: "style",
    kind: OptionKind::Choice(&BADGE_STYLE_CHOICES),
    default: DefaultValue::Choice("filled"),
    shown_if: None,
};

pub const SEQUENCE_CHOICES: [&str; 2] = ["numbers", "letters"];

pub const SEQUENCE: OptionSpec = OptionSpec {
    id: "sequence",
    kind: OptionKind::Choice(&SEQUENCE_CHOICES),
    default: DefaultValue::Choice("numbers"),
    shown_if: None,
};

pub const STEP: ToolDefinition = ToolDefinition {
    kind: "step",
    geometry: GeometryKind::Anchor,
    outline: Outline::Disc,
    hides_content: false,
    options: &[COLOR, OPACITY, BADGE_SIZE, BADGE_STYLE, SEQUENCE, SHADOW],
};

pub const FONT_FAMILY_CHOICES: [&str; 3] = ["sans", "serif", "mono"];
pub const TEXT_BACKGROUND_CHOICES: [&str; 4] = ["none", "light", "dark", "bubble"];

pub const CONTENT: OptionSpec = OptionSpec {
    id: "content",
    kind: OptionKind::Text,
    default: DefaultValue::Text(""),
    shown_if: None,
};

pub const FONT_SIZE: OptionSpec = OptionSpec {
    id: "font_size",
    kind: OptionKind::Number {
        min: 8.0,
        max: 200.0,
        step: 1.0,
    },
    default: DefaultValue::Number(32.0),
    shown_if: None,
};

pub const BOLD: OptionSpec = OptionSpec {
    id: "bold",
    kind: OptionKind::Toggle,
    default: DefaultValue::Toggle(false),
    shown_if: None,
};

pub const ITALIC: OptionSpec = OptionSpec {
    id: "italic",
    kind: OptionKind::Toggle,
    default: DefaultValue::Toggle(false),
    shown_if: None,
};

pub const FONT_FAMILY: OptionSpec = OptionSpec {
    id: "family",
    kind: OptionKind::Choice(&FONT_FAMILY_CHOICES),
    default: DefaultValue::Choice("sans"),
    shown_if: None,
};

pub const TEXT_BACKGROUND: OptionSpec = OptionSpec {
    id: "background",
    kind: OptionKind::Choice(&TEXT_BACKGROUND_CHOICES),
    default: DefaultValue::Choice("none"),
    shown_if: None,
};

pub const TEXT_OUTLINE: OptionSpec = OptionSpec {
    id: "outline",
    kind: OptionKind::Toggle,
    default: DefaultValue::Toggle(false),
    shown_if: None,
};

pub const TEXT: ToolDefinition = ToolDefinition {
    kind: "text",
    geometry: GeometryKind::Anchor,
    outline: Outline::TextBox,
    hides_content: false,
    options: &[
        CONTENT,
        COLOR,
        OPACITY,
        FONT_SIZE,
        BOLD,
        ITALIC,
        FONT_FAMILY,
        TEXT_BACKGROUND,
        TEXT_OUTLINE,
        SHADOW,
    ],
};

pub const DEFINITIONS: [&ToolDefinition; 11] = [
    &RECTANGLE,
    &ELLIPSE,
    &LINE,
    &ARROW,
    &PEN,
    &HIGHLIGHTER,
    &STEP,
    &TEXT,
    &BLUR,
    &PIXELATE,
    &SPOTLIGHT,
];

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
