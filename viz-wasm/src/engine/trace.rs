//! The raw tracer wire model — the anti-corruption
//! boundary in front of the foreign tracer JSON. Language-agnostic: the Python and Java
//! harnesses emit the same `{steps, truncated}` shape. Serde here serves the TEST fixtures
//! (hand-built traces stored as JSON) and the client decoder.

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
/// frames, and the heap. `BTreeMap` keeps every heap scan deterministic by construction —
/// a Rust `HashMap`'s iteration order is unspecified, and object-key order varies across
/// JS engines too, so nothing here can be allowed to depend on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeapStep {
    pub line: i32,
    pub event: String,
    pub frames: Vec<HeapFrame>,
    pub heap: BTreeMap<String, HeapObject>,
    /// How many BYTES the program had printed by the time this step ran — an index into the run's
    /// program output. What lets a reader watch output arrive as they step instead of reading the
    /// whole run's answer at step 0.
    #[serde(default)]
    pub out: usize,
}

/// Why a run ended badly: an uncaught exception, or a source that never compiled at all.
///
/// The only useful thing such a run has to say. A trace whose program crashed still has every
/// step up to the crash, so the run is worth showing — but showing it WITHOUT this leaves the
/// reader stepping to the end of a story that simply stops, with nothing saying it broke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunError {
    /// The exception's own class name — `ZeroDivisionError`, `SyntaxError`.
    pub kind: String,
    pub message: String,
    /// Where it happened, 1-indexed. 0 when the harness could not place it.
    pub line: i32,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)?;
        if self.line > 0 {
            write!(f, " (line {})", self.line)?;
        }
        Ok(())
    }
}

/// One value `input()` handed the program, and the step that read it.
///
/// The step matters because a reader can stand ANYWHERE in the trace: at step 3 the program has
/// not yet asked for the value it read at step 40, and a log that presented both as consumed
/// would credit it with knowing an answer it had not been given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Served {
    pub value: String,
    /// Index into `HeapTrace::steps` of the step whose line called `input()`.
    pub at: usize,
}

/// The whole trace: the surviving steps, whether the harness had to drop some, and what the
/// program did with stdin.
///
/// The input fields are what make stepping feel interactive over a batch sandbox. A run is given
/// its whole stdin up front and cannot be typed into, so instead the harness REPORTS: `inputs` is
/// every value it served, in order, and `waiting` says the program asked for one more and stdin
/// was empty — it stopped at that step rather than raising. Replaying `inputs` plus one new line
/// resumes the same story, which is why their order is a contract and not a convenience.
///
/// Every one of them defaults: the Java harness emits none of this, and the golden fixtures
/// predate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HeapTrace {
    pub steps: Vec<HeapStep>,
    pub truncated: bool,
    /// The values `input()` returned, in the order it returned them.
    pub inputs: Vec<Served>,
    /// The program asked for input the run could not serve, and stopped at that step.
    pub waiting: bool,
    /// What it asked with, when it passed a prompt — the program's own words, not ours.
    pub prompt: String,
    /// The exception that ended the run, when one did. `None` is a program that finished.
    pub error: Option<RunError>,
}
