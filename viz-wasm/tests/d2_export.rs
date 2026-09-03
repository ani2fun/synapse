//! The d2 EXPORT goldens: every structure in the vocabulary, emitted from a REAL adapted trace
//! and pinned byte-exact.
//!
//! The corpus is the cortex goldens — sixteen finished `VizCases`, one per family — so these
//! exercise the shapes a real run produces rather than fixtures written to suit the emitter.
//! What they cannot tell you is whether the text is valid d2; that is
//! `dev-tools/d2-viz-exports-compile.mjs`, which compiles these same files through the engine
//! that will draw them.
//!
//! Regenerate deliberately: `UPDATE_D2_EXPORT=1 cargo test -p viz-wasm --test d2_export`, then
//! READ the diff. A golden that changed because the picture improved is a good commit; one that
//! changed because a label lost its quoting is the bug this file exists to catch.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use viz_wasm::engine::d2;
use viz_wasm::engine::graph::VizCases;
use viz_wasm::engine::vocabulary::VizStructure;

/// The golden name → the structure a lesson would declare for it. Spelled out rather than read
/// off the fixture's `layoutHint`, because that field carries legacy layout-kind names
/// (`tree-binary`, and empty for most) while `viz=` takes the vocabulary's own token.
const CORPUS: [(&str, VizStructure); 16] = [
    ("array", VizStructure::Array),
    ("avl-rotation", VizStructure::Tree),
    ("bitset", VizStructure::Bitset),
    ("fenwick", VizStructure::Fenwick),
    ("graph-bfs", VizStructure::Graph),
    ("graph-kind", VizStructure::Graph),
    ("hashmap-chained-collisions", VizStructure::Hashmap),
    ("hashmap-kind", VizStructure::Hashmap),
    ("heap", VizStructure::Heap),
    ("linked-list", VizStructure::List),
    ("queue", VizStructure::Queue),
    ("segment-tree", VizStructure::SegmentTree),
    ("skiplist", VizStructure::Skiplist),
    ("stack-push", VizStructure::Stack),
    ("trie", VizStructure::Trie),
    ("union-find", VizStructure::UnionFind),
];

fn manifest(sub: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(sub)
}

fn golden_cases(name: &str) -> VizCases {
    let path = manifest("tests/fixtures/cortex-goldens").join(format!("{name}.json"));
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Compare against the checked-in file, or rewrite it when asked.
fn pin(file: &str, actual: &str) {
    let path = manifest("tests/fixtures/d2-export").join(file);
    if std::env::var_os("UPDATE_D2_EXPORT").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("no golden at {} — run with UPDATE_D2_EXPORT=1", path.display()));
    assert_eq!(
        expected, actual,
        "{file} drifted — re-run with UPDATE_D2_EXPORT=1 and read the diff",
    );
}

#[test]
fn every_family_exports_a_board_and_a_walkthrough() {
    for (name, structure) in CORPUS {
        let cases = golden_cases(name);
        let graph = cases.cases.first().expect("a golden has at least one case");
        // The MIDDLE step: step 0 is usually the structure before it exists, which would pin an
        // empty board for half the corpus.
        let at = graph.steps.len() / 2;
        pin(&format!("{name}.d2"), &d2::step_source(graph, structure, at));
        pin(
            &format!("{name}.boards.d2"),
            &d2::walkthrough_source(graph, structure),
        );
    }
}

#[test]
fn the_corpus_covers_every_structure_that_has_a_golden() {
    // A new structure with no export coverage is the failure mode here: the emitter would fall
    // through to its family's default and nobody would look at the result.
    let covered: Vec<&str> = CORPUS.iter().map(|(n, _)| *n).collect();
    let dir = manifest("tests/fixtures/cortex-goldens");
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().replace(".json", "");
        assert!(
            covered.contains(&name.as_str()),
            "golden {name} has no d2 export coverage"
        );
    }
}
