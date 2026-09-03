//! `VizGraph` → **d2 source**: the picture on the canvas, as a document you can keep.
//!
//! The canvas is drawn by this crate's own renderers, and it has to be: layout is computed once
//! across the union of every step, which is what stops a node moving while you scrub. d2 lays out
//! per board and would teleport them. So this is an EXPORT, not a second renderer — the figure you
//! stopped on, in a form `/d2` can edit and a lesson can carry.
//!
//! Two shapes come out of it: one board for one step, or a `layers` walkthrough over the steps
//! that CHANGED something. The reader clicks through the second deliberately, which is the one
//! reading where per-board layout is not a defect.
//!
//! It derives nothing of its own. Every family's structure comes from `shapes` — the same helpers
//! the renderers use — so the export tells the story the canvas told, and a fix in one is a fix in
//! both.
//!
//! **The `near:` trap.** A cursor is an ARROW into its target, never `near: <cell>`: d2 refuses
//! that outright inside a grid ("near keys cannot be set to descendants of special objects"), and
//! grids are how the cell families are drawn here. Arrows work everywhere, including into a grid
//! cell, and several into one cell all render.
//!
//! Colours are literal hex, not tokens: a d2 figure renders fixed-light on both themes, so it
//! carries its own palette rather than borrowing the page's.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::engine::graph::{NodeId, VizGraph, VizNode, VizStep};
use crate::engine::render_family::RenderFamily;
use crate::engine::shapes;
use crate::engine::vocabulary::VizStructure;

/// The most boards a walkthrough export writes. A walkthrough is a lesson figure, not a movie:
/// past a dozen boards nobody clicks to the end, and a 600-step trace would write 600 of them.
pub const MAX_BOARDS: usize = 12;

// ── the fixed-light palette (the app's light tokens, resolved) ──
const INK: &str = "#2a303c";
const RULE: &str = "#e5dfd2";
const MUTED: &str = "#6a6f7c";
const NEW_FILL: &str = "#e1f4eb";
const NEW_STROKE: &str = "#307e57";
const CHANGED_STROKE: &str = "#0c7d69";
const CURSOR: &str = "#0c7d69";

// ─────────────────────────────────────────────────────────────────────────────
// THE TWO SHAPES
// ─────────────────────────────────────────────────────────────────────────────

/// One step, as a standalone d2 document.
#[must_use]
pub fn step_source(graph: &VizGraph, structure: VizStructure, step_index: usize) -> String {
    let Some(step) = graph.steps.get(step_index) else {
        return String::new();
    };
    let mut out = String::new();
    write_caption(&mut out, &step.annotation.body);
    write_board(&mut out, graph, structure, step, step_index);
    out
}

/// The steps that CHANGED something, as a `layers` walkthrough — one board each, capped at
/// [`MAX_BOARDS`]. Falls back to every step when nothing is marked unchanged (a short trace often
/// changes at each line), and always keeps at least the first.
#[must_use]
pub fn walkthrough_source(graph: &VizGraph, structure: VizStructure) -> String {
    let stops = board_stops(graph);
    let mut out = String::new();
    let _ = writeln!(out, "title: {} walkthrough", structure.token());
    let _ = writeln!(out, "direction: right");
    out.push_str("\nlayers: {\n");
    for (n, &i) in stops.iter().enumerate() {
        let Some(step) = graph.steps.get(i) else { continue };
        let _ = writeln!(out, "  step-{}: {{", n + 1);
        let mut board = String::new();
        write_caption(&mut board, &step.annotation.body);
        write_board(&mut board, graph, structure, step, i);
        for line in board.lines() {
            if line.is_empty() {
                out.push('\n');
            } else {
                let _ = writeln!(out, "    {line}");
            }
        }
        out.push_str("  }\n");
    }
    out.push_str("}\n");
    out
}

/// Which steps become boards: the ones that changed the structure, first one always included.
fn board_stops(graph: &VizGraph) -> Vec<usize> {
    let changed: Vec<usize> = graph
        .steps
        .iter()
        .enumerate()
        .filter(|(i, s)| *i == 0 || !s.unchanged)
        .map(|(i, _)| i)
        .collect();
    let stops = if changed.len() > 1 {
        changed
    } else {
        (0..graph.steps.len()).collect()
    };
    stops.into_iter().take(MAX_BOARDS).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// ONE BOARD
// ─────────────────────────────────────────────────────────────────────────────

fn write_board(
    out: &mut String,
    graph: &VizGraph,
    structure: VizStructure,
    step: &VizStep,
    step_index: usize,
) {
    // Each family answers with the keys a CURSOR can point at, because only it knows what it
    // called them — `arr.c3` inside a grid, `n2` in a walk order that is not the wire order.
    let targets = match RenderFamily::of(structure) {
        RenderFamily::Cells => cells_board(out, graph, step, "arr", false),
        RenderFamily::Queue => cells_board(out, graph, step, "queue", false),
        RenderFamily::Stack => cells_board(out, graph, step, "stack", true),
        RenderFamily::Grid => grid_board(out, step),
        RenderFamily::Buckets => buckets_board(out, step),
        RenderFamily::LinkedList => chain_board(out, step),
        RenderFamily::Forest => {
            let forest = shapes::forest_graph(graph);
            match forest.steps.get(step_index) {
                Some(projected) => nodes_board(out, projected, "up"),
                None => nodes_board(out, step, "up"),
            }
        }
        // Heap keeps its DUAL view — the backing array and the tree it means, in one board. The
        // cursors stay on the ARRAY: that is where an index means something.
        RenderFamily::HeapDual => {
            let cells = cells_board(out, graph, step, "heap", false);
            let tree = shapes::heap_tree(graph);
            if let Some(projected) = tree.steps.get(step_index) {
                out.push('\n');
                let _ = nodes_board(out, projected, "down");
            }
            cells
        }
        RenderFamily::Tree | RenderFamily::Trie => nodes_board(out, step, "down"),
        RenderFamily::Chain | RenderFamily::Force => nodes_board(out, step, "right"),
    };
    cursors(out, step, &targets);
}

/// The cell families: one grid container, values then the index rail beneath them. Two rows of N
/// declared as 2N children of a `grid-rows: 2` container — d2 fills row-major in declaration
/// order, so the indices land column-aligned under their values.
fn cells_board(
    out: &mut String,
    graph: &VizGraph,
    step: &VizStep,
    container: &str,
    vertical: bool,
) -> HashMap<NodeId, String> {
    let cells = ordered_cells(step);
    if cells.is_empty() {
        let _ = writeln!(out, "{container}: \"(empty)\" {{ style.stroke: \"{RULE}\" }}");
        return HashMap::new();
    }
    // The KEY is the family's, short and stable, because it is what every cursor arrow and every
    // hand edit has to type; the graph's own title — a sentence, usually — is the label.
    if graph.title.is_empty() {
        let _ = writeln!(out, "{container}: {{");
    } else {
        let _ = writeln!(out, "{container}: {} {{", quote(&graph.title));
    }
    if vertical {
        // A stack reads top-down, so one column — and the index rail would double its width for
        // no gain, since a stack is addressed by its top rather than by position.
        let _ = writeln!(out, "  grid-columns: 1");
    } else {
        let _ = writeln!(out, "  grid-rows: 2");
    }
    let _ = writeln!(out, "  grid-gap: 0");
    for (slot, node) in &cells {
        let _ = write!(out, "  c{slot}: {}", quote(&node.label));
        diff_style(out, &node.id, step);
        out.push('\n');
    }
    if !vertical {
        for (slot, _) in &cells {
            let _ = writeln!(
                out,
                "  i{slot}: {slot} {{ style.stroke-width: 0; style.fill: transparent; \
                 style.font-color: \"{MUTED}\"; style.font-size: 13 }}"
            );
        }
    }
    out.push_str("}\n");
    cells
        .into_iter()
        .map(|(slot, node)| (node.id, format!("{container}.c{slot}")))
        .collect()
}

/// The 2-D grid: `grid-columns` over the SAME cell derivation the table renderer uses, so a
/// ragged row's holes stay holes rather than closing up.
fn grid_board(out: &mut String, step: &VizStep) -> HashMap<NodeId, String> {
    let rows = shapes::grid_cells(step);
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols == 0 {
        let _ = writeln!(out, "grid: \"(empty)\" {{ style.stroke: \"{RULE}\" }}");
        return HashMap::new();
    }
    let mut keys = HashMap::new();
    let _ = writeln!(out, "grid: {{");
    let _ = writeln!(out, "  grid-columns: {cols}");
    let _ = writeln!(out, "  grid-gap: 0");
    for (r, row) in rows.iter().enumerate() {
        for c in 0..cols {
            match row.get(c).and_then(Option::as_ref) {
                // A hole is a cell with no label — the grid keeps its shape, and the gap reads
                // as a gap rather than as a shorter row.
                None => {
                    let _ = writeln!(
                        out,
                        "  c{r}_{c}: \"\" {{ style.stroke-dash: 3; style.stroke: \"{RULE}\" }}"
                    );
                }
                Some(node) => {
                    let _ = write!(out, "  c{r}_{c}: {}", quote(&node.label));
                    diff_style(out, &node.id, step);
                    out.push('\n');
                    keys.insert(node.id.clone(), format!("grid.c{r}_{c}"));
                }
            }
        }
    }
    out.push_str("}\n");
    keys
}

/// The hash map: one container per bucket, its chain inside.
fn buckets_board(out: &mut String, step: &VizStep) -> HashMap<NodeId, String> {
    let buckets = shapes::buckets(step);
    if buckets.is_empty() {
        let _ = writeln!(out, "map: \"(empty)\" {{ style.stroke: \"{RULE}\" }}");
        return HashMap::new();
    }
    let mut keys = HashMap::new();
    let _ = writeln!(out, "map: {{");
    let _ = writeln!(out, "  grid-columns: 1");
    for (b, bucket) in buckets.iter().enumerate() {
        let _ = writeln!(out, "  b{b}: {} {{", quote(&bucket.index));
        let _ = writeln!(out, "    grid-rows: 1");
        for (e, entry) in bucket.entries.iter().enumerate() {
            let label = match &entry.key {
                Some(key) => format!("{key}: {}", entry.value),
                None => entry.value.clone(),
            };
            let _ = write!(out, "    e{e}: {}", quote(&label));
            diff_style(out, &entry.id, step);
            out.push('\n');
            keys.insert(entry.id.clone(), format!("map.b{b}.e{e}"));
        }
        out.push_str("  }\n");
    }
    out.push_str("}\n");
    keys
}

/// The linked list: the walk order the chain renderer uses, `next` forward and `prev` back when
/// the list is doubly linked, ending at a real `null` so the tail is visibly the tail.
fn chain_board(out: &mut String, step: &VizStep) -> HashMap<NodeId, String> {
    let info = shapes::chain(step);
    if info.nodes.is_empty() {
        let _ = writeln!(out, "null: null {{ shape: text; style.font-color: \"{MUTED}\" }}");
        return HashMap::new();
    }
    let _ = writeln!(out, "direction: right");
    for (i, node) in info.nodes.iter().enumerate() {
        let _ = write!(out, "n{i}: {}", quote(&node.label));
        diff_style(out, &node.id, step);
        out.push('\n');
    }
    let _ = writeln!(out, "null: null {{ shape: text; style.font-color: \"{MUTED}\" }}");
    for i in 0..info.nodes.len() {
        let to = if i + 1 == info.nodes.len() {
            "null".to_owned()
        } else {
            format!("n{}", i + 1)
        };
        let _ = writeln!(out, "n{i} -> {to}: next");
        if info.is_doubly && i > 0 {
            let _ = writeln!(out, "n{i} -> n{}: prev", i - 1);
        }
    }
    info.nodes
        .into_iter()
        .enumerate()
        .map(|(i, node)| (node.id, format!("n{i}")))
        .collect()
}

/// The node/edge families: every node, every edge, labelled as the trace labelled them.
fn nodes_board(out: &mut String, step: &VizStep, direction: &str) -> HashMap<NodeId, String> {
    if step.nodes.is_empty() {
        let _ = writeln!(
            out,
            "empty: \"(empty)\" {{ shape: text; style.font-color: \"{MUTED}\" }}"
        );
        return HashMap::new();
    }
    let _ = writeln!(out, "direction: {direction}");
    let keys = node_keys(step);
    for node in &step.nodes {
        let Some(key) = keys.get(&node.id) else { continue };
        let _ = write!(out, "{key}: {}", quote(&node.label));
        diff_style(out, &node.id, step);
        out.push('\n');
    }
    for edge in &step.edges {
        let (Some(from), Some(to)) = (keys.get(&edge.from), keys.get(&edge.to)) else {
            continue;
        };
        if edge.label.is_empty() {
            let _ = writeln!(out, "{from} -> {to}");
        } else {
            let _ = writeln!(out, "{from} -> {to}: {}", quote(&edge.label));
        }
    }
    keys
}

// ─────────────────────────────────────────────────────────────────────────────
// SHARED PARTS
// ─────────────────────────────────────────────────────────────────────────────

/// The step's narration, as the board's own line of prose. `near: top-center` is a constant
/// position, so it is legal even on a board whose figure is a grid.
fn write_caption(out: &mut String, body: &str) {
    if body.trim().is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "caption: {} {{ shape: text; near: top-center; style.font-color: \"{INK}\"; \
         style.font-size: 15 }}\n",
        quote(body)
    );
}

/// One `shape: text` node per pointer, with an arrow to what it points at. Several arrows into
/// one cell is fine — which is what a two-pointer walk needs when `left` and `right` meet.
fn cursors(out: &mut String, step: &VizStep, targets: &HashMap<NodeId, String>) {
    for (i, cursor) in step.cursor.iter().enumerate() {
        let Some(target) = targets.get(&cursor.target) else {
            continue;
        };
        let name = if cursor.name.is_empty() {
            "\u{2022}"
        } else {
            &cursor.name
        };
        let _ = writeln!(
            out,
            "p{i}: {} {{ shape: text; style.font-color: \"{CURSOR}\"; style.bold: true }}",
            quote(name)
        );
        let _ = writeln!(out, "p{i} -> {target}: {{ style.stroke: \"{CURSOR}\" }}");
    }
}

/// Cells in draw order, paired with the slot that names them.
fn ordered_cells(step: &VizStep) -> Vec<(i32, VizNode)> {
    let mut cells: Vec<&VizNode> = step.nodes.iter().filter(|n| n.kind == "cell").collect();
    cells.sort_by_key(|n| n.slot.unwrap_or(i32::MAX));
    cells
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let slot = n.slot.unwrap_or(i as i32);
            (slot, n.clone())
        })
        .collect()
}

/// Stable d2 keys for a step's nodes: positional, not derived from the id, because a trace id
/// (`obj:140234…`) sanitises into something both ugly and collision-prone.
fn node_keys(step: &VizStep) -> HashMap<NodeId, String> {
    step.nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), format!("n{i}")))
        .collect()
}

/// The diff cue as an inline style block, or nothing when the node is unremarkable this step.
fn diff_style(out: &mut String, id: &NodeId, step: &VizStep) {
    if step.removed.contains(id) {
        let _ = write!(out, " {{ style.stroke-dash: 3; style.opacity: 0.45 }}");
    } else if step.changed.contains(id) {
        let _ = write!(
            out,
            " {{ style.stroke: \"{CHANGED_STROKE}\"; style.stroke-width: 3 }}"
        );
    } else if step.highlight.contains(id) {
        let _ = write!(
            out,
            " {{ style.fill: \"{NEW_FILL}\"; style.stroke: \"{NEW_STROKE}\" }}"
        );
    }
}

/// A quoted d2 label. Newlines flatten to spaces — a multi-line label is legal but pins the
/// whole board's row height to the longest one.
fn quote(label: &str) -> String {
    let flat: String = label
        .replace(['\n', '\r'], " ")
        .replace('\\', r"\\")
        .replace('"', "\\\"");
    format!("\"{flat}\"")
}

#[cfg(test)]
mod tests;
