//! The trace decoder (oracle: `TraceDecoder`): raw run stdout → program output + the heap
//! trace between the markers. The LAST `__SYNAPSE_HEAP_BEGIN__` wins (a program printing the
//! literal marker can't spoof); BEGIN without END means the trace overflowed the sandbox's
//! stdout cap. Loud, not silent — no markers is simply "no trace", never an error swallowed.
//! The JSON walk preserves object order (locals + fields ride insertion order on the wire).

use crate::viz::engine::trace::{ArrKind, HeapFrame, HeapObject, HeapScalar, HeapStep, HeapTrace, HeapValue};

pub const HEAP_BEGIN: &str = "__SYNAPSE_HEAP_BEGIN__";
pub const HEAP_END: &str = "__SYNAPSE_HEAP_END__";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TraceError {
    #[error("The program's output was too large to trace — trim it and try again.")]
    TruncatedOutput,
    #[error("Couldn't read the trace — {0}")]
    DecodeFailed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub program_out: String,
    pub trace: Option<HeapTrace>,
}

pub fn decode(stdout: &str) -> Result<Decoded, TraceError> {
    let Some(begin) = stdout.rfind(HEAP_BEGIN) else {
        return Ok(Decoded {
            program_out: stdout.strip_suffix('\n').unwrap_or(stdout).to_owned(),
            trace: None,
        });
    };
    let program_out = stdout[..begin]
        .strip_suffix('\n')
        .unwrap_or(&stdout[..begin])
        .to_owned();
    let after_begin = &stdout[begin + HEAP_BEGIN.len()..];
    let Some(end) = after_begin.find(HEAP_END) else {
        return Err(TraceError::TruncatedOutput);
    };
    let json_raw = after_begin[..end].trim();
    let value: serde_json::Value =
        serde_json::from_str(json_raw).map_err(|e| TraceError::DecodeFailed(e.to_string()))?;
    Ok(Decoded {
        program_out,
        trace: Some(decode_trace(&value)),
    })
}

fn decode_trace(v: &serde_json::Value) -> HeapTrace {
    HeapTrace {
        steps: v
            .get("steps")
            .and_then(|s| s.as_array())
            .map(|steps| steps.iter().map(decode_step).collect())
            .unwrap_or_default(),
        truncated: v
            .get("truncated")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    }
}

fn decode_step(v: &serde_json::Value) -> HeapStep {
    let frames = v
        .get("frames")
        .and_then(|f| f.as_array())
        .map(|frames| {
            frames
                .iter()
                .map(|f| HeapFrame {
                    fn_name: f
                        .get("fn")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    locals: f
                        .get("locals")
                        .and_then(|l| l.as_object())
                        .map(|o| o.iter().map(|(n, v)| (n.clone(), decode_value(v))).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let heap = v
        .get("heap")
        .and_then(|h| h.as_object())
        .map(|o| {
            o.iter()
                .filter(|(_, obj)| !obj.is_null())
                .map(|(id, obj)| (id.clone(), decode_object(obj)))
                .collect()
        })
        .unwrap_or_default();
    HeapStep {
        line: i32::try_from(v.get("line").and_then(serde_json::Value::as_i64).unwrap_or(0)).unwrap_or(0),
        event: v
            .get("event")
            .and_then(|s| s.as_str())
            .unwrap_or("line")
            .to_owned(),
        frames,
        heap,
    }
}

fn decode_value(v: &serde_json::Value) -> HeapValue {
    match v {
        serde_json::Value::Number(n) => n.as_i64().map_or_else(
            || HeapValue::Scalar(HeapScalar::D(n.as_f64().unwrap_or(f64::NAN))),
            |i| HeapValue::Scalar(HeapScalar::I(i)),
        ),
        serde_json::Value::Bool(b) => HeapValue::Scalar(HeapScalar::B(*b)),
        serde_json::Value::String(s) => HeapValue::Scalar(HeapScalar::S(s.clone())),
        serde_json::Value::Object(o) => o
            .get("ref")
            .and_then(|r| r.as_str())
            .map_or(HeapValue::Scalar(HeapScalar::Null), |id| {
                HeapValue::Ref(id.to_owned())
            }),
        _ => HeapValue::Scalar(HeapScalar::Null),
    }
}

fn decode_object(v: &serde_json::Value) -> HeapObject {
    let kind = v.get("type").and_then(|s| s.as_str()).unwrap_or("object");
    match kind {
        // Python list/tuple and Java native arrays unify here — the tag differs, the shape
        // doesn't.
        "list" | "tuple" | "array" => HeapObject::Arr {
            kind: match kind {
                "tuple" => ArrKind::Tup,
                "array" => ArrKind::JArr,
                _ => ArrKind::Lst,
            },
            items: v
                .get("items")
                .and_then(|i| i.as_array())
                .map(|items| items.iter().map(decode_value).collect())
                .unwrap_or_default(),
        },
        "dict" => HeapObject::Dict {
            entries: v
                .get("entries")
                .and_then(|e| e.as_array())
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|pair| {
                            let pair = pair.as_array()?;
                            Some((decode_value(pair.first()?), decode_value(pair.get(1)?)))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        _ => HeapObject::Instance {
            cls: v
                .get("cls")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_owned(),
            fields: v
                .get("fields")
                .and_then(|f| f.as_object())
                .map(|o| o.iter().map(|(n, val)| (n.clone(), decode_value(val))).collect())
                .unwrap_or_default(),
        },
    }
}
