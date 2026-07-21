//! Stage 7 (oracle: `StepNarration.scala`): captions · colours · assemble the `VizGraph`.
//! Caption precedence (top wins): removed → insert/added → changed → cursor-moved →
//! "initial structure" → the raw source line. Colours are assigned ONCE across the whole
//! trace (markers) so a pointer keeps its hue — and LAST, after every diff and caption
//! comparison (which key on (name, target), never colour). `cardCursor` stays cut.

use crate::engine::adapt::diff::DiffedStep;
use crate::engine::adapt::vocab;
use crate::engine::graph::{Annotation, NodeId, VizCursor, VizGraph, VizNode, VizStep};
use crate::engine::markers;

#[must_use]
pub fn finish(
    diffed: &[DiffedStep],
    source: &str,
    layout_hint: &str,
    title: &str,
    truncated: bool,
) -> VizGraph {
    let src_lines: Vec<&str> = source.split('\n').collect();
    let names: Vec<String> = diffed
        .iter()
        .flat_map(|d| d.cursor.iter().map(|c| c.name.clone()))
        .collect();
    let colors = markers::assign_colors(&names);
    let steps: Vec<VizStep> = diffed
        .iter()
        .enumerate()
        .map(|(i, ds)| {
            let body = source_line(ds.line, &src_lines);
            let prev = if i == 0 { None } else { Some(&diffed[i - 1]) };
            VizStep {
                nodes: ds.nodes.clone(),
                edges: ds.edges.clone(),
                cursor: ds
                    .cursor
                    .iter()
                    .map(|c| VizCursor {
                        color: colors.get(&c.name).cloned().unwrap_or_default(),
                        ..c.clone()
                    })
                    .collect(),
                highlight: ds.highlight.clone(),
                changed: ds.changed.clone(),
                removed: ds.removed.clone(),
                annotation: Annotation {
                    eyebrow: eyebrow_of(&ds.event).to_owned(),
                    title: narrate(prev, ds, &body),
                    body: body.clone(),
                    link: None,
                },
                line: ds.line,
                frames: ds.frames.clone(),
                card_cursor: Vec::new(),
                unchanged: ds.unchanged,
                structure_type: ds.structure_type.clone(),
            }
        })
        .collect();
    VizGraph {
        steps,
        layout_hint: layout_hint.to_owned(),
        title: title.to_owned(),
        truncated,
    }
}

fn source_line(line: i32, src_lines: &[&str]) -> String {
    let idx = usize::try_from(line - 1).ok();
    idx.and_then(|i| src_lines.get(i))
        .map(|l| l.trim().to_owned())
        .unwrap_or_default()
}

fn eyebrow_of(event: &str) -> &'static str {
    match event {
        "call" => "call",
        "return" => "return",
        "exception" => "exception",
        _ => "line",
    }
}

// ── the caption, by precedence ──
fn narrate(prev: Option<&DiffedStep>, step: &DiffedStep, src_body: &str) -> String {
    let node_by_id = |id: &NodeId| -> Option<&VizNode> { step.nodes.iter().find(|n| n.id == *id) };
    let lbl = |id: &NodeId| -> String { node_by_id(id).map_or_else(|| "?".to_owned(), |n| n.label.clone()) };
    let key_of = |id: &NodeId| -> Option<String> {
        node_by_id(id).and_then(|n| n.meta.iter().find(|f| f.name == "key").map(|f| f.value.clone()))
    };
    let named = |id: &NodeId| -> String { key_of(id).unwrap_or_else(|| lbl(id)) };

    if !step.removed.is_empty() {
        let list: Vec<String> = step.removed.iter().map(named).collect();
        return format!("removed {}", list.join(", "));
    }
    if !step.highlight.is_empty() {
        let id = step
            .highlight
            .iter()
            .find(|i| lbl(i) != vocab::REF_LABEL)
            .unwrap_or(&step.highlight[0]);
        return match key_of(id) {
            Some(k) => format!("{k} = {}", lbl(id)),
            None => match step.edges.iter().find(|e| e.to == *id && !e.label.is_empty()) {
                Some(e) => format!("inserted {} as {}.{}", lbl(id), lbl(&e.from), e.label),
                None => format!("added {}", lbl(id)),
            },
        };
    }
    if !step.changed.is_empty() {
        let id = &step.changed[0];
        let was = prev
            .and_then(|p| p.nodes.iter().find(|n| n.id == *id))
            .map(|n| n.label.clone());
        return match (key_of(id), was) {
            (Some(k), Some(w)) => format!("{k} changed from {w} to {}", lbl(id)),
            (None, Some(w)) => format!("{w} changed to {}", lbl(id)),
            (_, None) => format!("set {}", lbl(id)),
        };
    }
    let prev_targets: std::collections::HashMap<&str, &NodeId> = prev
        .map(|p| p.cursor.iter().map(|c| (c.name.as_str(), &c.target)).collect())
        .unwrap_or_default();
    let moved: Vec<String> = step
        .cursor
        .iter()
        .filter(|c| prev_targets.get(c.name.as_str()) != Some(&&c.target))
        .map(|c| format!("{} moves to {}", c.name, lbl(&c.target)))
        .collect();
    if !moved.is_empty() {
        moved.join(", ")
    } else if prev.is_none() {
        "initial structure".to_owned()
    } else {
        src_body.to_owned()
    }
}
