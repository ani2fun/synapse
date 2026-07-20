//! The trace session (oracle: `TraceSession`): wrap the code in the language's harness, run
//! it through the ORDINARY `/api/run` (no new endpoint — ADR-S029), decode the markers, and
//! adapt through the SAME shared pipeline the goldens pin. Cached per
//! (language, source, structure, root, stdin); Re-trace forces a fresh run. Every failure is
//! a Failed card, never a blank modal.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::viz::engine::adapt;
use crate::viz::engine::graph::VizCases;
use crate::viz::engine::vocabulary::VizStructure;
use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::execution::RunRequest;

use crate::api;
use crate::islands::tracer;
use crate::viz::engine::decoder::{self, Decoded};

#[derive(Clone, PartialEq)]
pub enum TraceState {
    Tracing,
    Ready(VizCases, String),
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
                    TraceState::Ready(cases, _) => {
                        crate::log::debug(&format!("trace ready: {} case(s)", cases.cases.len()));
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

/// Pure: run output → the modal's state (oracle: `TraceSession.outcome`).
fn outcome(key: &Key, stdout: &str, stderr: &str, compile_output: &str) -> TraceState {
    match decoder::decode(stdout) {
        Err(error) => TraceState::Failed(error.to_string()),
        Ok(Decoded {
            program_out,
            trace: None,
        }) => TraceState::Failed(no_trace_message(stderr, compile_output, &program_out)),
        Ok(Decoded {
            program_out,
            trace: Some(trace),
        }) => {
            match adapt::adapt(
                &trace,
                &key.source,
                key.structure.token(),
                key.root.as_deref(),
                None,
                key.structure.token(),
            ) {
                Ok(cases) => TraceState::Ready(cases, program_out),
                Err(error) => TraceState::Failed(error.message()),
            }
        }
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
