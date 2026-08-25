//! The edge-limit invariants. These are cheap unit tests guarding two numbers whose
//! relationship is easy to break by editing either one in isolation.

use super::{MAX_BODY_BYTES, REQUEST_TIMEOUT};
use crate::execution::domain::GO_JUDGE_LIMITS;
use crate::execution::infrastructure::GO_JUDGE_REQUEST_TIMEOUT;

/// The one that actually matters.
///
/// The runner waits 100 s on purpose, so go-judge's own clock limit fires first and the user
/// gets a clean TLE rather than an opaque HTTP timeout. If the edge timeout ever drops below
/// that, a legitimate slow run is killed at the door — and it would look like a flaky sandbox,
/// not a misconfigured router, which is the kind of bug that costs a day.
#[test]
fn the_edge_timeout_outlasts_the_longest_outbound_call() {
    assert!(
        REQUEST_TIMEOUT > GO_JUDGE_REQUEST_TIMEOUT,
        "the router's {REQUEST_TIMEOUT:?} timeout must exceed the go-judge runner's \
         {GO_JUDGE_REQUEST_TIMEOUT:?}, or a slow-but-valid run is cut off before the sandbox \
         can answer"
    );
}

/// The body limit is a backstop, not the gate. If it ever fell below what the application is
/// willing to accept, requests would be cut off by the transport with a bare 413 instead of
/// the application's explanatory one — the byte caps would still be enforced, but the user
/// would lose the message telling them which limit they hit.
#[test]
fn the_body_limit_leaves_room_for_the_largest_payload_the_app_accepts() {
    let largest_accepted = GO_JUDGE_LIMITS.max_source_bytes + GO_JUDGE_LIMITS.max_stdin_bytes;
    assert!(
        MAX_BODY_BYTES > largest_accepted * 2,
        "the {MAX_BODY_BYTES}-byte edge limit must leave room for {largest_accepted} bytes of \
         source+stdin plus JSON escaping, or the app's own 413 never gets to explain itself"
    );
}
