//! Edge limits: a bound on how long a request may take and how large it may be.
//!
//! Axum applies neither by default beyond a 2 MB body cap, so a hung handler or a slow client
//! held a connection indefinitely. The application already caps what it will *process*
//! (`GO_JUDGE_LIMITS` — 64 KiB of source, 16 KiB of stdin, answered with a clean 413); these
//! are the transport-level backstops beneath that, for requests that never reach a handler.

use std::time::Duration;

use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;

/// The outer bound on a single request.
///
/// **This must stay larger than `execution::infrastructure::runner::REQUEST_TIMEOUT`.** The
/// runner deliberately waits 100 s so go-judge's own clock limit fires first — a cold
/// `scala-cli` compile can outlast 30 s, and a clean TLE beats an opaque HTTP timeout. A
/// global timeout below that would cut the connection while the sandbox was still working,
/// turning a legitimate slow run into a failure the user cannot interpret. The margin is the
/// point, not the number; `the_edge_timeout_outlasts_the_longest_outbound_call` locks it.
pub const REQUEST_TIMEOUT: Duration = Duration::from_mins(2);

/// The largest body the edge will read.
///
/// Deliberately generous against the app's own caps (64 KiB source + 16 KiB stdin, plus JSON
/// escaping which can roughly double a worst-case payload, plus the coach's conversation
/// history). This is not the size gate — that lives in the application and answers 413 with a
/// message. This only stops an unbounded read from ever reaching it.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Both layers, applied together.
pub fn apply(router: axum::Router) -> axum::Router {
    router
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        // 504, not tower-http's historical 408. A request that reaches this bound has almost
        // certainly been waiting on an upstream that blew past its OWN timeout — go-judge or
        // Ollama — so "gateway timed out" is the honest description. 408 would say the CLIENT
        // was slow to send, which is the one thing we know it was not.
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
