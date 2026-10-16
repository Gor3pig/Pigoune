use serde_json::{Map, Value};

use super::options::Options;
use super::tools::{GeometryKind, ToolDefinition, definition};
use super::{Color, Point, Rect};

#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    TwoPoints { from: Point, to: Point },
    Box(Rect),
    Polyline(Vec<Point>),
    Anchor(Point),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub kind: String,
    pub geometry: Geometry,
    pub options: Options,
}

impl Shape {
    #[must_use]
    pub fn new(kind: &str, geometry: Geometry, options: Options) -> Self {
        Self {
            kind: kind.to_owned(),
            geometry,
            options,
        }
    }

    #[must_use]
    pub fn definition(&self) -> Option<&'static ToolDefinition> {
        definition(&self.kind)
    }

    #[must_use]
    pub fn hides_content(&self) -> bool {
        self.definition()
            .is_some_and(|definition| definition.hides_content)
    }

    #[must_use]
    pub fn color(&self, id: &str) -> Color {
        self.definition()
            .and_then(|definition| definition.spec(id))
            .map_or(
                Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                |spec| self.options.color(spec),
            )
    }

    #[must_use]
    pub fn number(&self, id: &str) -> f32 {
        self.definition()
            .and_then(|definition| definition.spec(id))
            .map_or(0.0, |spec| self.options.number(spec))
    }

    #[must_use]
    pub fn choice(&self, id: &str) -> &str {
        self.definition()
            .and_then(|definition| definition.spec(id))
            .map_or("", |spec| self.options.choice(spec))
    }

    #[must_use]
    pub fn text(&self, id: &str) -> &str {
        self.definition()
            .and_then(|definition| definition.spec(id))
            .map_or("", |spec| self.options.text(spec))
    }

    #[must_use]
    pub fn toggle(&self, id: &str) -> bool {
        self.definition()
            .and_then(|definition| definition.spec(id))
            .is_some_and(|spec| self.options.toggle(spec))
    }

    #[must_use]
    pub fn stroke_color(&self) -> Color {
        faded(self.color("color"), self.number("opacity"))
    }

    #[must_use]
    pub fn fill_color(&self) -> Option<Color> {
        self.toggle("fill")
            .then(|| faded(self.color("color"), self.number("fill_opacity")))
    }

    #[must_use]
    pub fn with_option(&self, id: &str, value: Value) -> Self {
        let mut shape = self.clone();
        shape.options.set(id, value);
        shape
    }

    #[must_use]
    pub fn with_geometry(&self, geometry: Geometry) -> Self {
        Self {
            geometry,
            ..self.clone()
        }
    }

    pub(super) fn to_map(&self) -> Map<String, Value> {
        let mut fields = Map::new();
        fields.insert("kind".to_owned(), Value::from(self.kind.clone()));
        match &self.geometry {
            Geometry::TwoPoints { from, to } => {
                fields.insert("from".to_owned(), point_to_value(*from));
                fields.insert("to".to_owned(), point_to_value(*to));
            }
            Geometry::Box(bounds) => {
                fields.insert("bounds".to_owned(), rect_to_value(*bounds));
            }
            Geometry::Polyline(points) => {
                let points = points.iter().copied().map(point_to_value).collect();
                fields.insert("points".to_owned(), Value::Array(points));
            }
            Geometry::Anchor(at) => {
                fields.insert("at".to_owned(), point_to_value(*at));
            }
        }
        fields.insert(
            "options".to_owned(),
            Value::Object(self.options.as_map().clone()),
        );
        fields
    }

    pub(super) fn from_map(
        definition: &'static ToolDefinition,
        fields: &Map<String, Value>,
    ) -> Result<Self, String> {
        let get = |key: &str| fields.get(key).ok_or_else(|| format!("{key} is missing"));
        let geometry = match definition.geometry {
            GeometryKind::TwoPoints => Geometry::TwoPoints {
                from: value_to_point(get("from")?)?,
                to: value_to_point(get("to")?)?,
            },
            GeometryKind::Box => Geometry::Box(value_to_rect(get("bounds")?)?),
            GeometryKind::Polyline => {
                let list = get("points")?
                    .as_array()
                    .ok_or_else(|| "points is not a list".to_owned())?;
                Geometry::Polyline(list.iter().map(value_to_point).collect::<Result<_, _>>()?)
            }
            GeometryKind::Anchor => Geometry::Anchor(value_to_point(get("at")?)?),
        };
        let options = match fields.get("options") {
            None | Some(Value::Null) => Options::new(),
            Some(Value::Object(map)) => Options::from_map(map.clone()),
            Some(_) => return Err("options is not an object".to_owned()),
        };
        Ok(Self {
            kind: definition.kind.to_owned(),
            geometry,
            options,
        })
    }

    pub(super) fn numbers_are_finite(&self) -> bool {
        let point = |point: &Point| point.x.is_finite() && point.y.is_finite();
        match &self.geometry {
            Geometry::TwoPoints { from, to } => point(from) && point(to),
            Geometry::Box(rect) => [rect.x, rect.y, rect.width, rect.height]
                .iter()
                .all(|number| number.is_finite()),
            Geometry::Polyline(points) => points.iter().all(point),
            Geometry::Anchor(at) => point(at),
        }
    }
}

fn faded(color: Color, opacity: f32) -> Color {
    Color {
        a: opacity_to_alpha(f32::from(color.a) / 255.0 * opacity),
        ..color
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the opacity is clamped to 0..=1 before the conversion"
)]
fn opacity_to_alpha(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn point_to_value(point: Point) -> Value {
    serde_json::to_value(point).unwrap_or(Value::Null)
}

fn rect_to_value(rect: Rect) -> Value {
    serde_json::to_value(rect).unwrap_or(Value::Null)
}

fn value_to_point(value: &Value) -> Result<Point, String> {
    serde_json::from_value(value.clone()).map_err(|error| format!("a point: {error}"))
}

fn value_to_rect(value: &Value) -> Result<Rect, String> {
    serde_json::from_value(value.clone()).map_err(|error| format!("a rectangle: {error}"))
}
