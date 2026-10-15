mod crop;
mod geometry;
mod interaction;
mod options;
mod session;
mod shape;
mod tools;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Map;

pub use crop::{CropEditor, CropRatio, SMALLEST_SIDE};
pub use geometry::{
    ArrowHead, Handle, arrow_head, dash_pattern, handles, head_size_factor, hit, next_stroke_point,
    rect_between, resized, shape_bounds, shape_from_drag, shape_from_points, simplified,
    translated,
};
pub use interaction::{CursorKind, Interaction, Tool};
pub use options::{
    DefaultValue, OptionKind, OptionSpec, Options, color_to_value, number_to_value, value_to_color,
};
pub use serde_json::Value;
pub use session::EditSession;
pub use shape::{Geometry, Shape};
pub use tools::{
    ARROW, BADGE_SIZE, BADGE_STYLE, BLUR, CELL, COLOR, CORNER_RADIUS, DASH, DASH_CHOICES, ELLIPSE,
    FILL, FILL_OPACITY, GeometryKind, HEAD, HEAD_CHOICES, HEAD_SIZE, HEAD_SIZE_CHOICES,
    HIGHLIGHT_COLOR, HIGHLIGHT_OPACITY, HIGHLIGHT_WIDTH, HIGHLIGHTER, LINE, OPACITY, Outline, PEN,
    PIXELATE, RADIUS, RECTANGLE, SMOOTH, STEP, ToolDefinition, WIDTH, definition, definitions,
};

pub const DOCUMENT_VERSION: u32 = 1;
pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("the annotations are damaged: {0}")]
    Corrupt(String),
    #[error(
        "the annotations use document version {found}, but this version of Pigoune supports up to {supported}"
    )]
    TooNew { found: u32, supported: u32 },
    #[error("the annotations are too large")]
    TooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectContent {
    Known {
        shape: Shape,
        extra: Map<String, Value>,
    },
    Unknown(Map<String, Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationObject {
    pub id: u32,
    pub content: ObjectContent,
}

impl AnnotationObject {
    #[must_use]
    pub fn new(id: u32, shape: Shape) -> Self {
        Self {
            id,
            content: ObjectContent::Known {
                shape,
                extra: Map::new(),
            },
        }
    }

    #[must_use]
    pub fn shape(&self) -> Option<&Shape> {
        match &self.content {
            ObjectContent::Known { shape, .. } => Some(shape),
            ObjectContent::Unknown(_) => None,
        }
    }

    fn to_value(&self) -> Value {
        match &self.content {
            ObjectContent::Unknown(fields) => Value::Object(fields.clone()),
            ObjectContent::Known { shape, extra } => {
                let mut fields = shape.to_map();
                for (name, value) in extra {
                    fields.entry(name.clone()).or_insert_with(|| value.clone());
                }
                fields.insert("id".to_owned(), Value::from(self.id));
                Value::Object(fields)
            }
        }
    }

    fn from_value(value: Value) -> Result<Self, String> {
        let Value::Object(fields) = value else {
            return Err("an annotation is not an object".to_owned());
        };
        let id = fields
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok())
            .ok_or("an annotation has no valid id")?;
        let kind = fields
            .get("kind")
            .and_then(Value::as_str)
            .ok_or("an annotation has no kind")?;
        let Some(definition) = definition(kind) else {
            return Ok(Self {
                id,
                content: ObjectContent::Unknown(fields),
            });
        };
        let shape = Shape::from_map(definition, &fields)
            .map_err(|error| format!("annotation {id}: {error}"))?;
        let used = shape.to_map();
        let extra = fields
            .into_iter()
            .filter(|(name, _)| name != "id" && !used.contains_key(name))
            .collect();
        Ok(Self {
            id,
            content: ObjectContent::Known { shape, extra },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnnotationDocument {
    pub crop: Option<Crop>,
    pub objects: Vec<AnnotationObject>,
    extra: Map<String, Value>,
}

impl AnnotationDocument {
    #[must_use]
    pub fn new(crop: Option<Crop>, objects: Vec<AnnotationObject>) -> Self {
        Self {
            crop,
            objects,
            extra: Map::new(),
        }
    }

    #[must_use]
    pub fn next_object_id(&self) -> u32 {
        self.objects
            .iter()
            .map(|object| object.id)
            .max()
            .map_or(1, |highest| highest.saturating_add(1))
    }

    #[must_use]
    pub fn ordinal(&self, id: u32) -> Option<u32> {
        let kind = &self
            .objects
            .iter()
            .find(|object| object.id == id)?
            .shape()?
            .kind;
        let mut ordinal = 0;
        for object in &self.objects {
            if object.shape().is_some_and(|shape| &shape.kind == kind) {
                ordinal += 1;
            }
            if object.id == id {
                return Some(ordinal);
            }
        }
        None
    }

    #[must_use]
    pub fn next_ordinal(&self, kind: &str) -> u32 {
        let count = self
            .objects
            .iter()
            .filter(|object| object.shape().is_some_and(|shape| shape.kind == kind))
            .count();
        u32::try_from(count).map_or(u32::MAX, |count| count.saturating_add(1))
    }

    #[must_use]
    pub fn hides_content(&self) -> bool {
        self.objects
            .iter()
            .any(|object| object.shape().is_some_and(Shape::hides_content))
    }

    pub fn from_json(text: &str) -> Result<Self, DocumentError> {
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err(DocumentError::TooLarge);
        }
        let value: Value = serde_json::from_str(text)
            .map_err(|error| DocumentError::Corrupt(error.to_string()))?;
        let Value::Object(mut fields) = value else {
            return Err(DocumentError::Corrupt(
                "the document is not an object".to_owned(),
            ));
        };
        let found = fields
            .remove("version")
            .and_then(|version| version.as_u64())
            .and_then(|version| u32::try_from(version).ok())
            .ok_or_else(|| DocumentError::Corrupt("the document has no version".to_owned()))?;
        if found > DOCUMENT_VERSION {
            return Err(DocumentError::TooNew {
                found,
                supported: DOCUMENT_VERSION,
            });
        }
        let crop = match fields.remove("crop") {
            None | Some(Value::Null) => None,
            Some(crop) => Some(
                serde_json::from_value(crop)
                    .map_err(|error| DocumentError::Corrupt(format!("crop: {error}")))?,
            ),
        };
        let Some(Value::Array(objects)) = fields.remove("objects") else {
            return Err(DocumentError::Corrupt(
                "the document has no objects".to_owned(),
            ));
        };
        let objects: Vec<AnnotationObject> = objects
            .into_iter()
            .map(AnnotationObject::from_value)
            .collect::<Result<_, _>>()
            .map_err(DocumentError::Corrupt)?;
        let mut seen = std::collections::HashSet::new();
        if let Some(repeated) = objects.iter().find(|object| !seen.insert(object.id)) {
            return Err(DocumentError::Corrupt(format!(
                "annotation {} appears twice",
                repeated.id
            )));
        }
        Ok(Self {
            crop,
            objects,
            extra: fields,
        })
    }

    pub fn to_json(&self) -> Result<String, DocumentError> {
        if let Some(object) = self.objects.iter().find(|object| {
            object
                .shape()
                .is_some_and(|shape| !shape.numbers_are_finite())
        }) {
            return Err(DocumentError::Corrupt(format!(
                "annotation {} has a number that is not finite",
                object.id
            )));
        }
        let mut fields = self.extra.clone();
        fields.insert("version".to_owned(), Value::from(DOCUMENT_VERSION));
        if let Some(crop) = &self.crop {
            fields.insert(
                "crop".to_owned(),
                serde_json::to_value(crop).unwrap_or(Value::Null),
            );
        }
        let objects = self
            .objects
            .iter()
            .map(AnnotationObject::to_value)
            .collect();
        fields.insert("objects".to_owned(), Value::Array(objects));
        Ok(Value::Object(fields).to_string())
    }
}

impl Serialize for AnnotationObject {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AnnotationObject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_value(Value::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}
