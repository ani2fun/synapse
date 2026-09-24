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
//! **Objects sit in COLUMNS BY DEPTH.** What a frame points at is column 1, what those point at is
//! column 2, and so on, each object level with the row that first referred to it — so a reference
//! reads ACROSS the canvas, a matrix's rows sit beside the matrix, and a linked list reads left to
//! right. One column would stack a child below unrelated boxes and send its arrow looping past
//! them.
//!
//! **Geometry is computed HERE, in pure Rust, not measured in the DOM.** The renderer emits one
//! SVG from these boxes, so the arrows land exactly where the rows are, the layout is identical
//! across browsers, and every rule below is testable natively. The cost is that text is MEASURED
//! BY CHARACTER COUNT — sound only because the figure is drawn in the mono face, which the
//! stylesheet pins.

use std::collections::{HashMap, HashSet};

use crate::engine::trace::{ArrKind, HeapObject, HeapScalar, HeapStep, HeapValue, TraceEvent};

// ── metrics (px, at the 12px mono the sheet pins) ──
// The renderer reads these too: a box drawn from one set of numbers and an arrow aimed with
// another is a bug that only shows up as a line landing slightly off a row.
const CHAR_W: f64 = 7.3;
pub const ROW_H: f64 = 22.0;
pub const HEAD_H: f64 = 26.0;
pub const PAD_X: f64 = 10.0;
/// The band above both columns that names them — "Frames", "Objects".
pub const HEADER_H: f64 = 22.0;
/// Where those names' baseline sits, and where the frames column's left edge is.
pub const HEADER_BASELINE: f64 = MARGIN + HEADER_H - 8.0; // 8px clear of the first box
pub const FRAMES_X: f64 = MARGIN;
const GAP_Y: f64 = 18.0;
/// Between the frames column and the objects column — the arrows' whole run.
const COL_GAP: f64 = 96.0;
/// Between one objects column and the next: a reference's run from a parent to its child.
const OBJ_COL_GAP: f64 = 56.0;
const MARGIN: f64 = 12.0;
/// Below a box's last row — breathing room, so the bottom row does not sit on the border.
const BOX_PAD_B: f64 = 6.0;
/// Below a strip's cells: the rail its indices are drawn on.
const INDEX_RAIL_H: f64 = 14.0;
/// The frames column is never narrower than this, so a lone `Global frame` does not read as a tag.
const FRAMES_MIN_W: f64 = 140.0;

// ── where text sits, for the renderer — every one an offset from a geometry the layout owns ──
/// A box title's baseline, below the box's top edge.
pub const TITLE_BASELINE: f64 = 17.0;
/// A row's (or a cell's) text baseline, below the row's top edge.
pub const ROW_BASELINE: f64 = 15.0;
/// A cell's index baseline, below the cell's top edge — inside `INDEX_RAIL_H`.
pub const INDEX_BASELINE: f64 = ROW_H + 12.0;
/// Every box's corner radius.
pub const CORNER_R: f64 = 7.0;
/// One array cell, and the floor a short value still occupies.
const CELL_MIN_W: f64 = 30.0;
const NAME_COL_MIN: f64 = 52.0;
/// The narrowest a box of rows is drawn — below it a short name and value crowd the edges.
const ROWS_MIN_W: f64 = 120.0;
/// Past this a value is elided: a 400-character repr is not a diagram.
const VALUE_MAX_CHARS: usize = 36;

/// The name the harness gives what a returning frame hands back: a synthetic local, spaced so it
/// can never collide with one the reader wrote. The canvas marks it so it does not read as one
/// more variable.
pub const RETURN_VALUE: &str = "Return value";

/// Titles the frame of a comprehension the runtime inlined into its owner.
const COMPREHENSION: &str = "comprehension";

#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
    Function,
    Class,
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
    /// The frame's result rather than a variable — see [`RETURN_VALUE`].
    pub is_return: bool,
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
    /// Set for a non-empty list/tuple, whose rows draw as a horizontal indexed strip rather than
    /// a column.
    pub cell_w: Option<f64>,
    pub is_new: bool,
    /// Which objects column it sits in: 0 = a frame points at it, 1 = something in column 0 does.
    pub column: usize,
}

/// A reference, resolved to the two points it joins.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrow {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    /// It leaves a frame that is not the running one — a caller's reference, still true, but not
    /// where the reader's attention belongs.
    pub dim: bool,
    /// It leaves from UNDER an array cell, heading down, rather than out of a row's right edge.
    pub from_below: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MemoryStep {
    pub line: i32,
    /// Bytes of program output printed by the time this step ran — the slice of the run's output
    /// a reader standing here has actually seen the program produce.
    pub out: usize,
    pub frames: Vec<Frame>,
    pub objects: Vec<Object>,
    pub arrows: Vec<Arrow>,
    pub width: f64,
    pub height: f64,
    /// Where the objects columns begin — the "Objects" header sits here.
    pub objects_x: f64,
    /// The OUTERMOST frame returning: the program has finished, so the line on this step has run
    /// and there is no next one.
    pub ends_run: bool,
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
    let reached = reachable(&frames, step);
    let objects = reached
        .into_iter()
        .filter_map(|found| {
            let object = step.heap.get(&found.id)?;
            let before = previous.and_then(|p| p.heap.get(&found.id));
            let built = build_object(&found.id, object, before, previous, found.depth - 1);
            Some((built, found.from))
        })
        .collect();
    let mut laid = place(step.line, step.out, frames, objects);
    laid.ends_run = step.event == TraceEvent::Return && step.frames.len() == 1;
    laid
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
///
/// A comprehension the runtime INLINED into a frame gets a frame of its own, directly below its
/// owner. Its variables belong to neither the owner nor a call, and where it is running it is the
/// running frame — the owner is waiting on it, the way a caller waits on a call.
fn frames_of(step: &HeapStep, previous: Option<&HeapStep>) -> Vec<Frame> {
    let depth = step.frames.len();
    let mut frames: Vec<Frame> = step
        .frames
        .iter()
        .enumerate()
        .flat_map(|(i, frame)| {
            // The same frame a step ago is the one at the same distance from the OUTERMOST — the
            // innermost end is where calls come and go, so counting from it would pair a caller
            // with the callee that just returned.
            let from_outermost = depth - i;
            let was = previous
                .and_then(|p| {
                    p.frames
                        .len()
                        .checked_sub(from_outermost)
                        .and_then(|at| p.frames.get(at))
                })
                .filter(|earlier| earlier.fn_name == frame.fn_name);
            // The innermost frame is the one executing — index 0 before the reversal below.
            let innermost = i == 0;
            let inlined = !frame.comprehension.is_empty();
            let comprehension = inlined.then(|| Frame {
                title: COMPREHENSION.to_owned(),
                is_active: innermost,
                slots: slots_of(&frame.comprehension, was.map(|w| w.comprehension.as_slice())),
                rect: Rect::default(),
            });
            let owner = Frame {
                title: frame_title(&frame.fn_name),
                is_active: innermost && !inlined,
                slots: slots_of(&frame.locals, was.map(|w| w.locals.as_slice())),
                rect: Rect::default(),
            };
            // Innermost-first, like the trace: the comprehension runs INSIDE its owner.
            comprehension.into_iter().chain(std::iter::once(owner))
        })
        .collect();
    frames.reverse();
    frames
}

/// `<module>` is Python's name for the file itself, and `main` is Java's entry — neither reads as
/// a frame to someone who did not write the runtime.
fn frame_title(fn_name: &str) -> String {
    match fn_name {
        "<module>" | "<main>" | "" => "Global frame".to_owned(),
        other => format!("{other}()"),
    }
}

/// Named values as rows, each marked changed against the same name a step ago.
fn slots_of(values: &[(String, HeapValue)], earlier: Option<&[(String, HeapValue)]>) -> Vec<Slot> {
    values
        .iter()
        .map(|(name, value)| {
            let before = earlier.and_then(|e| e.iter().find(|(n, _)| n == name).map(|(_, v)| v));
            slot(name.clone(), value, before)
        })
        .collect()
}

fn slot(name: String, value: &HeapValue, before: Option<&HeapValue>) -> Slot {
    let changed = before.is_some_and(|earlier| earlier != value);
    let is_return = name == RETURN_VALUE;
    match value {
        HeapValue::Ref(id) => Slot {
            name,
            value: String::new(),
            target: Some(id.clone()),
            changed,
            is_return,
        },
        HeapValue::Scalar(scalar) => Slot {
            name,
            value: elide(&scalar_text(scalar)),
            target: None,
            changed,
            is_return,
        },
    }
}

/// Whatever first pointed at an object, which is where the layout puts it: level with that row.
#[derive(Debug, Clone, PartialEq)]
enum Referrer {
    Frame { frame: usize, row: usize },
    Object { parent: String, row: usize },
}

struct Reached {
    id: String,
    /// 1 = a frame points at it.
    depth: usize,
    from: Referrer,
}

/// The objects to draw, in the order a reader MEETS them — breadth-first from the frames, so a
/// list a variable points at sits beside that variable rather than wherever its address sorted —
/// each with its depth and what reached it first. Unreferenced heap entries are dropped: they are
/// the tracer's, not the program's.
fn reachable(frames: &[Frame], step: &HeapStep) -> Vec<Reached> {
    let mut reached: Vec<Reached> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (frame, f) in frames.iter().enumerate() {
        for (row, slot) in f.slots.iter().enumerate() {
            if let Some(id) = &slot.target
                && seen.insert(id.clone())
            {
                reached.push(Reached {
                    id: id.clone(),
                    depth: 1,
                    from: Referrer::Frame { frame, row },
                });
            }
        }
    }
    let mut at = 0;
    while at < reached.len() {
        let (id, depth) = (reached[at].id.clone(), reached[at].depth);
        at += 1;
        let Some(object) = step.heap.get(&id) else {
            continue;
        };
        for (row, next) in object_refs(object) {
            if seen.insert(next.to_owned()) {
                reached.push(Reached {
                    id: next.to_owned(),
                    depth: depth + 1,
                    from: Referrer::Object {
                        parent: id.clone(),
                        row,
                    },
                });
            }
        }
    }
    reached
}

/// Every reference an object holds, with the ROW it is drawn on — a dict entry's key and value
/// share their entry's row.
fn object_refs(object: &HeapObject) -> Vec<(usize, &str)> {
    match object {
        HeapObject::Instance { fields, .. } | HeapObject::Class { members: fields, .. } => fields
            .iter()
            .enumerate()
            .filter_map(|(row, (_, value))| as_ref(value).map(|id| (row, id)))
            .collect(),
        HeapObject::Arr { items, .. } => items
            .iter()
            .enumerate()
            .filter_map(|(row, value)| as_ref(value).map(|id| (row, id)))
            .collect(),
        HeapObject::Dict { entries } => entries
            .iter()
            .enumerate()
            .flat_map(|(row, (key, value))| {
                [as_ref(key), as_ref(value)]
                    .into_iter()
                    .flatten()
                    .map(move |id| (row, id))
            })
            .collect(),
        HeapObject::Function { .. } => Vec::new(),
    }
}

fn as_ref(value: &HeapValue) -> Option<&str> {
    match value {
        HeapValue::Ref(id) => Some(id),
        HeapValue::Scalar(_) => None,
    }
}

fn build_object(
    id: &str,
    object: &HeapObject,
    before: Option<&HeapObject>,
    previous: Option<&HeapStep>,
    column: usize,
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
            // An empty one says so in words: a strip with no cells reads as a drawing that failed.
            let title = match (kind, items.is_empty()) {
                (ObjKind::Tuple, true) => "empty tuple",
                (ObjKind::Tuple, false) => "tuple",
                (_, true) => "empty list",
                (_, false) => "list",
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
            let title = if entries.is_empty() { "empty dict" } else { "dict" };
            (title.to_owned(), ObjKind::Dict, rows)
        }
        HeapObject::Instance { cls, fields } => {
            let earlier = match before {
                Some(HeapObject::Instance { fields, .. }) => Some(fields.as_slice()),
                _ => None,
            };
            (cls.clone(), ObjKind::Instance, slots_of(fields, earlier))
        }
        HeapObject::Class { members, .. } => {
            let earlier = match before {
                Some(HeapObject::Class { members, .. }) => Some(members.as_slice()),
                _ => None,
            };
            let title = object.code_title().unwrap_or_default();
            (title, ObjKind::Class, slots_of(members, earlier))
        }
        HeapObject::Function { .. } => {
            let title = object.code_title().unwrap_or_default();
            (title, ObjKind::Function, Vec::new())
        }
    };
    Object {
        id: id.to_owned(),
        title,
        kind,
        rows,
        rect: Rect::default(),
        cell_w: None,
        is_new,
        column,
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
// LAYOUT — frames, then objects in columns by depth; the arrows follow from the boxes
// ─────────────────────────────────────────────────────────────────────────────

fn text_w(text: &str) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let chars = text.chars().count() as f64;
    chars * CHAR_W
}

/// How wide a `name │ value` row needs its box to be — one rule for frames and objects alike.
fn row_w(slot: &Slot) -> f64 {
    text_w(&slot.name).max(NAME_COL_MIN) + text_w(&slot.value) + PAD_X * 3.0
}

/// A box of `rows` rows, header included.
fn rows_h(rows: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let n = rows as f64;
    n.mul_add(ROW_H, HEAD_H + BOX_PAD_B)
}

/// A box's own size, before it has a place.
fn size(object: &mut Object) {
    let is_strip = matches!(object.kind, ObjKind::List | ObjKind::Tuple) && !object.rows.is_empty();
    if is_strip {
        // A list draws as an indexed strip, the way it is indexed — one uniform cell so the eye
        // reads position rather than value width.
        let cell = object
            .rows
            .iter()
            .map(|r| text_w(&r.value) + PAD_X)
            .fold(CELL_MIN_W, f64::max);
        #[allow(clippy::cast_precision_loss)]
        let w = (object.rows.len() as f64).mul_add(cell, PAD_X);
        object.cell_w = Some(cell);
        object.rect.w = w.max(text_w(&object.title) + PAD_X * 2.0);
        object.rect.h = HEAD_H + ROW_H + INDEX_RAIL_H;
        return;
    }
    // A box with nothing inside it — a function, an empty list, an instance with no fields — is
    // as wide as its title and no wider; the floor is for rows that would crowd its edges.
    let floor = if object.rows.is_empty() { 0.0 } else { ROWS_MIN_W };
    object.rect.w = object
        .rows
        .iter()
        .map(row_w)
        .fold(text_w(&object.title) + PAD_X * 2.0, f64::max)
        .max(floor);
    object.rect.h = rows_h(object.rows.len());
}

/// Boxes and arrows for one step. `placed` is in breadth-first order, each object with whatever
/// first pointed at it — which is where it prefers to sit.
fn place(line: i32, out: usize, mut frames: Vec<Frame>, placed: Vec<(Object, Referrer)>) -> MemoryStep {
    let (mut objects, referrers): (Vec<Object>, Vec<Referrer>) = placed.into_iter().unzip();
    let top = MARGIN + HEADER_H;

    // ── the frames column ──
    let frame_w = frames
        .iter()
        .map(|f| {
            f.slots
                .iter()
                .map(row_w)
                .fold(text_w(&f.title) + PAD_X * 2.0, f64::max)
        })
        .fold(FRAMES_MIN_W, f64::max);
    let mut y = top;
    for frame in &mut frames {
        let h = rows_h(frame.slots.len());
        frame.rect = Rect {
            x: MARGIN,
            y,
            w: frame_w,
            h,
        };
        y += h + GAP_Y;
    }
    let frames_bottom = y;

    // ── the objects: sized, then placed column by column ──
    for object in &mut objects {
        size(object);
    }
    let columns = objects.iter().map(|o| o.column + 1).max().unwrap_or(0);
    let mut column_w = vec![0.0_f64; columns];
    for object in &objects {
        column_w[object.column] = column_w[object.column].max(object.rect.w);
    }
    let objects_x = MARGIN + frame_w + COL_GAP;
    let mut column_x = Vec::with_capacity(columns);
    let mut x = objects_x;
    for w in &column_w {
        column_x.push(x);
        x += w + OBJ_COL_GAP;
    }
    let index: HashMap<String, usize> = objects
        .iter()
        .enumerate()
        .map(|(i, o)| (o.id.clone(), i))
        .collect();
    // How far down each column is already taken — a box never lands on one placed before it.
    let mut taken = vec![top; columns];
    for (i, referrer) in referrers.iter().enumerate() {
        // Breadth-first order puts every parent before its children, so a parent is always among
        // the boxes already placed.
        let (placed, rest) = objects.split_at_mut(i);
        let object = &mut rest[0];
        let preferred = match referrer {
            Referrer::Frame { frame, row } => frames.get(*frame).map_or(top, |f| f.rect.row_y(*row)),
            Referrer::Object { parent, row } => {
                index
                    .get(parent)
                    .and_then(|&p| placed.get(p))
                    .map_or(top, |parent| {
                        // A strip's references leave from UNDER its cells, so its children start below
                        // it and every one of those arrows runs down and across; a box of rows points
                        // from the row itself, so its child sits level with that row.
                        if parent.cell_w.is_some() {
                            parent.rect.y + parent.rect.h + GAP_Y
                        } else {
                            parent.rect.row_y(*row)
                        }
                    })
            }
        };
        let column = object.column;
        object.rect.x = column_x[column];
        object.rect.y = preferred.max(taken[column]);
        taken[column] = object.rect.y + object.rect.h + GAP_Y;
    }

    let arrows = arrows_of(&frames, &objects, &index);
    let width = objects
        .iter()
        .map(|o| o.rect.x + o.rect.w)
        .fold(objects_x, f64::max)
        + MARGIN;
    let height = taken.iter().copied().fold(frames_bottom, f64::max) + MARGIN;
    MemoryStep {
        line,
        out,
        frames,
        objects,
        arrows,
        width,
        height,
        objects_x,
        ends_run: false,
    }
}

/// Every reference, as a line from the row that holds it to the box it names. A row pointing at
/// an object that is not drawn (a truncated heap) yields no arrow rather than one into blank
/// canvas.
fn arrows_of(frames: &[Frame], objects: &[Object], index: &HashMap<String, usize>) -> Vec<Arrow> {
    let target_of = |id: &str| index.get(id).and_then(|&i| objects.get(i));
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
                dim: !frame.is_active,
                from_below: false,
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
            let (x1, y1, from_below) = if let Some(cell) = object.cell_w {
                (
                    object.rect.cell_x(i, cell) + cell / 2.0,
                    object.rect.y + object.rect.h,
                    true,
                )
            } else {
                (object.rect.x + object.rect.w, object.rect.row_mid_y(i), false)
            };
            arrows.push(Arrow {
                x1,
                y1,
                x2: target.rect.x,
                y2: target.rect.y + target.rect.h / 2.0,
                dim: false,
                from_below,
            });
        }
    }
    arrows
}

#[cfg(test)]
mod tests;
