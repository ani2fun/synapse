//! The tolerant decoders over VERBATIM authored payloads, and
//! the faithful round-trip of the adapter wire form.

#![allow(clippy::unwrap_used)]

use super::*;

const ARRAY_PAYLOAD: &str = r##"{ "steps": [ {
  "nodes": [
    {"id":"0","label":"2","kind":"cell","meta":[],"slot":0,"cardId":"","layoutKind":""},
    {"id":"1","label":"8","kind":"cell","meta":[],"slot":1,"cardId":"","layoutKind":""}
  ],
  "edges": [],
  "cursor": [ {"name":"left","target":"0","color":"#3b82f6"}, {"name":"right","target":"1","color":"#f59e0b"} ],
  "highlight": [], "changed": [], "removed": [],
  "annotation": "sum = 2 + 8 = 10 > 7 -> discard arr[right]; right--.",
  "line": 0, "frames": [], "cardCursor": []
} ] }"##;

const LIST_PAYLOAD: &str = r##"{ "title": "Singly-linked list: 1 -> 2 -> 3",
  "steps": [ {
    "nodes": [
      {"id":"n0","label":"1","kind":"node","slot":null,"meta":[],"cardId":"","layoutKind":""},
      {"id":"n1","label":"2","kind":"node","slot":null,"meta":[],"cardId":"","layoutKind":""},
      {"id":"n2","label":"3","kind":"node","slot":null,"meta":[],"cardId":"","layoutKind":""}
    ],
    "edges": [ {"from":"n0","to":"n1","label":"next"}, {"from":"n1","to":"n2","label":"next"} ],
    "cursor": [ {"name":"head","target":"n0","color":"#6366f1"} ],
    "annotation": "head -> 1 -> 2 -> 3 -> null."
  } ] }"##;

const TREE_PAYLOAD: &str = r#"{ "steps": [ {
  "nodes": [
    {"id":"n4","label":"4","kind":"node"}, {"id":"n2","label":"2","kind":"node"}, {"id":"n6","label":"6","kind":"node"}
  ],
  "edges": [ {"from":"n4","to":"n2","label":"left"}, {"from":"n4","to":"n6","label":"right"} ]
} ] }"#;

#[test]
fn a_bare_steps_payload_decodes_to_one_case_string_annotation_lands_in_body() {
    let cases: VizCases = serde_json::from_str(ARRAY_PAYLOAD).unwrap();
    assert_eq!(cases.cases.len(), 1);
    let step = &cases.cases[0].steps[0];
    assert_eq!(
        step.nodes.iter().map(|n| n.slot).collect::<Vec<_>>(),
        vec![Some(0), Some(1)]
    );
    assert_eq!(
        step.cursor
            .iter()
            .map(|c| (c.name.as_str(), c.target.value()))
            .collect::<Vec<_>>(),
        vec![("left", "0"), ("right", "1")]
    );
    assert!(step.annotation.body.starts_with("sum = 2 + 8"));
    assert!(step.annotation.eyebrow.is_empty());
}

#[test]
fn a_list_payload_keeps_title_next_edges_and_null_slots() {
    let cases: VizCases = serde_json::from_str(LIST_PAYLOAD).unwrap();
    let graph = &cases.cases[0];
    assert_eq!(graph.title, "Singly-linked list: 1 -> 2 -> 3");
    assert_eq!(
        graph.steps[0]
            .edges
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>(),
        vec!["next", "next"]
    );
    assert!(graph.steps[0].nodes.iter().all(|n| n.slot.is_none()));
}

#[test]
fn a_tree_payload_with_omitted_fields_decodes_to_defaults() {
    let cases: VizCases = serde_json::from_str(TREE_PAYLOAD).unwrap();
    let step = &cases.cases[0].steps[0];
    assert_eq!(
        step.edges.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(),
        vec!["left", "right"]
    );
    assert!(step.highlight.is_empty() && step.changed.is_empty() && step.frames.is_empty());
    assert_eq!(step.line, 0);
    assert!(!step.unchanged);
    assert!(step.annotation.body.is_empty());
}

#[test]
fn the_adapter_wire_form_round_trips() {
    let original = VizCases {
        cases: vec![VizGraph {
            steps: vec![VizStep {
                nodes: vec![VizNode {
                    id: NodeId::new("a"),
                    label: "1".to_owned(),
                    kind: "node".to_owned(),
                    ..VizNode::default()
                }],
                annotation: Annotation {
                    eyebrow: "line".to_owned(),
                    title: "i moves to 4".to_owned(),
                    body: "arr[i] = 4".to_owned(),
                    link: None,
                },
                line: 7,
                structure_type: Some("stack".to_owned()),
                ..VizStep::default()
            }],
            layout_hint: "array".to_owned(),
            title: "demo".to_owned(),
            truncated: false,
        }],
    };
    let json = serde_json::to_string(&original).unwrap();
    let decoded: VizCases = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn the_faithful_encoder_emits_every_field_including_nulls() {
    // The faithful encoder never skips a field: options serialize as null, nothing omitted.
    let step = VizStep::default();
    let json = serde_json::to_string(&step).unwrap();
    for field in [
        "\"nodes\"",
        "\"edges\"",
        "\"cursor\"",
        "\"highlight\"",
        "\"changed\"",
        "\"removed\"",
        "\"annotation\"",
        "\"line\"",
        "\"frames\"",
        "\"cardCursor\"",
        "\"unchanged\"",
        "\"structureType\":null",
    ] {
        assert!(json.contains(field), "{field} missing from {json}");
    }
    let node_json = serde_json::to_string(&VizNode::default()).unwrap();
    assert!(node_json.contains("\"slot\":null"));
    assert!(node_json.contains("\"cardId\""));
    assert!(node_json.contains("\"layoutKind\""));
}
