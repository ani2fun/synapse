//! The dual views (oracle: `DualViewRenderers`, step 33): the derived tree above, the raw
//! backing array below, both driven by the SAME step signal — identical node ids make the
//! diff cues light up in both panes at once.

use crate::viz::engine::graph::VizGraph;
use leptos::prelude::*;

use crate::viz::engine::shapes;
use crate::viz::render::{cells, tree};

/// Heap: the slot-tree (`i → 2i+1 · 2i+2`) over the backing array.
#[must_use]
pub fn heap(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    dual(
        tree::tree(&shapes::heap_tree(graph), step_index),
        "as a tree",
        cells::array(graph, step_index),
        "the backing array",
    )
}

/// Union-find: the parent forest (labels = element indices, roots badged) over `parent[i]`.
#[must_use]
pub fn union_find(graph: &VizGraph, step_index: Signal<usize>) -> AnyView {
    dual(
        tree::tree(&shapes::forest_graph(graph), step_index),
        "the forest (labels are element indices; roots badged)",
        cells::array(graph, step_index),
        "parent[i] — the array that encodes it",
    )
}

fn dual(top: AnyView, top_cap: &str, bottom: AnyView, bottom_cap: &str) -> AnyView {
    let top_cap = top_cap.to_owned();
    let bottom_cap = bottom_cap.to_owned();
    view! {
        <div class="viz-dom-widget viz-dual">
            <div class="viz-dual__pane">{top}<div class="viz-dual__caption">{top_cap}</div></div>
            <div class="viz-dual__pane">{bottom}<div class="viz-dual__caption">{bottom_cap}</div></div>
        </div>
    }
    .into_any()
}
