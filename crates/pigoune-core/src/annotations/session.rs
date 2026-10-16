use std::fmt;
use std::rc::Rc;

use serde_json::Value;

use super::geometry::{self, HitMode};
use super::{
    AnnotationDocument, AnnotationObject, Crop, Geometry, ObjectContent, ObjectState, Point, Rect,
    Shape,
};

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
    Reordered {
        from: usize,
        to: usize,
    },
    Flagged {
        id: u32,
        before: ObjectState,
        after: ObjectState,
    },
}

#[derive(Debug, Clone)]
struct Gesture {
    id: u32,
    before: Shape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerMove {
    Front,
    Forward,
    Backward,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Locked,
    Sealed,
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
    nudging: Option<u32>,
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
            nudging: None,
        }
    }

    #[must_use]
    pub fn restored(document: AnnotationDocument) -> Self {
        Self {
            saved_at: None,
            ..Self::new(document)
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
        if id != self.selected {
            self.nudging = None;
        }
        self.selected = id.filter(|id| self.shape_of(*id).is_some());
    }

    #[must_use]
    pub fn hit_test(&self, point: Point, tolerance: f32) -> Option<u32> {
        self.pick(point, tolerance)
    }

    #[must_use]
    pub fn pick(&self, point: Point, tolerance: f32) -> Option<u32> {
        self.objects_at(point, tolerance).into_iter().next()
    }

    #[must_use]
    pub fn objects_at(&self, point: Point, tolerance: f32) -> Vec<u32> {
        self.document
            .objects
            .iter()
            .rev()
            .filter(|object| object.is_pickable() && self.is_visible(object))
            .filter(|object| self.is_hit(object, point, tolerance))
            .map(|object| object.id)
            .collect()
    }

    fn is_hit(&self, object: &AnnotationObject, point: Point, tolerance: f32) -> bool {
        let Some(shape) = object.shape() else {
            return false;
        };
        let mode = if self.selected == Some(object.id) {
            HitMode::Body
        } else {
            HitMode::Outline
        };
        let extent = self
            .measurer
            .as_ref()
            .filter(|_| shape.kind == "text")
            .map(|measurer| (measurer.0)(shape));
        geometry::hit_in_mode(shape, point, tolerance, extent, mode)
    }

    #[must_use]
    pub fn refusal(&self, id: u32) -> Option<Refusal> {
        let state = &self.document.objects.iter().find(|o| o.id == id)?.state;
        if state.sealed {
            Some(Refusal::Sealed)
        } else if state.locked {
            Some(Refusal::Locked)
        } else {
            None
        }
    }

    fn can_edit(&self, id: u32) -> bool {
        self.document
            .objects
            .iter()
            .any(|object| object.id == id && object.is_editable())
    }

    fn is_visible(&self, object: &AnnotationObject) -> bool {
        let (Some(crop), Some(shape)) = (self.document.crop, object.shape()) else {
            return true;
        };
        let Some(bounds) = self.bounds_of(shape) else {
            return true;
        };
        let pixels = |value: u32| f32::from(u16::try_from(value).unwrap_or(u16::MAX));
        let (left, top) = (pixels(crop.x), pixels(crop.y));
        bounds.x <= left + pixels(crop.width)
            && bounds.x + bounds.width >= left
            && bounds.y <= top + pixels(crop.height)
            && bounds.y + bounds.height >= top
    }

    fn bounds_of(&self, shape: &Shape) -> Option<Rect> {
        if let Some(bounds) = geometry::shape_bounds(shape) {
            return Some(bounds);
        }
        let Geometry::Anchor(at) = shape.geometry else {
            return None;
        };
        let (width, height) = self
            .measurer
            .as_ref()
            .map_or_else(|| geometry::estimated_text_extent(shape), |m| (m.0)(shape));
        Some(Rect {
            x: at.x,
            y: at.y,
            width,
            height,
        })
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
        let id = self.document.reserve_id();
        let object = AnnotationObject::new(id, shape);
        let index = self.document.objects.len();
        self.document.objects.push(object.clone());
        self.record(Edit::Added { index, object });
        self.selected = Some(id);
        id
    }

    fn selected_position(&self) -> Option<usize> {
        let id = self.selected?;
        self.document
            .objects
            .iter()
            .position(|object| object.id == id)
    }

    fn target_position(&self, movement: LayerMove) -> Option<usize> {
        let position = self.selected_position()?;
        let last = self.document.objects.len().checked_sub(1)?;
        let target = match movement {
            LayerMove::Front => last,
            LayerMove::Forward => (position + 1).min(last),
            LayerMove::Backward => position.saturating_sub(1),
            LayerMove::Back => 0,
        };
        (target != position).then_some(target)
    }

    #[must_use]
    pub fn can_move_selected(&self, movement: LayerMove) -> bool {
        self.gesture.is_none()
            && self.selected.is_some_and(|id| self.can_edit(id))
            && self.target_position(movement).is_some()
    }

    pub fn move_selected(&mut self, movement: LayerMove) -> bool {
        if !self.can_move_selected(movement) {
            return false;
        }
        let (Some(from), Some(to)) = (self.selected_position(), self.target_position(movement))
        else {
            return false;
        };
        let object = self.document.objects.remove(from);
        self.document.objects.insert(to, object);
        self.record(Edit::Reordered { from, to });
        true
    }

    pub fn nudge_selected(&mut self, dx: f32, dy: f32) -> bool {
        if self.gesture.is_some() {
            return false;
        }
        let Some(selected) = self.selected.filter(|id| self.can_edit(*id)) else {
            return false;
        };
        let Some(before) = self.shape_of(selected).cloned() else {
            return false;
        };
        let after = geometry::translated(&before, dx, dy);
        if after == before {
            return false;
        }
        self.set_shape(selected, after.clone());
        if self.nudging == Some(selected)
            && let Some(Edit::Changed {
                id, after: latest, ..
            }) = self.undo.last_mut()
            && *id == selected
        {
            *latest = after;
            if self.saved_at == Some(self.undo.len()) {
                self.saved_at = None;
            }
            self.redo.clear();
        } else {
            self.record(Edit::Changed {
                id: selected,
                before,
                after,
            });
        }
        self.nudging = Some(selected);
        true
    }

    pub fn duplicate_selected(&mut self, offset: f32) -> Option<u32> {
        if self.gesture.is_some() {
            return None;
        }
        if self
            .selected
            .is_some_and(|id| self.refusal(id) == Some(Refusal::Sealed))
        {
            return None;
        }
        let copy = geometry::translated(self.selected_shape()?, offset, offset);
        Some(self.add(copy))
    }

    pub fn remove_selected(&mut self) -> bool {
        match self.selected {
            Some(id) if self.can_edit(id) => self.remove_object(id),
            _ => false,
        }
    }

    pub fn remove_object(&mut self, id: u32) -> bool {
        if self.gesture.is_some() || self.refusal(id) == Some(Refusal::Sealed) {
            return false;
        }
        let Some(index) = self.document.objects.iter().position(|o| o.id == id) else {
            return false;
        };
        let object = self.document.objects.remove(index);
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.record(Edit::Removed { index, object });
        true
    }

    pub fn set_hidden(&mut self, id: u32, hidden: bool) -> bool {
        self.change_state(id, |state| state.hidden = hidden)
    }

    pub fn set_locked(&mut self, id: u32, locked: bool) -> bool {
        self.change_state(id, |state| state.locked = locked)
    }

    pub fn rename(&mut self, id: u32, name: Option<String>) -> bool {
        let name = name
            .map(|name| name.trim().to_owned())
            .filter(|n| !n.is_empty());
        self.change_state(id, |state| state.name = name)
    }

    fn change_state(&mut self, id: u32, change: impl FnOnce(&mut ObjectState)) -> bool {
        if self.gesture.is_some() || self.refusal(id) == Some(Refusal::Sealed) {
            return false;
        }
        let Some(object) = self.document.objects.iter_mut().find(|o| o.id == id) else {
            return false;
        };
        let before = object.state.clone();
        change(&mut object.state);
        let after = object.state.clone();
        if before == after {
            return false;
        }
        self.record(Edit::Flagged { id, before, after });
        true
    }

    pub fn begin_change(&mut self, id: u32) -> bool {
        if self.gesture.is_some() || !self.can_edit(id) {
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
        let Some(selected) = self.selected.filter(|id| self.can_edit(*id)) else {
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
        self.nudging = None;
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
        self.nudging = None;
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.apply(&edit, false);
        self.undo.push(edit);
        true
    }

    fn record(&mut self, edit: Edit) {
        self.nudging = None;
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
            (Edit::Reordered { from, to }, backwards) => {
                let (source, target) = if backwards {
                    (*to, *from)
                } else {
                    (*from, *to)
                };
                if source < self.document.objects.len() {
                    let object = self.document.objects.remove(source);
                    let id = object.id;
                    self.document
                        .objects
                        .insert(target.min(self.document.objects.len()), object);
                    self.selected = Some(id);
                }
            }
            (Edit::Cropped { before, after }, _) => {
                self.document.crop = if backwards { *before } else { *after };
            }
            (Edit::Flagged { id, before, after }, backwards) => {
                if let Some(object) = self.document.objects.iter_mut().find(|o| o.id == *id) {
                    object.state = if backwards { before } else { after }.clone();
                }
                self.selected = Some(*id);
            }
            (Edit::Changed { id, before, after }, _) => {
                let shape = if backwards { before } else { after };
                self.set_shape(*id, shape.clone());
                self.selected = Some(*id);
            }
        }
    }
}
