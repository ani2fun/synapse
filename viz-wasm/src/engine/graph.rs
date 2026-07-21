//! The render contract. `VizCases` is the
//! ubiquitous language every producer speaks: the live-trace adapter emits it, and
//! hand-authored ` ```viz widget= ` payloads decode INTO it — the modal canvas and an inline
//! widget are the same host consuming the same type.
//!
//! This is the anti-corruption WIRE contract: field names are pinned to match the
//! cortex-goldens fixtures exactly, so they compare byte-for-byte. Serialization is FAITHFUL
//! (every field emitted,
//! `None` as `null`); deserialization is TOLERANT — the authored payload is a bare
//! `{title?, steps}` (no `cases` wrapper), its `annotation` may be a plain string, and most
//! fields are omitted (defaults fill them). `kind`/`layoutKind` stay strings: an OPEN
//! renderer vocabulary, not a closed set.

use serde::{Deserialize, Serialize};

/// An opaque render-node id — a string on the wire, but not interchangeable with other
/// strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.0
    }
}

/// A `name: value` extra shown under a node (non-primary scalar fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VizField {
    pub name: String,
    pub value: String,
}

/// One drawable node. `slot` orders cells in a row/grid; `card_id` groups nodes per heap
/// object; `kind`/`layout_kind` are per-node styling/geometry hints (open vocabulary).
/// `id` is REQUIRED on decode (a node without one is a loud error, by design); the rest
/// default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VizNode {
    pub id: NodeId,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub meta: Vec<VizField>,
    #[serde(default)]
    pub slot: Option<i32>,
    #[serde(default)]
    pub card_id: String,
    #[serde(default)]
    pub layout_kind: String,
}

impl Default for VizNode {
    fn default() -> Self {
        Self {
            id: NodeId::new(""),
            label: String::new(),
            kind: String::new(),
            meta: Vec::new(),
            slot: None,
            card_id: String::new(),
            layout_kind: String::new(),
        }
    }
}

/// A directed edge; `label` is the field name it came from (`left`/`right`/`next`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VizEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub label: String,
}

/// A pointer landing on a node; `color` is a role colour from `markers` (empty → assigned
/// later). `target` is REQUIRED on decode, by design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VizCursor {
    #[serde(default)]
    pub name: String,
    pub target: NodeId,
    #[serde(default)]
    pub color: String,
}

/// The step caption. Authored payloads give a bare string → it lands in `body`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Annotation {
    pub eyebrow: String,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
}

// A plain string OR the object form — both decode (the authored-vs-adapter split).
impl<'de> Deserialize<'de> for Annotation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Text(String),
            Object {
                #[serde(default)]
                eyebrow: String,
                #[serde(default)]
                title: String,
                #[serde(default)]
                body: String,
                #[serde(default)]
                link: Option<String>,
            },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Text(body) => Self {
                body,
                ..Self::default()
            },
            Wire::Object {
                eyebrow,
                title,
                body,
                link,
            } => Self {
                eyebrow,
                title,
                body,
                link,
            },
        })
    }
}

/// One row in the frames panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VizLocal {
    pub name: String,
    pub type_name: String,
    pub value: String,
    pub changed: bool,
}

/// A call-stack frame (innermost-first) for the frames panel. `fn` is a Rust keyword — the
/// wire name is pinned by an explicit rename.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VizFrame {
    #[serde(rename = "fn")]
    pub fn_name: String,
    pub locals: Vec<VizLocal>,
    pub is_active: bool,
}

/// One animation step: the drawable graph + diff cues + the caption + the source line.
/// `card_cursor` is reserved (the arrow-layer feature is cut) — kept for wire parity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VizStep {
    pub nodes: Vec<VizNode>,
    pub edges: Vec<VizEdge>,
    pub cursor: Vec<VizCursor>,
    pub highlight: Vec<NodeId>,
    pub changed: Vec<NodeId>,
    pub removed: Vec<NodeId>,
    pub annotation: Annotation,
    pub line: i32,
    pub frames: Vec<VizFrame>,
    pub card_cursor: Vec<VizCursor>,
    /// Diff mode: only the source line advanced.
    pub unchanged: bool,
    pub structure_type: Option<String>,
}

/// One traced case (or one authored widget): its steps + the layout hint + title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VizGraph {
    pub steps: Vec<VizStep>,
    pub layout_hint: String,
    pub title: String,
    pub truncated: bool,
}

/// The payload the renderer consumes — one entry per case, or a single authored graph.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct VizCases {
    pub cases: Vec<VizGraph>,
}

// A `{cases:[…]}` wire payload, OR a bare authored `{title?, steps}` → one-case VizCases.
impl<'de> Deserialize<'de> for VizCases {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Wrapped { cases: Vec<VizGraph> },
            Bare(VizGraph),
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Wrapped { cases } => Self { cases },
            Wire::Bare(graph) => Self { cases: vec![graph] },
        })
    }
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
