//! The raw tracer wire model (oracle: `HeapTrace.scala`, ADR-S026) — the anti-corruption
//! boundary in front of the foreign tracer JSON. Language-agnostic: the Python and Java
//! harnesses emit the same `{steps, truncated}` shape. Serde here serves the TEST fixtures
//! (the oracle's hand-built traces exported as JSON) and, later, the client decoder.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A leaf value stored inline. Ints and floats are split so integer indices stay exact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HeapScalar {
    I(i64),
    D(f64),
    B(bool),
    S(String),
    Null,
}

/// A field/element value: an inline scalar or a reference to a heap object by id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HeapValue {
    Scalar(HeapScalar),
    Ref(String),
}

/// The flavour of an array-like object — a Python list/tuple or a native Java array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrKind {
    Lst,
    Tup,
    JArr,
}

/// A heap object: a class instance (named fields), an ordered array, or a dict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HeapObject {
    Instance {
        cls: String,
        fields: Vec<(String, HeapValue)>,
    },
    Arr {
        kind: ArrKind,
        items: Vec<HeapValue>,
    },
    Dict {
        entries: Vec<(HeapValue, HeapValue)>,
    },
}

/// One call-stack frame: the function name + its locals. Frames are innermost-first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeapFrame {
    #[serde(rename = "fn")]
    pub fn_name: String,
    pub locals: Vec<(String, HeapValue)>,
}

/// One traced event: the source `line`, the `event` kind (line/call/return), the live
/// frames, and the heap. `BTreeMap` keeps every heap scan deterministic by construction
/// (Rust `HashMap` order is as unspecified as the JVM↔JS hazard the oracle pinned).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeapStep {
    pub line: i32,
    pub event: String,
    pub frames: Vec<HeapFrame>,
    pub heap: BTreeMap<String, HeapObject>,
}

/// The whole trace: the surviving steps + whether the harness had to drop some.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeapTrace {
    pub steps: Vec<HeapStep>,
    pub truncated: bool,
}
