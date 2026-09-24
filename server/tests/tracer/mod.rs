//! What the tracer ITs share: the sandbox gate, the harness wrap and pulling the trace back out.
//!
//! Hand-rolled where a crate would do — base64 above all — because each piece is needed ONLY by
//! these suites, and a dependency earns its place by being needed in the product (RS001).

use synapse_server::execution::infrastructure::GoJudgeRunner;

const PLACEHOLDER: &str = "__SYNAPSE_USER_SOURCE_B64__";

/// The live sandbox, or `None` (with the line CI greps for) when the suite is not asked to run.
/// At the production anonymous share (50%), and the suites run `Tier::Anonymous`: the lab is
/// used signed-out, so a trace has to fit an anonymous budget, and this is where that is proved.
pub fn gated() -> Option<GoJudgeRunner> {
    if std::env::var("GOJUDGE_IT").is_err() {
        eprintln!("skipped (set GOJUDGE_IT=1 with a live go-judge to run)");
        return None;
    }
    let url = std::env::var("EXECUTOR_URL").unwrap_or_else(|_| "http://localhost:5150".to_owned());
    Some(GoJudgeRunner::new(&url, 50))
}

/// A harness with `source` embedded — what the client sends to `/api/run`.
pub fn wrap(harness: &str, source: &str) -> String {
    harness.replace(PLACEHOLDER, &base64(source.as_bytes()))
}

/// The trace the harness printed between its markers, and everything the program printed before.
pub fn split(stdout: &str) -> (&str, serde_json::Value) {
    let (program_out, rest) = stdout
        .split_once("__SYNAPSE_HEAP_BEGIN__")
        .unwrap_or_else(|| panic!("no heap trace in stdout: {stdout}"));
    let body = rest.split("__SYNAPSE_HEAP_END__").next().unwrap_or_default();
    (
        program_out,
        serde_json::from_str(body.trim()).expect("the trace is JSON"),
    )
}

/// Standard base64.
fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
