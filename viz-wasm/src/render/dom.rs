//! Shared bits for the HTML flow-layout families (oracle: `DomKit.scala`, step 33): the
//! per-widget diff modifier classes, the stacked cursor badge, and the ∅ / → glyphs. These
//! widgets size to content and wrap — no SVG canvas, no layout pass.

use crate::engine::graph::{NodeId, VizCursor, VizStep};
use leptos::prelude::*;
use std::collections::HashMap;

use super::themed;

/// The BEM modifiers for a node id in a step, appended to `block` (each cue independently,
/// oracle `diffMods`).
#[must_use]
pub fn diff_mods(block: &str, id: &NodeId, step: &VizStep) -> String {
    use std::fmt::Write;
    let cues = [
        (&step.highlight, "new"),
        (&step.changed, "changed"),
        (&step.removed, "removed"),
    ];
    let mut classes = block.to_owned();
    for (_, cue) in cues.iter().filter(|(ids, _)| ids.contains(id)) {
        let _ = write!(classes, " {block}--{cue}");
    }
    classes
}

/// Cursors grouped by target — the flow widgets look their node's badge up per id.
#[must_use]
pub fn cursors_by_target(step: &VizStep) -> HashMap<String, Vec<VizCursor>> {
    let mut m: HashMap<String, Vec<VizCursor>> = HashMap::new();
    for c in &step.cursor {
        m.entry(c.target.value().to_owned()).or_default().push(c.clone());
    }
    m
}

/// The floating pointer badge above a node: stacked names, one shared ▾ caret.
#[must_use]
pub fn cursor_badge(cursors: &[VizCursor]) -> AnyView {
    let caret_color = themed(&cursors.first().map(|c| c.color.clone()).unwrap_or_default());
    let names: Vec<_> = cursors
        .iter()
        .map(|c| {
            let color = themed(&c.color);
            let name = c.name.clone();
            view! { <span class="viz-dom-cursor__name" style=format!("color: {color}")>{name}</span> }
        })
        .collect();
    view! {
        <div class="viz-dom-cursor">
            {names}
            <span class="viz-dom-cursor__caret" style=format!("color: {caret_color}")>"▾"</span>
        </div>
    }
    .into_any()
}

#[must_use]
pub fn null_glyph() -> AnyView {
    view! { <span class="viz-dom-null">"∅"</span> }.into_any()
}

#[must_use]
pub fn arrow_glyph() -> AnyView {
    view! { <span class="viz-dom-connector">"→"</span> }.into_any()
}
