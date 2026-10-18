//! The placed linked list (`layoutHint: "placed"`): node boxes where the author put them,
//! each with a value and a pointer field (`next`, and `prev` when doubly), every arrow drawn
//! from the field to where the pointer really points, `∅` for null. Geometry is
//! `engine::geometry::placed`; this only paints a frame.

use crate::engine::geometry::constants::{LIST_BOX_H, LIST_FIELD_W};
use crate::engine::geometry::placed;
use crate::engine::graph::VizGraph;
use leptos::prelude::*;

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{cursor_stack, diff_class, fitted_text, top_margin};

/// Each list draws its own arrowhead: a shared `#viz-arrow` resolves to the FIRST one in the
/// document, and when that widget is hidden (a collapsed section, an inactive tab) every other
/// widget's arrows lose their heads.
static MARKERS: AtomicUsize = AtomicUsize::new(0);

#[must_use]
pub fn list(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let placement = placed::placement(graph);
    let hh = LIST_BOX_H / 2.0;
    let margin = top_margin(graph, &placement.layout(), hh + 8.0);
    let view_box = format!("0 {} {} {}", -margin, placement.width, placement.height + margin);
    let (width, height) = (placement.width, placement.height + margin);
    let graph = graph.clone();
    let marker = format!("viz-plist-arrow-{}", MARKERS.fetch_add(1, Ordering::Relaxed));
    let marker_url = format!("url(#{marker})");
    view! {
        <svg class="viz-svg viz-plist" viewBox=view_box width=width height=height>
            <defs>
                <marker id=marker viewBox="0 0 10 10" refX="9" refY="5"
                        markerWidth="7" markerHeight="7" orient="auto-start-reverse">
                    <path class="viz-arrowhead" d="M0,0 L10,5 L0,10 z"></path>
                </marker>
            </defs>
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                let f = placement.frame(step);
                let hw = placement.box_w() / 2.0;
                let doubly = placement.doubly;
                let boxes: Vec<_> = f
                    .boxes
                    .iter()
                    .map(|b| {
                        let p = b.at;
                        let class = format!("{} viz-plist__node", diff_class(step, &b.node.id, "viz-node"));
                        let val_lo = if doubly { p.x - hw + LIST_FIELD_W } else { p.x - hw };
                        let val_hi = p.x + hw - LIST_FIELD_W;
                        view! {
                            <g class=class>
                                <rect x=p.x - hw y=p.y - hh width=hw * 2.0 height=LIST_BOX_H rx="4"></rect>
                                {doubly.then(|| view! {
                                    <line class="viz-plist__divider" x1=val_lo y1=p.y - hh x2=val_lo y2=p.y + hh></line>
                                    <circle class="viz-plist__dot" cx=p.x - hw + LIST_FIELD_W / 2.0 cy=p.y + 7.0 r="2.5"></circle>
                                })}
                                <line class="viz-plist__divider" x1=val_hi y1=p.y - hh x2=val_hi y2=p.y + hh></line>
                                <circle class="viz-plist__dot" cx=p.x + hw - LIST_FIELD_W / 2.0
                                        cy=if doubly { p.y - 7.0 } else { p.y } r="2.5"></circle>
                                {fitted_text(&b.node.label, f64::midpoint(val_lo, val_hi), p.y, val_hi - val_lo - 6.0, 8.7)}
                            </g>
                        }
                    })
                    .collect();
                let arrows: Vec<_> = f
                    .arrows
                    .iter()
                    .map(|a| {
                        let class = if a.prev { "viz-edge viz-edge--prev" } else { "viz-edge viz-edge--next" };
                        view! { <path class=class d=a.d.clone() fill="none" marker-end=marker_url.clone()></path> }
                    })
                    .collect();
                let nulls: Vec<_> = f
                    .nulls
                    .iter()
                    .map(|g| view! {
                        <text class="viz-plist__null" x=g.x y=g.y text-anchor="middle" dominant-baseline="central">"∅"</text>
                    })
                    .collect();
                let cursors: Vec<_> = f.cursors.iter().map(|c| cursor_stack(&c.cursors, c.at, c.rise)).collect();
                let regions: Vec<_> = f
                    .regions
                    .iter()
                    .map(|r| {
                        let lines: Vec<_> = r
                            .lines
                            .iter()
                            .map(|(a, b)| view! {
                                <line class="viz-plist__cell" x1=a.x y1=a.y x2=b.x y2=b.y></line>
                            })
                            .collect();
                        let label = (!r.label.is_empty()).then(|| view! {
                            <text class="viz-plist__region-label" x=r.x + r.w / 2.0
                                  y=r.y + r.h + placed::REGION_LABEL_DY text-anchor="middle">{r.label.clone()}</text>
                        });
                        view! {
                            <g class="viz-plist__region">
                                <rect x=r.x y=r.y width=r.w height=r.h rx="6"></rect>
                                {lines}
                                {label}
                            </g>
                        }
                    })
                    .collect();
                view! {
                    <g class="viz-plist__regions">{regions}</g>
                    <g class="viz-plist__edges">{arrows}</g>
                    <g class="viz-plist__nodes">{boxes}</g>
                    <g class="viz-plist__nulls">{nulls}</g>
                    <g class="viz-plist__cursors">{cursors}</g>
                }
            }}
        </svg>
    }
    .into_any()
}
