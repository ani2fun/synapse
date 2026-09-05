//! The MEMORY lens: a raw trace step → frames on the left, objects on the right, arrows between.
//!
//! The other lens (`adapt` → `VizCases`) answers "what shape is this data structure": it picks a
//! root, projects one structure, and throws the rest of the heap away. This one answers the
//! different question — "what does the program's memory look like right now" — so it keeps
//! everything and interprets nothing. No root hint, no structure vocabulary, no family: every
//! frame, every reachable object, and an arrow for every reference.
//!
//! It is therefore the lens that works when the other cannot. A program with no single rooted
//! structure — two lists and a counter, a class instance pointing at a dict — has no `viz=` token
//! that describes it, and would draw an empty canvas. This draws it.
//!
//! **Geometry is computed HERE, in pure Rust, not measured in the DOM.** The renderer emits one
//! SVG from these boxes, so the arrows land exactly where the rows are, the layout is identical
//! across browsers, and every rule below is testable natively. The cost is that text is MEASURED
//! BY CHARACTER COUNT — sound only because the figure is drawn in the mono face, which the
//! stylesheet pins.

use crate::engine::trace::{ArrKind, HeapObject, HeapScalar, HeapStep, HeapValue};

// ── metrics (px, at the 12px mono the sheet pins) ──
// The renderer reads these too: a box drawn from one set of numbers and an arrow aimed with
// another is a bug that only shows up as a line landing slightly off a row.
const CHAR_W: f64 = 7.3;
pub const ROW_H: f64 = 22.0;
pub const HEAD_H: f64 = 26.0;
pub const PAD_X: f64 = 10.0;
const GAP_Y: f64 = 18.0;
/// Between the frames column and the objects column — the arrows' whole run.
const COL_GAP: f64 = 96.0;
const MARGIN: f64 = 12.0;
/// One array cell, and the floor a short value still occupies.
const CELL_MIN_W: f64 = 30.0;
const NAME_COL_MIN: f64 = 52.0;
/// Past this a value is elided: a 400-character repr is not a diagram.
const VALUE_MAX_CHARS: usize = 36;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    /// The top edge of row `i`, below the header. THE place row geometry is computed — both the
    /// renderer's boxes and this module's arrow endpoints come through here, so they cannot
    /// disagree about where a row is.
    #[must_use]
    pub fn row_y(self, i: usize) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let n = i as f64;
        self.y + HEAD_H + n * ROW_H
    }

    /// Where an arrow leaves row `i` — its vertical middle.
    #[must_use]
    pub fn row_mid_y(self, i: usize) -> f64 {
        self.row_y(i) + ROW_H / 2.0
    }

    /// The left edge of array cell `i` in a strip of `cell_w`-wide cells.
    #[must_use]
    pub fn cell_x(self, i: usize, cell_w: f64) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let n = i as f64;
        self.x + PAD_X / 2.0 + n * cell_w
    }
}

/// What a box is, which is all the renderer needs to know to style it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjKind {
    List,
    Tuple,
    Dict,
    Instance,
}

/// One `name → value` line, in a frame or in an object. `target` set means the value is a
/// REFERENCE and an arrow leaves this row; `value` is then the empty string, because a row cannot
/// both name a thing and point at it.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub name: String,
    pub value: String,
    pub target: Option<String>,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub title: String,
    pub is_active: bool,
    pub slots: Vec<Slot>,
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub id: String,
    pub title: String,
    pub kind: ObjKind,
    pub rows: Vec<Slot>,
    pub rect: Rect,
    /// Set for a list/tuple, whose rows draw as a horizontal indexed strip rather than a column.
    pub cell_w: Option<f64>,
    pub is_new: bool,
}

/// A reference, resolved to the two points it joins.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrow {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MemoryStep {
    pub line: i32,
    pub frames: Vec<Frame>,
    pub objects: Vec<Object>,
    pub arrows: Vec<Arrow>,
    pub width: f64,
    pub height: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PROJECTION
// ─────────────────────────────────────────────────────────────────────────────

/// One step → one laid-out memory diagram. `previous` is the step before it, used only to mark
/// what CHANGED — pass `None` for the first step, where nothing has changed yet because there was
/// nothing to change from.
#[must_use]
pub fn project(step: &HeapStep, previous: Option<&HeapStep>) -> MemoryStep {
    let frames = frames_of(step, previous);
    let order = reachable_order(step);
    let objects = objects_of(step, previous, &order);
    place(step.line, frames, objects)
}

/// Every step, projected, with each one's predecessor supplied so the diff cues are honest.
#[must_use]
pub fn project_all(steps: &[HeapStep]) -> Vec<MemoryStep> {
    steps
        .iter()
        .enumerate()
        .map(|(i, step)| project(step, i.checked_sub(1).and_then(|p| steps.get(p))))
        .collect()
}

/// Frames OUTERMOST-first: the trace hands them innermost-first (the call that is running), but a
/// reader builds the picture the way the program did — global scope at the top, the current call
/// at the bottom, each one below its caller.
fn frames_of(step: &HeapStep, previous: Option<&HeapStep>) -> Vec<Frame> {
    let depth = step.frames.len();
    step.frames
        .iter()
        .enumerate()
        .map(|(i, frame)| {
            let was = previous
                .and_then(|p| p.frames.get(p.frames.len().wrapping_sub(depth - i)))
                .filter(|earlier| earlier.fn_name == frame.fn_name);
            Frame {
                title: frame_title(&frame.fn_name),
                // The innermost frame is the one executing — index 0 before the reversal below.
                is_active: i == 0,
                slots: frame
                    .locals
                    .iter()
                    .map(|(name, value)| {
                        let before = was.and_then(|earlier| {
                            earlier.locals.iter().find(|(n, _)| n == name).map(|(_, v)| v)
                        });
                        slot(name.clone(), value, before)
                    })
                    .collect(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                },
            }
        })
        .rev()
        .collect()
}

/// `<module>` is Python's name for the file itself, and `main` is Java's entry — neither reads as
/// a frame to someone who did not write the runtime.
fn frame_title(fn_name: &str) -> String {
    match fn_name {
        "<module>" | "<main>" | "" => "Global frame".to_owned(),
        other => format!("{other}()"),
    }
}

fn slot(name: String, value: &HeapValue, before: Option<&HeapValue>) -> Slot {
    let changed = before.is_some_and(|earlier| earlier != value);
    match value {
        HeapValue::Ref(id) => Slot {
            name,
            value: String::new(),
            target: Some(id.clone()),
            changed,
        },
        HeapValue::Scalar(scalar) => Slot {
            name,
            value: elide(&scalar_text(scalar)),
            target: None,
            changed,
        },
    }
}

/// The objects to draw, in the order a reader MEETS them: breadth-first from the frames, so a
/// list a variable points at sits beside that variable rather than wherever its address sorted.
/// Unreferenced heap entries are dropped — they are the tracer's, not the program's.
fn reachable_order(step: &HeapStep) -> Vec<String> {
    let mut queue: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for frame in step.frames.iter().rev() {
        for (_, value) in &frame.locals {
            if let HeapValue::Ref(id) = value {
                push_once(&mut queue, &mut seen, id);
            }
        }
    }
    let mut at = 0;
    while at < queue.len() {
        let id = queue[at].clone();
        at += 1;
        let Some(object) = step.heap.get(&id) else {
            continue;
        };
        for value in object_values(object) {
            if let HeapValue::Ref(next) = value {
                push_once(&mut queue, &mut seen, &next);
            }
        }
    }
    queue
}

fn push_once(queue: &mut Vec<String>, seen: &mut Vec<String>, id: &str) {
    if seen.iter().any(|s| s == id) {
        return;
    }
    seen.push(id.to_owned());
    queue.push(id.to_owned());
}

fn object_values(object: &HeapObject) -> Vec<HeapValue> {
    match object {
        HeapObject::Instance { fields, .. } => fields.iter().map(|(_, v)| v.clone()).collect(),
        HeapObject::Arr { items, .. } => items.clone(),
        HeapObject::Dict { entries } => entries.iter().flat_map(|(k, v)| [k.clone(), v.clone()]).collect(),
    }
}

fn objects_of(step: &HeapStep, previous: Option<&HeapStep>, order: &[String]) -> Vec<Object> {
    order
        .iter()
        .filter_map(|id| {
            let object = step.heap.get(id)?;
            let before = previous.and_then(|p| p.heap.get(id));
            Some(build_object(id, object, before, previous))
        })
        .collect()
}

fn build_object(
    id: &str,
    object: &HeapObject,
    before: Option<&HeapObject>,
    previous: Option<&HeapStep>,
) -> Object {
    // New = this id was not in the heap a step ago. Unknowable on the first step, where
    // "everything is new" would light the whole diagram up and say nothing.
    let is_new = previous.is_some_and(|p| !p.heap.contains_key(id));
    let (title, kind, rows) = match object {
        HeapObject::Arr { kind, items } => {
            let earlier = match before {
                Some(HeapObject::Arr { items, .. }) => Some(items),
                _ => None,
            };
            let rows = items
                .iter()
                .enumerate()
                .map(|(i, value)| slot(i.to_string(), value, earlier.and_then(|e| e.get(i))))
                .collect();
            let kind = match kind {
                ArrKind::Tup => ObjKind::Tuple,
                _ => ObjKind::List,
            };
            let title = match kind {
                ObjKind::Tuple => "tuple",
                _ => "list",
            };
            (title.to_owned(), kind, rows)
        }
        HeapObject::Dict { entries } => {
            let earlier = match before {
                Some(HeapObject::Dict { entries }) => Some(entries),
                _ => None,
            };
            let rows = entries
                .iter()
                .enumerate()
                .map(|(i, (key, value))| {
                    let before = earlier.and_then(|e| e.get(i)).map(|(_, v)| v);
                    slot(elide(&value_key(key)), value, before)
                })
                .collect();
            ("dict".to_owned(), ObjKind::Dict, rows)
        }
        HeapObject::Instance { cls, fields } => {
            let earlier = match before {
                Some(HeapObject::Instance { fields, .. }) => Some(fields),
                _ => None,
            };
            let rows = fields
                .iter()
                .map(|(name, value)| {
                    let before = earlier
                        .and_then(|e| e.iter().find(|(n, _)| n == name))
                        .map(|(_, v)| v);
                    slot(name.clone(), value, before)
                })
                .collect();
            (cls.clone(), ObjKind::Instance, rows)
        }
    };
    Object {
        id: id.to_owned(),
        title,
        kind,
        rows,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        },
        cell_w: None,
        is_new,
    }
}

/// A dict key drawn as text. A key that is itself a reference has no cell of its own to point
/// from, so it shows as the object marker rather than growing a second arrow layer.
fn value_key(value: &HeapValue) -> String {
    match value {
        HeapValue::Scalar(scalar) => scalar_text(scalar),
        HeapValue::Ref(_) => "◆".to_owned(),
    }
}

/// Matches `adapt`'s labelling, so one value reads the same in both lenses — `1.0` stays a float.
fn scalar_text(scalar: &HeapScalar) -> String {
    match scalar {
        HeapScalar::I(v) => v.to_string(),
        HeapScalar::D(v) => {
            if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{v:.1}")
            } else {
                v.to_string()
            }
        }
        HeapScalar::B(v) => v.to_string(),
        HeapScalar::S(v) => format!("\"{v}\""),
        HeapScalar::Null => "None".to_owned(),
    }
}

fn elide(text: &str) -> String {
    if text.chars().count() <= VALUE_MAX_CHARS {
        return text.to_owned();
    }
    let head: String = text.chars().take(VALUE_MAX_CHARS - 1).collect();
    format!("{head}…")
}

// ─────────────────────────────────────────────────────────────────────────────
// LAYOUT — two columns, stacked; the arrows follow from the boxes
// ─────────────────────────────────────────────────────────────────────────────

fn text_w(text: &str) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let chars = text.chars().count() as f64;
    chars * CHAR_W
}

fn place(line: i32, mut frames: Vec<Frame>, mut objects: Vec<Object>) -> MemoryStep {
    // ── the frames column ──
    let frame_w = frames
        .iter()
        .map(|f| {
            let widest = f
                .slots
                .iter()
                .map(|s| text_w(&s.name).max(NAME_COL_MIN) + text_w(&s.value) + PAD_X * 3.0)
                .fold(0.0_f64, f64::max);
            widest.max(text_w(&f.title) + PAD_X * 2.0)
        })
        .fold(140.0_f64, f64::max);
    let mut y = MARGIN;
    for frame in &mut frames {
        #[allow(clippy::cast_precision_loss)]
        let h = HEAD_H + frame.slots.len() as f64 * ROW_H + 6.0;
        frame.rect = Rect {
            x: MARGIN,
            y,
            w: frame_w,
            h,
        };
        y += h + GAP_Y;
    }
    let frames_bottom = y;

    // ── the objects column ──
    let obj_x = MARGIN + frame_w + COL_GAP;
    let mut oy = MARGIN;
    for object in &mut objects {
        match object.kind {
            // A list draws as an indexed strip, the way it is indexed — one uniform cell so the
            // eye reads position rather than value width.
            ObjKind::List | ObjKind::Tuple => {
                let cell = object
                    .rows
                    .iter()
                    .map(|r| text_w(&r.value) + PAD_X)
                    .fold(CELL_MIN_W, f64::max);
                #[allow(clippy::cast_precision_loss)]
                let w = (object.rows.len().max(1) as f64).mul_add(cell, PAD_X);
                object.cell_w = Some(cell);
                object.rect = Rect {
                    x: obj_x,
                    y: oy,
                    w,
                    h: HEAD_H + ROW_H + 14.0,
                };
            }
            ObjKind::Dict | ObjKind::Instance => {
                let w = object
                    .rows
                    .iter()
                    .map(|r| text_w(&r.name).max(NAME_COL_MIN) + text_w(&r.value) + PAD_X * 3.0)
                    .fold(text_w(&object.title) + PAD_X * 2.0, f64::max)
                    .max(120.0);
                #[allow(clippy::cast_precision_loss)]
                let h = HEAD_H + object.rows.len() as f64 * ROW_H + 6.0;
                object.rect = Rect {
                    x: obj_x,
                    y: oy,
                    w,
                    h,
                };
            }
        }
        oy += object.rect.h + GAP_Y;
    }

    let arrows = arrows_of(&frames, &objects);
    let width = objects.iter().map(|o| o.rect.x + o.rect.w).fold(obj_x, f64::max) + MARGIN;
    let height = frames_bottom.max(oy) + MARGIN;
    MemoryStep {
        line,
        frames,
        objects,
        arrows,
        width,
        height,
    }
}

/// Every reference, as a line from the row that holds it to the box it names. A row pointing at
/// an object that is not drawn (a truncated heap) yields no arrow rather than one into blank
/// canvas.
fn arrows_of(frames: &[Frame], objects: &[Object]) -> Vec<Arrow> {
    let target_of = |id: &str| objects.iter().find(|o| o.id == id);
    let mut arrows = Vec::new();
    for frame in frames {
        for (i, slot) in frame.slots.iter().enumerate() {
            let Some(object) = slot.target.as_deref().and_then(target_of) else {
                continue;
            };
            arrows.push(Arrow {
                x1: frame.rect.x + frame.rect.w,
                y1: frame.rect.row_mid_y(i),
                x2: object.rect.x,
                y2: object.rect.y + object.rect.h / 2.0,
            });
        }
    }
    for object in objects {
        for (i, row) in object.rows.iter().enumerate() {
            let Some(target) = row.target.as_deref().and_then(target_of) else {
                continue;
            };
            if target.id == object.id {
                continue; // a self-reference has no line to draw between two points
            }
            // A strip's reference leaves from UNDER its cell; a row's leaves from the right.
            let (x1, y1) = if let Some(cell) = object.cell_w {
                (
                    object.rect.cell_x(i, cell) + cell / 2.0,
                    object.rect.y + object.rect.h,
                )
            } else {
                (object.rect.x + object.rect.w, object.rect.row_mid_y(i))
            };
            arrows.push(Arrow {
                x1,
                y1,
                x2: target.rect.x,
                y2: target.rect.y + target.rect.h / 2.0,
            });
        }
    }
    arrows
}

#[cfg(test)]
mod tests;
