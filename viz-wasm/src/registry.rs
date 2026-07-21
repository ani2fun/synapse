//! Family → renderer dispatch (oracle: `RendererRegistry`). The DECISION is the shared pure
//! `RenderFamily::of`; this maps a family to a concrete Leptos renderer — the SVG geometry
//! families plus the step-33 bespoke HTML gallery. Two structure-level quirks live here:
//! callstack keeps the SVG frame boxes (the Stack strip is for data stacks), and deque flips
//! the queue strip's vocabulary.

use crate::engine::geometry::{self, graph_layout, tree as tree_layout};
use crate::engine::graph::VizCases;
use crate::engine::render_family::RenderFamily;
use crate::engine::vocabulary::VizStructure;
use leptos::prelude::*;

use crate::render::{buckets, cells, chain, dual, graph_canvas, grid_table, list_chain, strip, tree};

/// `None` when no case has steps OR the family has no renderer yet.
#[must_use]
pub fn render(structure: VizStructure, cases: &VizCases, step_index: Signal<usize>) -> Option<AnyView> {
    let graph = cases.cases.iter().find(|g| !g.steps.is_empty())?;
    match RenderFamily::of(structure) {
        RenderFamily::Cells => Some(cells::array(graph, step_index)),
        RenderFamily::Stack if structure == VizStructure::Callstack => Some(cells::stack(graph, step_index)),
        RenderFamily::Stack => Some(strip::stack(graph, step_index)),
        RenderFamily::Tree => Some(tree::tree(graph, step_index)),
        RenderFamily::Chain => Some(chain::chain(graph, step_index)),
        RenderFamily::Force => {
            let (un, ue) = geometry::union(graph);
            Some(graph_canvas::canvas(
                graph,
                graph_layout::graph(&un, &ue),
                step_index,
            ))
        }
        RenderFamily::Trie => {
            let (un, ue) = geometry::union(graph);
            Some(graph_canvas::canvas(
                graph,
                tree_layout::tree(&un, &ue),
                step_index,
            ))
        }
        // The step-33 bespoke HTML gallery.
        RenderFamily::Grid => Some(grid_table::table(graph, step_index)),
        RenderFamily::Buckets => Some(buckets::hashmap(graph, step_index)),
        RenderFamily::Queue => Some(strip::queue(graph, step_index, structure == VizStructure::Deque)),
        RenderFamily::LinkedList => Some(list_chain::list(graph, step_index)),
        RenderFamily::Forest => Some(dual::union_find(graph, step_index)),
        RenderFamily::HeapDual => Some(dual::heap(graph, step_index)),
    }
}
