//! The authored test suite + pure judging — shared because the workbench (client) and the
//! submission judge (server) apply the SAME rules.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::execution::RunResult;

/// One declared stdin argument. The authored JSON writes `type`; the field is `tpe` here
/// (mapped at the codec) since `type` is a reserved word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ArgSpec {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub tpe: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

/// One authored case: values per declared arg + the optional expected stdout. `sample` marks
/// the browser-visible cases — the judge runs every case, but only samples cross the wire so a
/// student cannot hard-code the hidden suite. Absent in `.tests.json` ⇒ `false` ⇒ hidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TestCase {
    pub args: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(default, skip_serializing_if = "is_hidden")]
    pub sample: bool,
}

/// `skip_serializing_if` for `sample`: a hidden (non-sample) case omits the key entirely, so the
/// wire payload the browser sees carries no redundant markers. The `&bool` signature is serde's
/// requirement, not a choice.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_hidden(sample: &bool) -> bool {
    !*sample
}

/// The whole authored suite (a testcases fence or a `.tests.json` sidecar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TestSpec {
    pub args: Vec<ArgSpec>,
    pub cases: Vec<TestCase>,
}

impl TestSpec {
    /// The browser-visible projection: the declared args plus only the `sample` cases, each with
    /// its flag cleared (the samples ARE the payload, so the marker adds nothing). This is the
    /// ONLY `TestSpec` the catalog serves to a page — the hidden judge cases never leave the server.
    #[must_use]
    pub fn samples(&self) -> TestSpec {
        TestSpec {
            args: self.args.clone(),
            cases: self
                .cases
                .iter()
                .filter(|case| case.sample)
                .map(|case| TestCase {
                    args: case.args.clone(),
                    expected: case.expected.clone(),
                    sample: false,
                })
                .collect(),
        }
    }
}

/// A judged case's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    WrongAnswer,
    Errored,
    /// Ran clean with no expected output declared — counts as a pass.
    Finished,
}

/// The stdin a case feeds the program: ONE LINE PER DECLARED ARG, in declaration order
/// (missing values become empty lines), with a trailing newline.
pub fn stdin_for(args: &[ArgSpec], values: &BTreeMap<String, String>) -> String {
    let mut lines: Vec<&str> = args
        .iter()
        .map(|arg| values.get(&arg.id).map_or("", String::as_str))
        .collect();
    lines.push(""); // the trailing newline
    lines.join("\n")
}

/// Judge one run: a non-clean run is `Errored`; a clean run with no expected output is
/// `Finished`; otherwise TRIMMED stdout comparison.
pub fn judge(result: &RunResult, expected: Option<&str>) -> Verdict {
    if !result.status.is_success() {
        return Verdict::Errored;
    }
    match expected {
        None => Verdict::Finished,
        Some(expected) if result.stdout.trim() == expected.trim() => Verdict::Accepted,
        Some(_) => Verdict::WrongAnswer,
    }
}

#[cfg(test)]
#[path = "test_run_tests.rs"]
mod tests;
