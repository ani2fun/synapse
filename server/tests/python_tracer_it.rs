//! Live Python-tracer IT, gated behind `GOJUDGE_IT` — needs `docker compose up -d go-judge`
//! (host :5150). Run:
//! `GOJUDGE_IT=1 EXECUTOR_URL=http://localhost:5150 cargo test --test python_tracer_it -- --test-threads=1`
//!
//! WHY THIS EXISTS. What the harness reports depends on the sandbox's Python, not on the harness
//! text: 3.12+ INLINES a comprehension into its owner (PEP 709), and at module scope that turns
//! the module frame's `f_locals` into a proxy over ONLY the comprehension's variables. Read as
//! the module's, the Global frame lost every name the reader had defined for as long as the loop
//! ran. A class body is likewise a frame of its own that the tracer walked, one `def` line at a
//! time. Neither shows in a fixture: only a real interpreter behaves like one, hence a gated IT.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod tracer;

use synapse_server::execution::application::CodeRunner;
use synapse_server::execution::domain::Language;

const HARNESS: &str = include_str!("../../web/src/lib/islands/tracer/python-harness.py");

/// A class with a method, then a driver that reads its input and builds a matrix with a NESTED
/// comprehension at module scope — the shape that emptied the Global frame.
const USER_SOURCE: &str = r#"class Solution:
    def spiralOrder(self, matrix):
        ans = []
        top, bottom, left, right = 0, len(matrix) - 1, 0, len(matrix[0]) - 1
        while left <= right and top <= bottom:
            for i in range(left, right + 1):
                ans.append(matrix[top][i])
            top += 1
            for i in range(top, bottom + 1):
                ans.append(matrix[i][right])
            right -= 1
            if top <= bottom:
                for i in range(right, left - 1, -1):
                    ans.append(matrix[bottom][i])
                bottom -= 1
            if left <= right:
                for i in range(bottom, top - 1, -1):
                    ans.append(matrix[i][left])
                left += 1
        return ans


inner = input().strip()[1:-1].strip()
rows = inner.split("], [") if inner else []
matrix = [[int(t) for t in r.strip("[]").split(",")] for r in rows]
print(Solution().spiralOrder(matrix))
"#;

const STDIN: &str = "[[1, 2, 3], [4, 5, 6], [7, 8, 9]]\n";

/// The 1-based line holding `needle`, so the assertions follow the source rather than a count.
fn line_of(needle: &str) -> i64 {
    let at = USER_SOURCE
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line contains {needle:?}"));
    i64::try_from(at + 1).unwrap()
}

/// One traced run of the program: what it printed, and its steps.
async fn traced() -> Option<(String, Vec<serde_json::Value>, serde_json::Value)> {
    let runner = tracer::gated()?;
    let source = tracer::wrap(HARNESS, USER_SOURCE);
    let result = runner.run(Language::Python, &source, Some(STDIN)).await.unwrap();
    let (program_out, trace) = tracer::split(&result.stdout);
    let steps = trace["steps"].as_array().unwrap().clone();
    assert!(
        steps.len() > 20,
        "expected a real trace, got {} step(s)",
        steps.len()
    );
    Some((program_out.trim().to_owned(), steps, trace))
}

fn innermost(step: &serde_json::Value) -> &str {
    step["frames"][0]["fn"].as_str().unwrap()
}

/// The module's frame — the outermost, last in the innermost-first list.
fn module(step: &serde_json::Value) -> &serde_json::Value {
    step["frames"].as_array().unwrap().last().unwrap()
}

#[tokio::test]
async fn a_class_body_is_not_stepped() {
    let Some((_, steps, _)) = traced().await else {
        return;
    };
    assert!(
        steps.iter().all(|step| innermost(step) != "Solution"),
        "a step walks the class body — the reader sees a frame named after their class stepping \
         through `def` lines"
    );
    // So the step after the `class` line is the first line after the class.
    assert_eq!(steps[1]["line"].as_i64(), Some(line_of("inner = input()")));
}

#[tokio::test]
async fn the_global_frame_keeps_its_names_through_a_comprehension() {
    let Some((_, steps, _)) = traced().await else {
        return;
    };
    let comprehension = line_of("matrix = [[int(t)");
    let during: Vec<&serde_json::Value> = steps
        .iter()
        .filter(|step| step["line"].as_i64() == Some(comprehension) && module(step).get("comp").is_some())
        .collect();
    assert!(
        !during.is_empty(),
        "no step inside the comprehension reported its variables"
    );
    for step in &during {
        let globals = &module(step)["locals"];
        for name in ["Solution", "inner", "rows"] {
            assert!(
                globals.get(name).is_some(),
                "`{name}` vanished from the Global frame mid-comprehension: {globals}"
            );
        }
        assert!(
            module(step)["comp"].get("r").is_some(),
            "the comprehension's own variable is missing"
        );
        assert!(
            globals.get("r").is_none(),
            "a comprehension variable is passing for a global"
        );
    }
    // And once defined, `Solution` is there at every step — the frame never empties.
    assert!(
        steps
            .iter()
            .skip(1)
            .all(|step| module(step)["locals"].get("Solution").is_some())
    );
}

#[tokio::test]
async fn a_function_is_its_signature_and_the_readers_class_lists_its_methods() {
    let Some((_, steps, _)) = traced().await else {
        return;
    };
    let last = steps.last().unwrap();
    let heap = last["heap"].as_object().unwrap();
    let class_id = module(last)["locals"]["Solution"]["ref"].as_str().unwrap();
    let class = &heap[class_id];
    assert_eq!(class["type"], "class");
    assert_eq!(class["name"], "Solution");
    let method_id = class["members"]["spiralOrder"]["ref"].as_str().unwrap();
    assert_eq!(heap[method_id]["type"], "function");
    assert_eq!(heap[method_id]["sig"], "spiralOrder(self, matrix)");
}

#[tokio::test]
async fn the_run_ends_on_the_module_returning_and_prints_the_answer() {
    let Some((program_out, steps, trace)) = traced().await else {
        return;
    };
    assert_eq!(program_out, "[1, 2, 3, 6, 9, 8, 7, 4, 5]");
    assert!(trace["error"].is_null() && trace["waiting"] == false);
    let last = steps.last().unwrap();
    assert_eq!(last["event"], "return");
    assert_eq!(
        last["frames"].as_array().unwrap().len(),
        1,
        "the last step is the module's own return"
    );
}
