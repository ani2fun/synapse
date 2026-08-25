//! Pins the wire shape of every `SubmissionError` → HTTP mapping.
//!
//! These exist because the mapping is the one place a refactor can change the API without
//! changing a signature: status and body are values, not types, so nothing else would notice.
//! Every variant is covered, so `to_error`'s match staying total is checked by the compiler
//! and its OUTPUT is checked here.

use axum::Json;
use axum::http::StatusCode;

use super::to_error;
use crate::submission::application::SubmissionError;

// ─────────────────────────────────────────────────────────────────────────────
// THE RICH BODIES
// The two auth refusals carry copy the workbench renders verbatim — the reader
// sees `error — detail`, so the wording is API surface, not a log line.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn anonymous_submit_is_401_with_the_why() {
    let (status, Json(body)) = to_error(&SubmissionError::SubmitRequiresSignIn);

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body.error, "Sign in to submit");
    assert_eq!(
        body.detail.as_deref(),
        Some("Submitting runs your code against every hidden case and saves the attempt")
    );
    assert_eq!(body.hint, None);
}

#[test]
fn a_non_allowlisted_user_is_403_naming_them_and_the_way_out() {
    let (status, Json(body)) = to_error(&SubmissionError::NotAllowlisted("stranger".to_owned()));

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body.error, "Submitting is allow-listed on this deployment");
    assert_eq!(
        body.detail.as_deref(),
        Some("'stranger' isn't on the allowlist yet — saving uses shared compute + storage")
    );
    assert_eq!(
        body.hint.as_deref(),
        // Verbatim, and deliberately so: this sentence is a second copy of the one in
        // `web/src/lib/contact.ts`, and asserting it here is what stops the two drifting.
        Some(
            "To request one, email your GitHub username to synapse.kakde.eu@gmail.com — access \
             may or may not be granted. Or self-host your own instance."
        )
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PLAIN BODIES
// Everything else reports the variant's Display as `detail` and carries no hint.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn not_a_problem_is_404() {
    let error = SubmissionError::NotAProblem("dsa/two-sum".to_owned());
    let (status, Json(body)) = to_error(&error);

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body.error, "Not a problem");
    assert_eq!(body.detail.as_deref(), Some(error.to_string().as_str()));
    assert_eq!(body.hint, None);
}

#[test]
fn an_unknown_submission_is_404() {
    let error = SubmissionError::UnknownSubmission("nope".to_owned());
    let (status, Json(body)) = to_error(&error);

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body.error, "Unknown submission");
    assert_eq!(body.detail.as_deref(), Some(error.to_string().as_str()));
    assert_eq!(body.hint, None);
}

#[test]
fn someone_elses_submission_is_403() {
    let error = SubmissionError::NotYours("abc".to_owned());
    let (status, Json(body)) = to_error(&error);

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body.error, "Not your submission");
    assert_eq!(body.detail.as_deref(), Some(error.to_string().as_str()));
    assert_eq!(body.hint, None);
}

/// An author's broken suite is MY bug, not the caller's — 500 keeps it in the error rate
/// instead of blaming a perfectly valid request.
#[test]
fn an_invalid_authored_suite_is_500() {
    let error = SubmissionError::InvalidSuite {
        path: "dsa/two-sum".to_owned(),
        detail: "expected an array".to_owned(),
    };
    let (status, Json(body)) = to_error(&error);

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body.error, "The authored suite is invalid");
    assert_eq!(body.detail.as_deref(), Some(error.to_string().as_str()));
    assert_eq!(body.hint, None);
}

#[test]
fn a_failed_store_is_500() {
    let error = SubmissionError::StoreFailed("connection reset".to_owned());
    let (status, Json(body)) = to_error(&error);

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body.error, "Submission store failed");
    assert_eq!(body.detail.as_deref(), Some(error.to_string().as_str()));
    assert_eq!(body.hint, None);
}

// ─────────────────────────────────────────────────────────────────────────────
// NO VARIANT FALLS THROUGH
// The guard against the shape this file was written for: an auth refusal that
// silently degrades into a generic 500.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn no_variant_maps_to_a_generic_store_failure_by_accident() {
    for error in [
        SubmissionError::SubmitRequiresSignIn,
        SubmissionError::NotAllowlisted("stranger".to_owned()),
    ] {
        let (status, Json(body)) = to_error(&error);
        assert_ne!(
            status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "an auth refusal must never present as a server fault: {error:?}"
        );
        assert_ne!(body.error, "Submission store failed", "{error:?}");
    }
}
