use std::fmt;
use std::rc::Rc;

use serde_json::Value;

use super::geometry;
use super::{AnnotationDocument, AnnotationObject, Crop, ObjectContent, Point, Shape};

const HISTORY_LIMIT: usize = 500;

#[derive(Debug, Clone)]
enum Edit {
    Added {
        index: usize,
        object: AnnotationObject,
    },
    Removed {
        index: usize,
        object: AnnotationObject,
    },
    Changed {
        id: u32,
        before: Shape,
        after: Shape,
    },
    Cropped {
        before: Option<Crop>,
        after: Option<Crop>,
    },
}

#[derive(Debug, Clone)]
struct Gesture {
    id: u32,
    before: Shape,
}

type TextMeasure = dyn Fn(&Shape) -> (f32, f32);

#[derive(Clone)]
struct Measurer(Rc<TextMeasure>);

impl fmt::Debug for Measurer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Measurer")
    }
}

#[derive(Debug, Clone)]
pub struct EditSession {
    document: AnnotationDocument,
    selected: Option<u32>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    gesture: Option<Gesture>,
    saved_at: Option<usize>,
    measurer: Option<Measurer>,
}

impl EditSession {
    #[must_use]
    pub fn new(document: AnnotationDocument) -> Self {
        Self {
            document,
            selected: None,
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            saved_at: Some(0),
            measurer: None,
        }
    }

    pub fn set_text_measurer(&mut self, measurer: impl Fn(&Shape) -> (f32, f32) + 'static) {
        self.measurer = Some(Measurer(Rc::new(measurer)));
    }

    #[must_use]
    pub fn document(&self) -> &AnnotationDocument {
        &self.document
    }

    #[must_use]
    pub fn selected(&self) -> Option<u32> {
        self.selected
    }

    #[must_use]
    pub fn selected_shape(&self) -> Option<&Shape> {
        self.shape_of(self.selected?)
    }

    #[must_use]
    pub fn shape_of(&self, id: u32) -> Option<&Shape> {
        self.document
            .objects
            .iter()
            .find(|object| object.id == id)
            .and_then(AnnotationObject::shape)
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.document.objects.len()
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.gesture.is_none() && !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.gesture.is_none() && !self.redo.is_empty()
    }

    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.gesture.is_some() || self.saved_at != Some(self.undo.len())
    }

    pub fn mark_saved(&mut self) {
        self.saved_at = Some(self.undo.len());
    }

    pub fn select(&mut self, id: Option<u32>) {
        self.selected = id.filter(|id| self.shape_of(*id).is_some());
    }

    #[must_use]
    pub fn hit_test(&self, point: Point, tolerance: f32) -> Option<u32> {
        self.document
            .objects
            .iter()
            .rev()
            .find(|object| {
                object.shape().is_some_and(|shape| {
                    let extent = self
                        .measurer
                        .as_ref()
                        .filter(|_| shape.kind == "text")
                        .map(|measurer| (measurer.0)(shape));
                    geometry::hit_with_extent(shape, point, tolerance, extent)
                })
            })
            .map(|object| object.id)
    }

    #[must_use]
    pub fn crop(&self) -> Option<Crop> {
        self.document.crop
    }

    pub fn set_crop(&mut self, crop: Option<Crop>) -> bool {
        let before = self.document.crop;
        if before == crop {
            return false;
        }
        self.document.crop = crop;
        self.record(Edit::Cropped {
            before,
            after: crop,
        });
        true
    }

    pub fn add(&mut self, shape: Shape) -> u32 {
        let id = self.document.next_object_id();
        let object = AnnotationObject::new(id, shape);
        let index = self.document.objects.len();
        self.document.objects.push(object.clone());
        self.record(Edit::Added { index, object });
        self.selected = Some(id);
        id
    }

    pub fn remove_selected(&mut self) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let Some(index) = self.document.objects.iter().position(|o| o.id == id) else {
            return false;
        };
        let object = self.document.objects.remove(index);
        self.selected = None;
        self.record(Edit::Removed { index, object });
        true
    }

    pub fn begin_change(&mut self, id: u32) -> bool {
        if self.gesture.is_some() {
            return false;
        }
        let Some(before) = self.shape_of(id).cloned() else {
            return false;
        };
        self.gesture = Some(Gesture { id, before });
        true
    }

    pub fn preview(&mut self, shape: Shape) {
        let Some(gesture) = &self.gesture else {
            return;
        };
        let id = gesture.id;
        self.set_shape(id, shape);
    }

    pub fn end_change(&mut self) -> bool {
        let Some(gesture) = self.gesture.take() else {
            return false;
        };
        let Some(after) = self.shape_of(gesture.id).cloned() else {
            return false;
        };
        if after == gesture.before {
            return false;
        }
        self.record(Edit::Changed {
            id: gesture.id,
            before: gesture.before,
            after,
        });
        true
    }

    pub fn cancel_change(&mut self) {
        if let Some(gesture) = self.gesture.take() {
            self.set_shape(gesture.id, gesture.before);
        }
    }

    pub fn set_option_on_selected(&mut self, id: &str, value: Value) -> bool {
        let Some(selected) = self.selected else {
            return false;
        };
        let Some(before) = self.shape_of(selected).cloned() else {
            return false;
        };
        let after = before.with_option(id, value);
        if after == before {
            return false;
        }
        self.set_shape(selected, after.clone());
        self.record(Edit::Changed {
            id: selected,
            before,
            after,
        });
        true
    }

    pub fn preview_option(&mut self, id: &str, value: Value) {
        let Some(gesture) = &self.gesture else {
            return;
        };
        let target = gesture.id;
        let Some(current) = self.shape_of(target) else {
            return;
        };
        let changed = current.with_option(id, value);
        self.set_shape(target, changed);
    }

    pub fn undo(&mut self) -> bool {
        if self.gesture.is_some() {
            return false;
        }
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.apply(&edit, true);
        self.redo.push(edit);
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.gesture.is_some() {
            return false;
        }
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.apply(&edit, false);
        self.undo.push(edit);
        true
    }

    fn record(&mut self, edit: Edit) {
        if self.saved_at.is_some_and(|saved| saved > self.undo.len()) {
            self.saved_at = None;
        }
        self.redo.clear();
        self.undo.push(edit);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
            self.saved_at = self.saved_at.and_then(|saved| saved.checked_sub(1));
        }
    }

    fn set_shape(&mut self, id: u32, shape: Shape) {
        if let Some(object) = self.document.objects.iter_mut().find(|o| o.id == id)
            && let ObjectContent::Known { shape: current, .. } = &mut object.content
        {
            *current = shape;
        }
    }

    fn apply(&mut self, edit: &Edit, backwards: bool) {
        match (edit, backwards) {
            (Edit::Added { index, object }, false) | (Edit::Removed { index, object }, true) => {
                let index = (*index).min(self.document.objects.len());
                self.document.objects.insert(index, object.clone());
                self.selected = Some(object.id);
            }
            (Edit::Added { object, .. }, true) | (Edit::Removed { object, .. }, false) => {
                self.document.objects.retain(|other| other.id != object.id);
                if self.selected == Some(object.id) {
                    self.selected = None;
                }
            }
            (Edit::Cropped { before, after }, _) => {
                self.document.crop = if backwards { *before } else { *after };
            }
            (Edit::Changed { id, before, after }, _) => {
                let shape = if backwards { before } else { after };
                self.set_shape(*id, shape.clone());
                self.selected = Some(*id);
            }
        }
    }
}
