//! The CORTEX PARITY gate (oracle: `CortexGoldenSpec` + `VizParity`): the 16 goldens are
//! Cortex's own finished `HeapToGraph` output; the paired inputs are the oracle's hand-built
//! traces (exported verbatim as JSON). Each fixture adapts through the REAL Rust pipeline and
//! must match its golden after normalisation — `VizParity.normalize` erases exactly the three
//! deliberate-delta fields (`structureType` → None, `cardCursor` → [], `unchanged` → false),
//! each citing an ADR-S030 delta row; ANY other difference fails.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde::Deserialize;
use synapse_shared::viz::adapt;
use synapse_shared::viz::graph::{VizCases, VizGraph, VizStep};
use synapse_shared::viz::trace::HeapTrace;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    name: String,
    title: String,
    source: String,
    root_hint: Option<String>,
    layout_hint: String,
    trace: HeapTrace,
}

fn fixtures() -> Vec<Fixture> {
    serde_json::from_str(include_str!("fixtures/cortex-fixture-inputs.json")).unwrap()
}

fn golden(name: &str) -> VizCases {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cortex-goldens");
    let json = std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap();
    serde_json::from_str(&json).unwrap()
}

/// The three deliberate deltas, erased on BOTH sides before compare (ADR-S030):
/// `structureType` (chrome inference deleted) · `cardCursor` (`ArrowLayer` cut) ·
/// `unchanged` (gained edges — delta #8).
fn normalize(cases: &VizCases) -> VizCases {
    VizCases {
        cases: cases
            .cases
            .iter()
            .map(|g| VizGraph {
                steps: g
                    .steps
                    .iter()
                    .map(|s| VizStep {
                        structure_type: None,
                        card_cursor: Vec::new(),
                        unchanged: false,
                        ..s.clone()
                    })
                    .collect(),
                ..g.clone()
            })
            .collect(),
    }
}

fn canonical(cases: &VizCases) -> String {
    serde_json::to_string(&normalize(cases)).unwrap()
}

#[test]
fn all_sixteen_goldens_match() {
    let fixtures = fixtures();
    assert_eq!(fixtures.len(), 16, "the full cortex fixture set");
    let mut failures = Vec::new();
    for f in &fixtures {
        let actual = adapt::adapt(
            &f.trace,
            &f.source,
            &f.layout_hint,
            f.root_hint.as_deref(),
            None,
            &f.title,
        )
        .unwrap_or_else(|e| panic!("{}: adapt failed: {e}", f.name));
        let want = golden(&f.name);
        if canonical(&actual) != canonical(&want) {
            failures.push(describe_mismatch(&f.name, &actual, &want));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

fn describe_mismatch(name: &str, actual: &VizCases, want: &VizCases) -> String {
    let a = normalize(actual);
    let w = normalize(want);
    if a.cases.len() != w.cases.len() {
        return format!("{name}: case count {} != {}", a.cases.len(), w.cases.len());
    }
    for (ci, (ac, wc)) in a.cases.iter().zip(&w.cases).enumerate() {
        if ac.steps.len() != wc.steps.len() {
            return format!(
                "{name}: case {ci} step count {} != {}",
                ac.steps.len(),
                wc.steps.len()
            );
        }
        for (si, (as_, ws)) in ac.steps.iter().zip(&wc.steps).enumerate() {
            if as_ != ws {
                let field = if as_.nodes != ws.nodes {
                    "nodes"
                } else if as_.edges != ws.edges {
                    "edges"
                } else if as_.cursor != ws.cursor {
                    "cursor"
                } else if as_.annotation != ws.annotation {
                    "annotation"
                } else if as_.frames != ws.frames {
                    "frames"
                } else if as_.highlight != ws.highlight
                    || as_.changed != ws.changed
                    || as_.removed != ws.removed
                {
                    "diff cues"
                } else {
                    "line/other"
                };
                return format!(
                    "{name}: case {ci} step {si} differs on {field}\n  actual: {}\n  want:   {}",
                    serde_json::to_string(as_).unwrap(),
                    serde_json::to_string(ws).unwrap()
                );
            }
        }
        if (ac.layout_hint.as_str(), ac.title.as_str(), ac.truncated)
            != (wc.layout_hint.as_str(), wc.title.as_str(), wc.truncated)
        {
            return format!("{name}: case {ci} graph attrs differ");
        }
    }
    format!("{name}: differs (unlocated)")
}
