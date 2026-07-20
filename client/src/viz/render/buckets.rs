//! The hashmap buckets widget (oracle: `BucketRenderers.hashmap`, step 33): one row per
//! bucket — the index chip, then the chain of `key: value` pills joined by → connectors.

use crate::viz::engine::graph::{VizGraph, VizStep};
use leptos::prelude::*;

use super::dom;
use crate::viz::engine::shapes::{self, BucketEntry};

#[must_use]
pub fn hashmap(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    let graph = graph.clone();
    view! {
        <div class="viz-dom-widget viz-hashmap">
            {move || {
                let step = &graph.steps[step_index.get().min(graph.steps.len() - 1)];
                frame(step)
            }}
        </div>
    }
    .into_any()
}

fn frame(step: &VizStep) -> AnyView {
    let buckets = shapes::buckets(step);
    if buckets.is_empty() {
        return view! {
            <div class="viz-hashmap__empty">{dom::null_glyph()}<span>" empty"</span></div>
        }
        .into_any();
    }
    let cursors = dom::cursors_by_target(step);
    let rows: Vec<_> = buckets
        .into_iter()
        .map(|b| {
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
                        parts.push(pill(e, step, &cursors));
                        parts
                    })
                    .collect()
            };
            view! {
                <div class="viz-hashmap__bucket">
                    <span class=index_class>{b.index}</span>
                    <div class="viz-hashmap__chain">{chain}</div>
                </div>
            }
        })
        .collect();
    rows.into_any()
}

fn pill(
    e: &BucketEntry,
    step: &VizStep,
    cursors: &std::collections::HashMap<String, Vec<crate::viz::engine::graph::VizCursor>>,
) -> AnyView {
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
