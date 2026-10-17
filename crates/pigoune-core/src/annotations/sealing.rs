use serde_json::Value;

use super::{AnnotationDocument, AnnotationObject, Rect};

const HASH_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealRecord {
    pub generation: u32,
    pub base: String,
}

impl SealRecord {
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.generation >= 1 && is_hash(&self.base)
    }

    pub(super) fn from_value(value: &Value) -> Option<Self> {
        let record = Self {
            generation: u32::try_from(value.get("generation")?.as_u64()?).ok()?,
            base: value.get("base")?.as_str()?.to_owned(),
        };
        record.is_well_formed().then_some(record)
    }

    pub(super) fn to_value(&self) -> Value {
        serde_json::json!({ "generation": self.generation, "base": self.base })
    }
}

fn is_hash(text: &str) -> bool {
    text.len() == HASH_LENGTH
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapChoice {
    KeepVisible,
    HideOverlapped,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SealPlan {
    pub masks: Vec<u32>,
    pub overlapped: Vec<u32>,
    pub texts: Vec<u32>,
}

impl SealPlan {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.masks.is_empty()
    }
}

fn is_mask(object: &AnnotationObject) -> bool {
    !object.state.sealed
        && !object.state.hidden
        && object
            .shape()
            .is_some_and(super::shape::Shape::hides_content)
}

fn touches(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

pub fn plan(
    document: &AnnotationDocument,
    bounds_of: impl Fn(&AnnotationObject) -> Option<Rect>,
) -> SealPlan {
    let masks: Vec<u32> = document
        .objects
        .iter()
        .filter(|object| is_mask(object))
        .map(|object| object.id)
        .collect();
    let mut overlapped = Vec::new();
    for (index, object) in document.objects.iter().enumerate() {
        let affected = !object.state.sealed
            && !object.state.hidden
            && !masks.contains(&object.id)
            && object.shape().is_some();
        if !affected {
            continue;
        }
        let Some(bounds) = bounds_of(object) else {
            continue;
        };
        let covered = document.objects[index + 1..].iter().any(|above| {
            masks.contains(&above.id)
                && bounds_of(above).is_some_and(|mask_bounds| touches(bounds, mask_bounds))
        });
        if covered {
            overlapped.push(object.id);
        }
    }
    let texts = overlapped
        .iter()
        .copied()
        .filter(|id| {
            document
                .objects
                .iter()
                .any(|o| o.id == *id && o.shape().is_some_and(|shape| shape.kind == "text"))
        })
        .collect();
    SealPlan {
        masks,
        overlapped,
        texts,
    }
}

#[must_use]
pub fn apply(
    document: &AnnotationDocument,
    plan: &SealPlan,
    choice: OverlapChoice,
    record: SealRecord,
) -> AnnotationDocument {
    let mut sealed = document.clone();
    for object in &mut sealed.objects {
        if plan.masks.contains(&object.id) {
            object.state.sealed = true;
        } else if choice == OverlapChoice::HideOverlapped && plan.overlapped.contains(&object.id) {
            object.state.hidden = true;
        }
    }
    let (mut front, rest): (Vec<_>, Vec<_>) = sealed
        .objects
        .drain(..)
        .partition(|object| object.state.sealed);
    front.extend(rest);
    sealed.objects = front;
    sealed.seal = Some(record);
    sealed
}
