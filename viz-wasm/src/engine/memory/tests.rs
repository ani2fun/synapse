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
