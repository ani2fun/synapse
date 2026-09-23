//! The memory lens: what it keeps, what it drops, and where the arrows land.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use std::collections::BTreeMap;

use super::*;
use crate::engine::trace::{HeapFrame, HeapTrace};

fn refv(id: &str) -> HeapValue {
    HeapValue::Ref(id.to_owned())
}

fn int(v: i64) -> HeapValue {
    HeapValue::Scalar(HeapScalar::I(v))
}

fn frame(fn_name: &str, locals: &[(&str, HeapValue)]) -> HeapFrame {
    HeapFrame {
        fn_name: fn_name.to_owned(),
        locals: locals.iter().map(|(n, v)| ((*n).to_owned(), v.clone())).collect(),
        comprehension: Vec::new(),
    }
}

fn list(items: &[HeapValue]) -> HeapObject {
    HeapObject::Arr {
        kind: ArrKind::Lst,
        items: items.to_vec(),
    }
}

fn heap(entries: &[(&str, HeapObject)]) -> BTreeMap<String, HeapObject> {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

fn step(frames: Vec<HeapFrame>, objects: &[(&str, HeapObject)]) -> HeapStep {
    HeapStep {
        line: 1,
        event: "line".to_owned(),
        frames,
        heap: heap(objects),
        out: 0,
    }
}

// ── frames ───────────────────────────────────────────────────────────────────

#[test]
fn frames_read_outermost_first_the_way_the_program_built_them() {
    // The trace hands them innermost-first; a reader wants the global frame at the top.
    let s = step(
        vec![frame("helper", &[]), frame("solve", &[]), frame("<module>", &[])],
        &[],
    );
    let m = project(&s, None);
    let titles: Vec<&str> = m.frames.iter().map(|f| f.title.as_str()).collect();
    assert_eq!(titles, ["Global frame", "solve()", "helper()"]);
}

#[test]
fn the_running_frame_is_the_active_one() {
    let s = step(vec![frame("helper", &[]), frame("<module>", &[])], &[]);
    let m = project(&s, None);
    assert!(!m.frames[0].is_active, "the global frame is not what is running");
    assert!(m.frames[1].is_active, "the innermost frame is");
}

#[test]
fn a_scalar_local_carries_its_value_and_a_reference_carries_an_arrow_instead() {
    let s = step(
        vec![frame("<module>", &[("n", int(7)), ("xs", refv("o1"))])],
        &[("o1", list(&[int(1)]))],
    );
    let m = project(&s, None);
    let slots = &m.frames[0].slots;
    assert_eq!(slots[0].value, "7");
    assert_eq!(slots[0].target, None);
    // A row cannot both name a value and point at one.
    assert_eq!(slots[1].value, "");
    assert_eq!(slots[1].target.as_deref(), Some("o1"));
}

// ── objects ──────────────────────────────────────────────────────────────────

#[test]
fn objects_come_in_the_order_a_reader_meets_them_not_by_address() {
    // `zs` is declared last but sorts first by id — reference order must win.
    let s = step(
        vec![frame("<module>", &[("xs", refv("o9")), ("zs", refv("o1"))])],
        &[("o1", list(&[])), ("o9", list(&[]))],
    );
    let m = project(&s, None);
    let ids: Vec<&str> = m.objects.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["o9", "o1"]);
}

#[test]
fn an_unreferenced_heap_entry_is_not_drawn() {
    // It belongs to the tracer, not to the program the reader wrote.
    let s = step(
        vec![frame("<module>", &[("xs", refv("o1"))])],
        &[("o1", list(&[])), ("orphan", list(&[int(4)]))],
    );
    let m = project(&s, None);
    assert_eq!(m.objects.len(), 1);
    assert_eq!(m.objects[0].id, "o1");
}

#[test]
fn a_nested_reference_is_reached_and_drawn() {
    let s = step(
        vec![frame("<module>", &[("grid", refv("outer"))])],
        &[("outer", list(&[refv("inner")])), ("inner", list(&[int(1)]))],
    );
    let m = project(&s, None);
    let ids: Vec<&str> = m.objects.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["outer", "inner"]);
}

#[test]
fn a_list_is_an_indexed_strip_and_an_instance_is_rows() {
    let s = step(
        vec![frame("<module>", &[("xs", refv("o1")), ("p", refv("o2"))])],
        &[
            ("o1", list(&[int(5), int(6)])),
            (
                "o2",
                HeapObject::Instance {
                    cls: "Point".to_owned(),
                    fields: vec![("x".to_owned(), int(1))],
                },
            ),
        ],
    );
    let m = project(&s, None);
    let xs = m.objects.iter().find(|o| o.id == "o1").unwrap();
    assert_eq!(xs.kind, ObjKind::List);
    assert!(xs.cell_w.is_some(), "a list draws as a strip of uniform cells");
    assert_eq!(
        xs.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["0", "1"]
    );
    let p = m.objects.iter().find(|o| o.id == "o2").unwrap();
    assert_eq!(p.kind, ObjKind::Instance);
    assert_eq!(p.title, "Point");
    assert_eq!(p.cell_w, None);
}

// ── diff cues ────────────────────────────────────────────────────────────────

#[test]
fn nothing_is_new_or_changed_on_the_very_first_step() {
    // "Everything is new" lights the whole diagram up and says nothing.
    let s = step(
        vec![frame("<module>", &[("n", int(1)), ("xs", refv("o1"))])],
        &[("o1", list(&[int(1)]))],
    );
    let m = project(&s, None);
    assert!(m.frames[0].slots.iter().all(|s| !s.changed));
    assert!(m.objects.iter().all(|o| !o.is_new));
}

#[test]
fn a_local_that_moved_is_marked_and_one_that_held_still_is_not() {
    let before = step(vec![frame("<module>", &[("n", int(1)), ("k", int(9))])], &[]);
    let after = step(vec![frame("<module>", &[("n", int(2)), ("k", int(9))])], &[]);
    let m = project(&after, Some(&before));
    assert!(m.frames[0].slots[0].changed, "n went 1 → 2");
    assert!(!m.frames[0].slots[1].changed, "k did not move");
}

#[test]
fn an_object_absent_a_step_ago_is_new() {
    let before = step(vec![frame("<module>", &[])], &[]);
    let after = step(
        vec![frame("<module>", &[("xs", refv("o1"))])],
        &[("o1", list(&[]))],
    );
    let m = project(&after, Some(&before));
    assert!(m.objects[0].is_new);
}

// ── layout + arrows ──────────────────────────────────────────────────────────

#[test]
fn a_reference_becomes_one_arrow_from_its_row_to_its_object() {
    let traced = step(
        vec![frame("<module>", &[("xs", refv("o1"))])],
        &[("o1", list(&[int(1)]))],
    );
    let drawn = project(&traced, None);
    assert_eq!(drawn.arrows.len(), 1);
    let arrow = &drawn.arrows[0];
    let holder = &drawn.frames[0];
    let target = &drawn.objects[0];
    // It leaves the frame's right edge and lands on the object's left edge.
    assert_eq!(arrow.x1, holder.rect.x + holder.rect.w);
    assert_eq!(arrow.x2, target.rect.x);
    assert!(
        arrow.x2 > arrow.x1,
        "the objects column sits clear of the frames column"
    );
    // And at the ROW that holds the reference, which is what `row_mid_y` is for.
    assert_eq!(arrow.y1, holder.rect.row_mid_y(0));
}

#[test]
fn a_reference_to_an_object_that_was_not_drawn_yields_no_arrow() {
    // A truncated heap loses objects; an arrow into blank canvas is worse than none.
    let s = step(vec![frame("<module>", &[("xs", refv("gone"))])], &[]);
    let m = project(&s, None);
    assert!(m.arrows.is_empty());
    assert!(m.objects.is_empty());
}

#[test]
fn a_self_referencing_object_draws_no_arrow_to_itself() {
    let s = step(
        vec![frame("<module>", &[("xs", refv("o1"))])],
        &[("o1", list(&[refv("o1")]))],
    );
    let m = project(&s, None);
    // One arrow: the frame's. The cycle has no two points to join.
    assert_eq!(m.arrows.len(), 1);
}

#[test]
fn the_canvas_is_sized_to_hold_everything_it_drew() {
    let s = step(
        vec![frame("<module>", &[("xs", refv("o1"))])],
        &[("o1", list(&[int(1), int(2), int(3)]))],
    );
    let m = project(&s, None);
    let o = &m.objects[0];
    assert!(
        m.width >= o.rect.x + o.rect.w,
        "width {} vs {}",
        m.width,
        o.rect.x + o.rect.w
    );
    assert!(m.height >= o.rect.y + o.rect.h);
}

#[test]
fn frames_stack_without_overlapping() {
    let s = step(
        vec![
            frame("inner", &[("a", int(1))]),
            frame("<module>", &[("b", int(2)), ("c", int(3))]),
        ],
        &[],
    );
    let m = project(&s, None);
    let first = m.frames[0].rect;
    let second = m.frames[1].rect;
    assert!(
        second.y >= first.y + first.h,
        "frames must not sit on top of each other"
    );
}

// ── values ───────────────────────────────────────────────────────────────────

#[test]
fn a_string_is_quoted_so_it_cannot_be_read_as_a_number() {
    let s = step(
        vec![frame(
            "<module>",
            &[("s", HeapValue::Scalar(HeapScalar::S("7".to_owned())))],
        )],
        &[],
    );
    let m = project(&s, None);
    assert_eq!(m.frames[0].slots[0].value, "\"7\"");
}

#[test]
fn an_integral_float_keeps_its_point() {
    let s = step(
        vec![frame("<module>", &[("f", HeapValue::Scalar(HeapScalar::D(1.0)))])],
        &[],
    );
    let m = project(&s, None);
    assert_eq!(m.frames[0].slots[0].value, "1.0");
}

#[test]
fn a_runaway_value_is_elided_rather_than_drawn() {
    let long = "x".repeat(400);
    let s = step(
        vec![frame(
            "<module>",
            &[("s", HeapValue::Scalar(HeapScalar::S(long)))],
        )],
        &[],
    );
    let m = project(&s, None);
    let value = &m.frames[0].slots[0].value;
    assert!(value.chars().count() <= 36, "{} chars", value.chars().count());
    assert!(value.ends_with('…'));
}

// ── the whole trace ──────────────────────────────────────────────────────────

#[test]
fn project_all_hands_each_step_its_own_predecessor() {
    let trace = HeapTrace {
        steps: vec![
            step(vec![frame("<module>", &[("n", int(1))])], &[]),
            step(vec![frame("<module>", &[("n", int(2))])], &[]),
        ],
        ..HeapTrace::default()
    };
    let all = project_all(&trace.steps);
    assert_eq!(all.len(), 2);
    assert!(
        !all[0].frames[0].slots[0].changed,
        "the first step has no predecessor"
    );
    assert!(all[1].frames[0].slots[0].changed, "the second does");
}

// ── the program's own shape ──────────────────────────────────────────────────

fn node(val: i64, next: Option<&str>) -> HeapObject {
    let mut fields = vec![("val".to_owned(), int(val))];
    if let Some(n) = next {
        fields.push(("next".to_owned(), refv(n)));
    }
    HeapObject::Instance {
        cls: "Node".to_owned(),
        fields,
    }
}

fn names(frame: &Frame) -> Vec<&str> {
    frame.slots.iter().map(|s| s.name.as_str()).collect()
}

#[test]
fn an_inlined_comprehension_gets_a_frame_of_its_own_below_its_owner() {
    // 3.12+ at module scope: the globals stay in the Global frame, and the comprehension's
    // variables — which are no global's — sit in their own frame, the running one.
    let mut module = frame("<module>", &[("rows", int(3))]);
    module.comprehension = vec![("r".to_owned(), int(1)), ("t".to_owned(), int(2))];
    let m = project(&step(vec![module], &[]), None);
    let titles: Vec<&str> = m.frames.iter().map(|f| f.title.as_str()).collect();
    assert_eq!(titles, ["Global frame", "comprehension"]);
    assert_eq!(names(&m.frames[0]), ["rows"]);
    assert_eq!(names(&m.frames[1]), ["r", "t"]);
    assert!(
        !m.frames[0].is_active,
        "the owner is waiting on the comprehension"
    );
    assert!(m.frames[1].is_active, "the comprehension is where execution is");
}

#[test]
fn a_comprehension_waiting_on_a_call_is_not_the_running_frame() {
    // `[f(x) for x in xs]` at module scope, standing inside f: f runs, the comprehension waits.
    let mut module = frame("<module>", &[]);
    module.comprehension = vec![("x".to_owned(), int(1))];
    let m = project(&step(vec![frame("f", &[]), module], &[]), None);
    let titles: Vec<&str> = m.frames.iter().map(|f| f.title.as_str()).collect();
    assert_eq!(titles, ["Global frame", "comprehension", "f()"]);
    let active: Vec<bool> = m.frames.iter().map(|f| f.is_active).collect();
    assert_eq!(active, [false, false, true]);
}

#[test]
fn a_nested_object_sits_in_the_column_after_whatever_points_at_it() {
    // matrix → three rows: the rows go in the next column, below the strip whose cells point at
    // them, and no two of them overlap.
    let s = step(
        vec![frame("<module>", &[("matrix", refv("m"))])],
        &[
            ("m", list(&[refv("r0"), refv("r1"), refv("r2")])),
            ("r0", list(&[int(1), int(2), int(3)])),
            ("r1", list(&[int(4), int(5), int(6)])),
            ("r2", list(&[int(7), int(8), int(9)])),
        ],
    );
    let m = project(&s, None);
    let outer = m.objects.iter().find(|o| o.id == "m").unwrap();
    let rows: Vec<&Object> = m.objects.iter().filter(|o| o.id.starts_with('r')).collect();
    assert_eq!(outer.column, 0);
    assert_eq!(rows.len(), 3);
    for row in &rows {
        assert_eq!(row.column, 1, "{} is not in the next column", row.id);
        assert!(
            row.rect.x >= outer.rect.x + outer.rect.w,
            "{} is not right of the matrix",
            row.id
        );
        assert!(
            row.rect.y >= outer.rect.y + outer.rect.h,
            "a strip's children start below it"
        );
    }
    for pair in rows.windows(2) {
        assert!(
            pair[1].rect.y >= pair[0].rect.y + pair[0].rect.h,
            "siblings overlap"
        );
    }
    // Each cell's arrow leaves from under ITS cell, heading down — and an object's references
    // are never a caller's, so none of them recede.
    let from_cells: Vec<&Arrow> = m.arrows.iter().filter(|a| a.from_below).collect();
    assert_eq!(from_cells.len(), 3);
    assert!(from_cells.iter().all(|a| !a.dim));
    let xs: Vec<f64> = from_cells.iter().map(|a| a.x1).collect();
    assert!(
        xs.windows(2).all(|w| w[0] < w[1]),
        "every arrow leaves its own cell"
    );
}

#[test]
fn a_linked_list_reads_left_to_right_level_with_the_field_that_points_on() {
    let s = step(
        vec![frame("<module>", &[("head", refv("a"))])],
        &[
            ("a", node(1, Some("b"))),
            ("b", node(2, Some("c"))),
            ("c", node(3, None)),
        ],
    );
    let m = project(&s, None);
    let [first, second, third] = ["a", "b", "c"].map(|id| m.objects.iter().find(|o| o.id == id).unwrap());
    assert_eq!((first.column, second.column, third.column), (0, 1, 2));
    assert!(first.rect.x < second.rect.x && second.rect.x < third.rect.x);
    // `next` is the first node's second row; the second node sits level with it, so the arrow
    // runs across.
    assert_eq!(second.rect.y, first.rect.row_y(1));
}

#[test]
fn a_reference_back_to_an_earlier_column_still_draws() {
    // A doubly linked pair: b points back at a, which sits to its LEFT.
    let back = HeapObject::Instance {
        cls: "Node".to_owned(),
        fields: vec![("prev".to_owned(), refv("a"))],
    };
    let s = step(
        vec![frame("<module>", &[("head", refv("a"))])],
        &[("a", node(1, Some("b"))), ("b", back)],
    );
    let m = project(&s, None);
    let a = m.objects.iter().find(|o| o.id == "a").unwrap();
    let b = m.objects.iter().find(|o| o.id == "b").unwrap();
    assert!(
        m.arrows
            .iter()
            .any(|arrow| arrow.x1 == b.rect.x + b.rect.w && arrow.x2 == a.rect.x),
        "no arrow from b back to a"
    );
}

#[test]
fn a_callers_references_recede_and_the_running_frames_do_not() {
    let s = step(
        vec![
            frame("solve", &[("xs", refv("l"))]),
            frame("<module>", &[("data", refv("l"))]),
        ],
        &[("l", list(&[int(1)]))],
    );
    let m = project(&s, None);
    let leaving = |f: &Frame| {
        m.arrows
            .iter()
            .find(|a| a.y1 == f.rect.row_mid_y(0) && a.x1 == f.rect.x + f.rect.w)
            .unwrap()
    };
    assert!(leaving(&m.frames[0]).dim, "the Global frame is a caller here");
    assert!(!leaving(&m.frames[1]).dim, "solve() is running");
}

#[test]
fn an_empty_list_says_so_rather_than_drawing_an_empty_strip() {
    let s = step(
        vec![frame("<module>", &[("ans", refv("l"))])],
        &[("l", list(&[]))],
    );
    let m = project(&s, None);
    let empty = &m.objects[0];
    assert_eq!(empty.title, "empty list");
    assert!(empty.cell_w.is_none());
    assert!(
        empty.rect.w < ROWS_MIN_W,
        "a box with nothing in it is as wide as its title"
    );
}

#[test]
fn a_function_is_its_signature_and_a_class_lists_its_members() {
    let s = step(
        vec![frame("<module>", &[("Solution", refv("cls"))])],
        &[
            (
                "cls",
                HeapObject::Class {
                    name: "Solution".to_owned(),
                    members: vec![("spiralOrder".to_owned(), refv("fn"))],
                },
            ),
            (
                "fn",
                HeapObject::Function {
                    signature: "spiralOrder(self, matrix)".to_owned(),
                },
            ),
        ],
    );
    let m = project(&s, None);
    let class = m.objects.iter().find(|o| o.id == "cls").unwrap();
    let function = m.objects.iter().find(|o| o.id == "fn").unwrap();
    assert_eq!(
        (class.title.as_str(), class.kind),
        ("Solution class", ObjKind::Class)
    );
    assert_eq!(class.rows[0].name, "spiralOrder");
    assert_eq!(class.rows[0].target.as_deref(), Some("fn"));
    assert_eq!(
        (function.title.as_str(), function.kind),
        ("function spiralOrder(self, matrix)", ObjKind::Function)
    );
    assert_eq!(function.column, class.column + 1);
}

#[test]
fn a_returned_value_is_marked_as_the_frames_result() {
    let s = step(
        vec![frame("solve", &[("n", int(1)), (RETURN_VALUE, int(7))])],
        &[],
    );
    let m = project(&s, None);
    let flags: Vec<bool> = m.frames[0].slots.iter().map(|s| s.is_return).collect();
    assert_eq!(flags, [false, true]);
}

#[test]
fn the_columns_are_named_above_both_of_them() {
    let s = step(
        vec![frame("<module>", &[("xs", refv("l"))])],
        &[("l", list(&[int(1)]))],
    );
    let m = project(&s, None);
    assert_eq!(
        m.frames[0].rect.y,
        MARGIN + HEADER_H,
        "the first frame sits under the header band"
    );
    assert_eq!(
        m.objects_x, m.objects[0].rect.x,
        "the Objects header sits over the first column"
    );
}

#[test]
fn only_the_outermost_frame_returning_ends_the_run() {
    let at = |event: &str, frames: Vec<HeapFrame>| HeapStep {
        event: event.to_owned(),
        ..step(frames, &[])
    };
    assert!(project(&at("return", vec![frame("<module>", &[])]), None).ends_run);
    assert!(
        !project(
            &at("return", vec![frame("solve", &[]), frame("<module>", &[])]),
            None
        )
        .ends_run,
        "a call returning is not the end of the program"
    );
    assert!(!project(&at("line", vec![frame("<module>", &[])]), None).ends_run);
}
