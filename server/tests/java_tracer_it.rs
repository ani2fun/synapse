//! Live Java-tracer IT, gated behind `GOJUDGE_IT` — needs `docker compose up -d go-judge`
//! (host :5150). Run:
//! `GOJUDGE_IT=1 EXECUTOR_URL=http://localhost:5150 cargo test --test java_tracer_it -- --test-threads=1`
//!
//! WHY THIS EXISTS. The heap ids the tracer emits must identify OBJECTS and stay stable for the
//! whole trace, because the adapt pipeline compares them BETWEEN steps: a root whose id changes
//! reads as "rebound to an unrelated structure", i.e. a new test case. Python gets this free from
//! `id(obj)`; the Java harness has to remember identity itself.
//!
//! It once numbered objects per step by walk order, so the same array changed id the moment the
//! walk order shifted — entering a method whose `this` is walked first was enough. One plain run
//! split into three phantom "cases", and the first of them showed the array before the algorithm
//! had touched it. Only a real JVM run can catch that, hence a gated IT rather than a fixture.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use synapse_server::execution::application::CodeRunner;
use synapse_server::execution::domain::Language;
use synapse_server::execution::infrastructure::GoJudgeRunner;

const HARNESS: &str = include_str!("../../web/src/lib/islands/tracer/java-harness.java");
const PLACEHOLDER: &str = "__SYNAPSE_USER_SOURCE_B64__";

/// A caller and a callee that mutate ONE array — the shape that exposed the bug: `arr` is a local
/// in `main` and a parameter in `flip`, and `Solution` adds a `this` that shifts the walk order.
const USER_SOURCE: &str = r"
public class Main {
    static class Solution {
        void flip(char[] arr) {
            int left = 0;
            int right = arr.length - 1;
            while (left < right) {
                char t = arr[left];
                arr[left] = arr[right];
                arr[right] = t;
                left++;
                right--;
            }
        }
    }

    public static void main(String[] args) {
        char[] arr = new char[] { 'a', 'b', 'c' };
        new Solution().flip(arr);
        System.out.println(arr[0]);
    }
}
";

fn gated() -> Option<GoJudgeRunner> {
    if std::env::var("GOJUDGE_IT").is_err() {
        eprintln!("skipped (set GOJUDGE_IT=1 with a live go-judge to run)");
        return None;
    }
    let url = std::env::var("EXECUTOR_URL").unwrap_or_else(|_| "http://localhost:5150".to_owned());
    Some(GoJudgeRunner::new(&url))
}

/// Standard base64, hand-rolled: the encoder is needed ONLY here, and a dependency earns its
/// place by being needed in the product (RS001).
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

/// The `arr` ref id in the innermost frame of the first step whose innermost frame is `fn_name`.
fn root_id_in(steps: &[serde_json::Value], fn_name: &str) -> Option<String> {
    steps.iter().find_map(|step| {
        let frames = step.get("frames")?.as_array()?;
        let inner = frames.first()?;
        if inner.get("fn")?.as_str()? != fn_name {
            return None;
        }
        Some(inner.get("locals")?.get("arr")?.get("ref")?.as_str()?.to_owned())
    })
}

#[tokio::test]
async fn heap_ids_identify_objects_across_frames_not_walk_order() {
    let Some(runner) = gated() else { return };
    let source = HARNESS.replace(PLACEHOLDER, &base64(USER_SOURCE.as_bytes()));

    let result = runner.run(Language::Java, &source, None).await.unwrap();
    let stdout = result.stdout;

    let body = stdout
        .split("__SYNAPSE_HEAP_BEGIN__")
        .nth(1)
        .and_then(|s| s.split("__SYNAPSE_HEAP_END__").next())
        .unwrap_or_else(|| panic!("no heap trace in stdout: {stdout}"));
    let trace: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
    let steps = trace["steps"].as_array().unwrap();
    assert!(
        steps.len() > 5,
        "expected a real trace, got {} step(s)",
        steps.len()
    );

    // The callee stepped into at all — without this the rest is vacuous.
    let inside_flip =
        root_id_in(steps, "flip").expect("no step with `flip` innermost — the callee was not traced");
    let from_main = root_id_in(steps, "main").expect("no step with `main` innermost");

    // THE REGRESSION: one array, passed by reference, must keep ONE id across both frames.
    assert_eq!(
        from_main, inside_flip,
        "the same array got different heap ids in `main` ({from_main}) and `flip` ({inside_flip}) — \
         ids are numbering walk order again, which splits one run into phantom test cases"
    );

    // And the mutation is visible: the last step's heap has the array reversed.
    let last = steps.last().unwrap();
    let items = last["heap"][&from_main]["items"].as_array().unwrap();
    let chars: Vec<&str> = items.iter().filter_map(serde_json::Value::as_str).collect();
    assert_eq!(
        chars,
        ["c", "b", "a"],
        "the final heap should show the flipped array"
    );
}
