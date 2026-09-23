//! The memory canvas: frames on the left, objects on the right, one arrow per reference.
//!
//! Everything it draws comes laid out from `engine::memory` — this file has no measurements and
//! no opinions about placement, only about paint. That split is what makes the arrows land on the
//! rows they belong to: both ends were computed from the same boxes.
//!
//! The arrows are CURVED, and deliberately: a straight line from a frame row to an object cuts
//! through whatever sits between them, and a diagram whose references cross its own boxes is the
//! one thing this lens exists to make legible. A horizontal bezier leaves and lands flat, so the
//! eye follows it out of one box and into the next.

use crate::engine::memory::{
    Arrow, FRAMES_X, Frame, HEAD_H, HEADER_BASELINE, MemoryStep, ObjKind, Object, PAD_X, ROW_H, Rect, Slot,
};
use leptos::prelude::*;

use super::arrow_defs;

/// The whole diagram for one step.
#[must_use]
pub fn canvas(steps: Vec<MemoryStep>, step_index: Signal<usize>) -> AnyView {
    // ONE viewBox for every step, taken from the widest and tallest: sizing per step would have
    // the picture jump as a list grows, which is exactly the movement stepping must not have.
    let width = steps.iter().map(|s| s.width).fold(320.0_f64, f64::max);
    let height = steps.iter().map(|s| s.height).fold(200.0_f64, f64::max);
    let view_box = format!("0 0 {width} {height}");
    view! {
        <svg class="viz-svg viz-mem" viewBox=view_box width=width height=height>
            {arrow_defs()}
            {move || {
                let Some(step) = steps.get(step_index.get().min(steps.len().saturating_sub(1)))
                else {
                    return ().into_any();
                };
                let arrows: Vec<_> = step.arrows.iter().map(arrow).collect();
                let frames: Vec<_> = step.frames.iter().map(frame).collect();
                let objects: Vec<_> = step.objects.iter().map(object).collect();
                // The two column names, so a first-time reader knows which side is the call
                // stack and which is what it points at.
                let objects_header = (!step.objects.is_empty()).then(|| view! {
                    <text class="viz-mem__colhead" x=step.objects_x y=HEADER_BASELINE>"Objects"</text>
                });
                view! {
                    <text class="viz-mem__colhead" x=FRAMES_X y=HEADER_BASELINE>"Frames"</text>
                    {objects_header}
                    // Arrows first, so a line passes BEHIND the boxes it runs between.
                    <g class="viz-mem__arrows">{arrows}</g>
                    <g class="viz-mem__frames">{frames}</g>
                    <g class="viz-mem__objects">{objects}</g>
                }
                .into_any()
            }}
        </svg>
    }
    .into_any()
}

fn frame(f: &Frame) -> AnyView {
    let r = f.rect;
    let class = if f.is_active {
        "viz-mem__frame viz-mem__frame--active"
    } else {
        "viz-mem__frame"
    };
    let rows: Vec<_> = f
        .slots
        .iter()
        .enumerate()
        .map(|(i, slot)| row(slot, r, i))
        .collect();
    view! {
        <g class=class>
            <rect class="viz-mem__box" x=r.x y=r.y width=r.w height=r.h rx="7"></rect>
            <rect class="viz-mem__head" x=r.x y=r.y width=r.w height=HEAD_H rx="7"></rect>
            <text class="viz-mem__title" x=r.x + PAD_X y=r.y + 17.0>{f.title.clone()}</text>
            {rows}
        </g>
    }
    .into_any()
}

/// A `name │ value` line, placed by the box that owns it — never by arithmetic of its own, or
/// the arrows aimed at these rows would land beside them.
fn row(slot: &Slot, box_rect: Rect, index: usize) -> AnyView {
    let x = box_rect.x;
    let y = box_rect.row_y(index);
    let w = box_rect.w;
    let class = match (slot.changed, slot.is_return) {
        (_, true) => "viz-mem__row viz-mem__row--return",
        (true, false) => "viz-mem__row viz-mem__row--changed",
        (false, false) => "viz-mem__row",
    };
    let value = (!slot.value.is_empty()).then(|| {
        view! {
            <text class="viz-mem__value" x=x + w - PAD_X y=y + 15.0 text-anchor="end">
                {slot.value.clone()}
            </text>
        }
    });
    // A reference row says so where its value would be — the arrow leaves from here, and a blank
    // cell would read as an empty variable rather than a pointer.
    let pointer = slot.target.is_some().then(|| {
        view! {
            <text class="viz-mem__ref" x=x + w - PAD_X y=y + 15.0 text-anchor="end">"●"</text>
        }
    });
    view! {
        <g class=class>
            <rect class="viz-mem__rowbg" x=x + 1.0 y=y width=w - 2.0 height=ROW_H></rect>
            <text class="viz-mem__name" x=x + PAD_X y=y + 15.0>{slot.name.clone()}</text>
            {value}
            {pointer}
        </g>
    }
    .into_any()
}

fn object(o: &Object) -> AnyView {
    let r = o.rect;
    // A function or class is part of the program, not its data, and is tinted so the eye can
    // skip past it to the lists and nodes the algorithm is actually changing.
    let kind = match o.kind {
        ObjKind::Function => " viz-mem__obj--function",
        ObjKind::Class => " viz-mem__obj--class",
        ObjKind::List | ObjKind::Tuple | ObjKind::Dict | ObjKind::Instance => "",
    };
    let fresh = if o.is_new { " viz-mem__obj--new" } else { "" };
    let class = format!("viz-mem__obj{kind}{fresh}");
    let body: Vec<AnyView> = match (o.kind, o.cell_w) {
        (ObjKind::List | ObjKind::Tuple, Some(cell)) => o
            .rows
            .iter()
            .enumerate()
            .map(|(i, slot)| cell_view(slot, r.cell_x(i, cell), r.y + HEAD_H, cell, i))
            .collect(),
        _ => o
            .rows
            .iter()
            .enumerate()
            .map(|(i, slot)| row(slot, r, i))
            .collect(),
    };
    view! {
        <g class=class>
            <rect class="viz-mem__box" x=r.x y=r.y width=r.w height=r.h rx="7"></rect>
            <text class="viz-mem__objtitle" x=r.x + PAD_X y=r.y + 17.0>{o.title.clone()}</text>
            {body}
        </g>
    }
    .into_any()
}

/// One array cell: the value boxed, its INDEX under it — the same rail the array renderer draws,
/// because an index is how a list is addressed and a strip without one is just a row of numbers.
fn cell_view(slot: &Slot, x: f64, y: f64, w: f64, index: usize) -> AnyView {
    let class = if slot.changed {
        "viz-mem__cell viz-mem__cell--changed"
    } else {
        "viz-mem__cell"
    };
    let text = if slot.target.is_some() {
        "●".to_owned()
    } else {
        slot.value.clone()
    };
    view! {
        <g class=class>
            <rect class="viz-mem__cellbox" x=x y=y width=w height=ROW_H></rect>
            <text class="viz-mem__cellv" x=x + w / 2.0 y=y + 15.0 text-anchor="middle">{text}</text>
            <text class="viz-mem__cellidx" x=x + w / 2.0 y=y + ROW_H + 12.0 text-anchor="middle">
                {index.to_string()}
            </text>
        </g>
    }
    .into_any()
}

/// A bezier that lands flat. Out of a row it also leaves flat — the control points sit at the
/// midpoint's x, so the tangent is horizontal at both ends however far it has to fall. Out from
/// under an array cell it leaves heading DOWN, so each of a strip's arrows is seen to come from
/// its own cell rather than all of them running along the strip's bottom edge together.
fn arrow(a: &Arrow) -> AnyView {
    let (c1x, c1y) = if a.from_below {
        (a.x1, a.y1 + (a.y2 - a.y1).abs().mul_add(0.5, 18.0))
    } else {
        (f64::midpoint(a.x1, a.x2), a.y1)
    };
    let c2x = if a.from_below {
        a.x2 - (a.x2 - a.x1).abs().mul_add(0.4, 18.0)
    } else {
        f64::midpoint(a.x1, a.x2)
    };
    let d = format!(
        "M {:.1} {:.1} C {:.1} {:.1}, {:.1} {:.1}, {:.1} {:.1}",
        a.x1, a.y1, c1x, c1y, c2x, a.y2, a.x2, a.y2
    );
    let class = if a.dim {
        "viz-mem__arrow viz-mem__arrow--dim"
    } else {
        "viz-mem__arrow"
    };
    view! {
        <path class=class d=d fill="none" marker-end="url(#viz-arrow)"></path>
    }
    .into_any()
}
