//! The trace session: wrap the code in the language's harness, run
//! it through the ORDINARY `/api/run` (no new endpoint), decode the markers, and
//! adapt through the SAME shared pipeline the goldens pin. Cached per
//! (language, source, structure, root, stdin); Re-trace forces a fresh run. Every failure is
//! a Failed card, never a blank modal.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::engine::adapt;
use crate::engine::graph::VizCases;
use crate::engine::memory::{self, MemoryStep};
use crate::engine::trace::{RunError, Served};
use crate::engine::vocabulary::VizStructure;
use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::execution::RunRequest;

use crate::api;
use crate::engine::decoder::{self, Decoded};
use crate::ffi::tracer;

/// One finished run, in both lenses.
///
/// `cases` is a RESULT, not a precondition: the structure lens needs a root it can project and
/// often has none — a program with two lists and a counter fits no `viz=` token. That used to
/// fail the whole run. It no longer can, because the MEMORY lens needs no vocabulary at all, so a
/// trace that decodes is always worth showing; only the structure half reports the objection.
#[derive(Clone, PartialEq)]
pub struct Run {
    pub cases: Result<VizCases, String>,
    pub memory: Vec<MemoryStep>,
    pub program_out: String,
    /// Every value `input()` was served, in order, each with the step that read it — what a
    /// re-run replays to get back here, and what tells a reader mid-trace which of them the
    /// program has actually reached.
    pub inputs: Vec<Served>,
    /// The program asked for a value the run could not serve and stopped there.
    pub waiting: bool,
    /// What it asked with, in the program's own words.
    pub prompt: String,
    /// The exception that ended the run, when one did — the reason a story that simply stops
    /// stopped. `None` is a program that finished.
    pub error: Option<RunError>,
}

#[derive(Clone, PartialEq)]
pub enum TraceState {
    Tracing,
    Ready(Run),
    Failed(String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Key {
    pub language: String,
    pub source: String,
    pub structure: VizStructure,
    pub root: Option<String>,
    pub stdin: String,
}

/// Everything the modal needs to show one traced run.
#[derive(Clone)]
pub struct Session {
    pub key: Key,
    pub state: RwSignal<TraceState>,
}

thread_local! {
    static CACHE: RefCell<HashMap<Key, Session>> = RefCell::new(HashMap::new());
    // Sessions live in the global cache and outlive every view, so their signals must be
    // owned by a DETACHED root — a session minted inside a click handler would otherwise die
    // with that handler's reactive scope (the modal's own re-trace button disposes itself on
    // store.open, and reading the dead signal panics the whole reactive graph).
    static SESSION_OWNER: Owner = Owner::new_root(None);
}

/// A session whose `state` signal is owned by the detached root — safe to cache + read from
/// any (later) view, regardless of which scope asked for the trace.
fn mint(key: Key) -> Session {
    SESSION_OWNER.with(|owner| {
        owner.with(|| Session {
            key,
            state: RwSignal::new(TraceState::Tracing),
        })
    })
}

/// Cached: the same code+case re-opens instantly; `force` re-traces.
pub fn obtain(key: Key) -> Session {
    if let Some(session) = CACHE.with_borrow(|c| c.get(&key).cloned()) {
        return session;
    }
    let session = mint(key.clone());
    CACHE.with_borrow_mut(|c| c.insert(key, session.clone()));
    run(&session);
    session
}

/// A FRESH trace for a (possibly new) key — replaces any cached session and re-runs.
pub fn obtain_fresh(key: Key) -> Session {
    let session = mint(key.clone());
    CACHE.with_borrow_mut(|c| c.insert(key, session.clone()));
    run(&session);
    session
}

pub fn force(session: &Session) {
    session.state.set(TraceState::Tracing);
    run(session);
}

fn run(session: &Session) {
    let key = session.key.clone();
    let state = session.state;
    crate::log::info(&format!("tracing {} ({})", key.language, key.structure.token()));
    spawn_local(async move {
        let wrapped = match key.language.to_lowercase().as_str() {
            "java" => tracer::wrap_java(&key.source).await,
            _ => tracer::wrap_python(&key.source).await,
        };
        let wrapped = match wrapped {
            Ok(w) => w,
            Err(error) => {
                return state.set(TraceState::Failed(format!("tracer island failed: {error:?}")));
            }
        };
        let request = RunRequest {
            language: key.language.clone(),
            source: wrapped,
            stdin: Some(key.stdin.clone()).filter(|s| !s.is_empty()),
        };
        match api::run(&request).await {
            Err(message) => {
                crate::log::error(&format!("trace failed: {message}"));
                state.set(TraceState::Failed(message));
            }
            Ok(result) => {
                let next = outcome(&key, &result.stdout, &result.stderr, &result.compile_output);
                match &next {
                    TraceState::Ready(run) => {
                        crate::log::debug(&format!(
                            "trace ready: {} step(s), {} input(s){}",
                            run.memory.len(),
                            run.inputs.len(),
                            if run.waiting { ", awaiting input" } else { "" },
                        ));
                    }
                    TraceState::Failed(message) => {
                        crate::log::warn(&format!("trace produced no playable run: {message}"));
                    }
                    TraceState::Tracing => {}
                }
                state.set(next);
            }
        }
    });
}

/// The stdin a re-run needs to reach the point a reader is standing at, plus their answer.
///
/// This is what makes stepping-with-input work over a batch sandbox: the program cannot be typed
/// into, so it is RUN AGAIN from the top with everything it was served last time and one line
/// more. One line each and a trailing newline on the last, because `input()` reads a LINE — an
/// unterminated final line is read as EOF, which would stop the run at the very prompt the
/// answer was meant to satisfy.
#[must_use]
pub fn replay_stdin(served: &[Served], answer: &str) -> String {
    let mut stdin = String::new();
    for input in served {
        stdin.push_str(&input.value);
        stdin.push('\n');
    }
    stdin.push_str(answer);
    stdin.push('\n');
    stdin
}

/// Pure: run output → the host's state.
fn outcome(key: &Key, stdout: &str, stderr: &str, compile_output: &str) -> TraceState {
    match decoder::decode(stdout) {
        Err(error) => TraceState::Failed(error.to_string()),
        Ok(Decoded {
            program_out,
            trace: None,
        }) => TraceState::Failed(no_trace_message(stderr, compile_output, &program_out)),
        // A trace with no steps is a program that never ran a line — a source that did not
        // compile, most often. There is nothing to show and everything to explain, so it fails
        // with the harness's own reason where it has one: the harness knows the LINE, and a
        // scraped stderr is a traceback through the harness's own frames.
        Ok(Decoded {
            program_out,
            trace: Some(trace),
        }) if trace.steps.is_empty() => TraceState::Failed(trace.error.map_or_else(
            || no_trace_message(stderr, compile_output, &program_out),
            |error| error.to_string(),
        )),
        Ok(Decoded {
            program_out,
            trace: Some(trace),
        }) => TraceState::Ready(Run {
            cases: adapt::adapt(
                &trace,
                &key.source,
                key.structure.token(),
                key.root.as_deref(),
                None,
                key.structure.token(),
            )
            .map_err(|error| error.message()),
            memory: memory::project_all(&trace.steps),
            program_out,
            inputs: trace.inputs,
            waiting: trace.waiting,
            prompt: trace.prompt,
            error: trace.error,
        }),
    }
}

/// The crash surfaced honestly: stderr, else the compiler, else whatever the program printed.
fn no_trace_message(stderr: &str, compile_output: &str, program_out: &str) -> String {
    [stderr, compile_output, program_out]
        .iter()
        .find(|s| !s.trim().is_empty())
        .map_or_else(
            || "The run produced no trace.".to_owned(),
            |s| (*s).trim().to_owned(),
        )
}

#[cfg(test)]
mod tests;
