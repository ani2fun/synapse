//! The hashmap buckets widget: one row per
//! bucket — the index chip, then the chain of `key: value` pills joined by → connectors.
//! With the `columns` layout hint the buckets stand side by side instead, each chain hanging
//! down from its index to a closing ∅ (`doubly` joins its pills both ways).

use crate::engine::graph::{NodeId, VizGraph, VizStep};
use leptos::prelude::*;

use super::dom;
use crate::engine::shapes::{self, Bucket, BucketEntry, BucketLayout};

#[must_use]
pub fn hashmap(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let graph = graph.clone();
    let layout = shapes::bucket_layout(&graph.layout_hint);
    let class = if layout.columns {
        "viz-dom-widget viz-hashmap viz-hashmap--columns"
    } else {
        "viz-dom-widget viz-hashmap"
    };
    view! {
        <div class=class>
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                frame(step, layout)
            }}
        </div>
    }
    .into_any()
}

type Cursors = std::collections::HashMap<String, Vec<crate::engine::graph::VizCursor>>;

fn frame(step: &VizStep, layout: BucketLayout) -> AnyView {
    let buckets = shapes::buckets(step);
    if buckets.is_empty() {
        return view! {
            <div class="viz-hashmap__empty">{dom::null_glyph()}<span>" empty"</span></div>
        }
        .into_any();
    }
    let cursors = dom::cursors_by_target(step);
    let rows: Vec<_> = buckets
        .iter()
        .map(|b| {
            if layout.columns {
                column(b, step, &cursors, layout.doubly)
            } else {
                row(b, step, &cursors)
            }
        })
        .collect();
    rows.into_any()
}

fn row(b: &Bucket, step: &VizStep, cursors: &Cursors) -> AnyView {
    let index_class = dom::diff_mods("viz-hashmap__index", &b.entry_id, step);
    let chain: Vec<AnyView> = if b.entries.is_empty() {
        vec![dom::null_glyph()]
    } else {
        b.entries
            .iter()
            .enumerate()
            .flat_map(|(i, e)| {
                let mut parts = Vec::new();
                if i > 0 {
                    parts.push(dom::arrow_glyph());
                }
                parts.push(pill(e, step, cursors));
                parts
            })
            .collect()
    };
    let index = b.index.clone();
    view! {
        <div class="viz-hashmap__bucket">
            <span class=index_class>{index}</span>
            <div class="viz-hashmap__chain">{chain}</div>
        </div>
    }
    .into_any()
}

/// One column: the index chip, then link, pill, link, …, ∅ down the page.
fn column(b: &Bucket, step: &VizStep, cursors: &Cursors, doubly: bool) -> AnyView {
    let index_class = dom::diff_mods("viz-hashmap__index", &b.entry_id, step);
    let mut parts: Vec<AnyView> = Vec::new();
    for (i, link) in shapes::column_links(b.entries.len(), doubly)
        .into_iter()
        .enumerate()
    {
        parts.push(view! { <span class="viz-dom-connector">{link}</span> }.into_any());
        parts.push(match b.entries.get(i) {
            Some(e) => pill(e, step, cursors),
            None => null_end(b, step, cursors),
        });
    }
    let index = b.index.clone();
    view! {
        <div class="viz-hashmap__bucket">
            <span class=index_class>{index}</span>
            <div class="viz-hashmap__chain">{parts}</div>
        </div>
    }
    .into_any()
}

/// The ∅ closing a column chain. It answers to the id `null:<bucket entry id>`, so a step can
/// point a cursor at the end of the chain (a search that walked off it) or cue it like a pill.
fn null_end(b: &Bucket, step: &VizStep, cursors: &Cursors) -> AnyView {
    let id = NodeId::new(format!("null:{}", b.entry_id.value()));
    let on_it = cursors.get(id.value());
    let mut class = dom::diff_mods("viz-hashmap__null", &id, step);
    if on_it.is_some() {
        class.push_str(" viz-hashmap__null--cursor");
    }
    let badge = on_it.map(|cs| dom::cursor_badge(cs));
    view! {
        <div class=class>
            {badge}
            {dom::null_glyph()}
        </div>
    }
    .into_any()
}

fn pill(e: &BucketEntry, step: &VizStep, cursors: &Cursors) -> AnyView {
    let on_it = cursors.get(e.id.value());
    let mut class = dom::diff_mods("viz-hashmap__entry", &e.id, step);
    if on_it.is_some() {
        class.push_str(" viz-hashmap__entry--cursor");
    }
    let badge = on_it.map(|cs| dom::cursor_badge(cs));
    let key = e.key.clone();
    let value = e.value.clone();
    view! {
        <div class=class>
            {badge}
            {key.clone().map(|k| view! { <span class="viz-hashmap__entry-key">{k}</span> })}
            {key.map(|_| view! { <span class="viz-hashmap__entry-sep">":"</span> })}
            <span class="viz-hashmap__entry-value">{value}</span>
        </div>
    }
    .into_any()
}
