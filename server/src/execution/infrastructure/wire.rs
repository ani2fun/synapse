//! The go-judge wire protocol, pure string→JSON→string, golden-tested. One `POST /run` cmd
//! runs compile+run in a single `/bin/sh -c` invocation;
//! compile failures are detected via the `__cf_crc`/`__cf_cerr` marker files (the compiler
//! step exits 0 WITHOUT running, go-judge reports "Accepted", and the parser reads the rc
//! file) — never via HTTP status.

use serde_json::{Value, json};
use synapse_shared::execution::{RunResult, RunStatus};

use crate::execution::domain::GO_JUDGE_LIMITS;

use crate::execution::domain::Language;
use crate::execution::infrastructure::java_rewriter;
use crate::execution::infrastructure::recipe::Recipe;

pub(crate) const RUN_PATH: &str = "/run";
const COMPILE_RC_FILE: &str = "__cf_crc";
const COMPILE_ERR_FILE: &str = "__cf_cerr";
const PROC_LIMIT: u32 = 256;

/// Build the `POST /run` body for one run. Output streams are RAW (not base64).
pub(crate) fn build_request_body(language: Language, source: &str, stdin: Option<&str>) -> String {
    let recipe = Recipe::for_language(language);
    let effective = java_rewriter::effective_source(language, source);
    let compiled = recipe.compile.is_some();

    let copy_out: Vec<&str> = if compiled {
        vec!["__cf_crc?", "__cf_cerr?"]
    } else {
        Vec::new()
    };
    let body = json!({
        "cmd": [{
            "args": ["/bin/sh", "-c", shell_body(&recipe)],
            "env": [
                "PATH=/usr/bin:/bin:/usr/local/bin",
                "HOME=/w",
                "GOCACHE=/w/.cache",
                "GOPATH=/w/go"
            ],
            "files": [
                { "content": stdin.unwrap_or("") },
                { "name": "stdout", "max": GO_JUDGE_LIMITS.max_stdout_bytes },
                { "name": "stderr", "max": GO_JUDGE_LIMITS.max_stdout_bytes }
            ],
            "cpuLimit": recipe.cpu_seconds * 1_000_000_000,
            "clockLimit": recipe.clock_seconds * 1_000_000_000,
            "memoryLimit": recipe.memory_mib * 1024 * 1024,
            "procLimit": PROC_LIMIT,
            "copyIn": { recipe.source_file: { "content": effective } },
            "copyOut": copy_out
        }]
    });
    body.to_string()
}

fn shell_body(recipe: &Recipe) -> String {
    match recipe.compile {
        None => recipe.run.to_owned(),
        Some(compile) => format!(
            "{compile} 2>{COMPILE_ERR_FILE}; echo $? >{COMPILE_RC_FILE}; \
             if [ \"$(cat {COMPILE_RC_FILE})\" != \"0\" ]; then exit 0; fi; {run}",
            run = recipe.run
        ),
    }
}

/// Map go-judge's response (an array; take `[0]`) back to the shared `RunResult`.
/// `time` is nanoseconds, `memory` is bytes; both optional (absent → `None`, not 0).
pub(crate) fn parse_run_result(compiled: bool, body: &str) -> Result<RunResult, String> {
    let parsed: Value =
        serde_json::from_str(body).map_err(|e| format!("go-judge returned invalid JSON: {e}"))?;
    let result = parsed
        .get(0)
        .ok_or_else(|| "go-judge returned an empty result array".to_owned())?;

    let status = result.get("status").and_then(Value::as_str).unwrap_or("");
    let exit_status = result.get("exitStatus").and_then(Value::as_i64).unwrap_or(0);
    let file = |name: &str| -> String {
        result
            .get("files")
            .and_then(|files| files.get(name))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let time_seconds = result.get("time").and_then(Value::as_f64).map(|ns| ns / 1e9);
    let memory_kb = result
        .get("memory")
        .and_then(Value::as_i64)
        .map(|bytes| bytes / 1024);

    let compile_rc = file(COMPILE_RC_FILE);
    let compile_failed = compiled && !compile_rc.trim().is_empty() && compile_rc.trim() != "0";
    if compile_failed {
        return Ok(RunResult {
            status: RunStatus::CompileError,
            stdout: String::new(),
            stderr: String::new(),
            compile_output: file(COMPILE_ERR_FILE),
            time_seconds,
            memory_kb,
        });
    }

    let run_status = match status {
        "Accepted" if exit_status == 0 => RunStatus::Accepted,
        "TimeLimitExceeded" => RunStatus::TimeLimitExceeded,
        "InternalError" | "FileError" => RunStatus::InternalError,
        _ => RunStatus::RuntimeError, // incl. "Accepted" with a nonzero exit
    };
    Ok(RunResult {
        status: run_status,
        stdout: file("stdout"),
        stderr: file("stderr"),
        compile_output: String::new(),
        time_seconds,
        memory_kb,
    })
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
