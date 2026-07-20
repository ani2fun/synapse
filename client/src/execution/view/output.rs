//! The run-output panel (oracle: `WorkbenchOutput`): the error card, the judged result card with
//! its badge and streams, and the running placeholder. Split out of `runnable.rs` in step 63 —
//! both module docs had named it a separate oracle unit since step 11, and `codebench.rs` was
//! already importing it across the module boundary.

use leptos::prelude::*;
use synapse_shared::execution::{RunResult, TestSpec, Verdict, judge};

use crate::execution::logic::{self, ExecutorState, RunState};
use crate::execution::view::workbench::TestsState;

#[component]
pub(crate) fn Output(
    state: Signal<ExecutorState>,
    spec: Option<RwSignal<TestSpec>>,
    tests: Option<TestsState>,
) -> impl IntoView {
    view! {
        {move || {
            let state = state.get();
            if let Some(error) = &state.error {
                return error_panel(error).into_any();
            }
            if let Some(result) = &state.result {
                // Judged against the case the run was LAUNCHED for — switching chips must
                // never re-label an old run's output under a different case's expected.
                // The spec read is UNTRACKED: appending a case must not re-render an
                // unrelated result.
                let expected = match (spec, tests) {
                    (Some(spec), Some(tests)) => tests
                        .ran_case
                        .get()
                        .and_then(|case| logic::expected_for(&spec.read_untracked(), case)),
                    _ => None,
                };
                return result_panel(result, expected.as_deref()).into_any();
            }
            if state.run_state == RunState::Running {
                return view! { <div class="runnable__out runnable__out--running">"Running…"</div> }
                    .into_any();
            }
            ().into_any()
        }}
    }
}

fn error_panel(error: &str) -> impl IntoView + use<> {
    view! {
        <div class="runnable__out runnable__out--error">
            <div class="runnable__status"><span class="runnable__badge runnable__badge--fail">"Error"</span></div>
            <pre class="runnable__stream">{error.to_owned()}</pre>
        </div>
    }
}

/// With an expected output the stdout is JUDGED (the wb-legend tint); without one it renders
/// plain — exactly the oracle's split.
fn result_panel(result: &RunResult, expected: Option<&str>) -> impl IntoView + use<> {
    let verdict = expected.map(|e| judge(result, Some(e)));
    let (badge_label, badge_ok) = match verdict {
        Some(Verdict::Accepted) => ("Accepted ✓".to_owned(), true),
        Some(Verdict::WrongAnswer) => ("Wrong answer ✗".to_owned(), false),
        _ => (result.status.label().to_owned(), result.status.is_success()),
    };
    let badge_class = if badge_ok {
        "runnable__badge runnable__badge--ok"
    } else {
        "runnable__badge runnable__badge--fail"
    };
    let stdout_class = match verdict {
        Some(Verdict::Accepted) => "runnable__stdout wb-legend--ok",
        Some(Verdict::WrongAnswer) => "runnable__stdout wb-legend--err",
        _ => "runnable__stdout",
    };
    let time = result.time_seconds.map(|s| format!("{s:.3} s"));
    let memory = result.memory_kb.map(|kb| format!("{} MB", kb / 1024));
    let stdout = result.stdout.clone();
    view! {
        <div class="runnable__out">
            <div class="runnable__status">
                <span class=badge_class>{badge_label}</span>
                {time.map(|t| view! { <span class="runnable__meta">{t}</span> })}
                {memory.map(|m| view! { <span class="runnable__meta">{m}</span> })}
            </div>
            {stream_block("compile output", &result.compile_output)}
            {stream_block("stderr", &result.stderr)}
            {if stdout.is_empty() {
                view! { <p class="runnable__empty">"(no output)"</p> }.into_any()
            } else {
                view! { <pre class=stdout_class>{stdout}</pre> }.into_any()
            }}
        </div>
    }
}

fn stream_block(label: &'static str, content: &str) -> Option<impl IntoView + use<>> {
    if content.is_empty() {
        return None;
    }
    let content = content.to_owned();
    Some(view! {
        <details class="runnable__details" open>
            <summary class="runnable__details-label">{label}</summary>
            <pre class="runnable__stream">{content}</pre>
        </details>
    })
}
