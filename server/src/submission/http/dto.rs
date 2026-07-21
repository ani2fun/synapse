//! Domain → wire flatten. Only the FIRST failing case crosses; verdict strings are a fixed
//! vocabulary the workbench matches on.

use axum::Json;
use axum::http::StatusCode;
use chrono::SecondsFormat;
use synapse_shared::api::ApiError;
use synapse_shared::submission::{FailedCaseDto, SubmissionDto};

use crate::submission::application::SubmissionError;
use crate::submission::domain::{Submission, SubmissionState, SuiteOutcome};

pub fn to_dto(submission: &Submission) -> SubmissionDto {
    let (status, verdict, passed, total, detail, first_failure, completed_at) = match &submission.state {
        SubmissionState::Pending => ("pending", None, None, None, None, None, None),
        SubmissionState::Judging => ("judging", None, None, None, None, None, None),
        SubmissionState::Completed { outcome, at } => {
            let at = Some(at.to_rfc3339_opts(SecondsFormat::Millis, true));
            match outcome {
                SuiteOutcome::Accepted { total } => (
                    "completed",
                    Some("accepted"),
                    Some(*total),
                    Some(*total),
                    None,
                    None,
                    at,
                ),
                SuiteOutcome::Rejected {
                    passed,
                    total,
                    first_failure,
                } => (
                    "completed",
                    Some("rejected"),
                    Some(*passed),
                    Some(*total),
                    None,
                    Some(FailedCaseDto {
                        index: first_failure.index,
                        args: first_failure.args.clone(),
                        expected: first_failure.expected.clone(),
                        stdout: first_failure.stdout.clone(),
                        stderr: first_failure.stderr.clone(),
                        run_status: format!("{:?}", first_failure.status),
                    }),
                    at,
                ),
                SuiteOutcome::JudgeFailed {
                    passed,
                    total,
                    detail,
                } => (
                    "completed",
                    Some("judge-failed"),
                    Some(*passed),
                    Some(*total),
                    Some(detail.clone()),
                    None,
                    at,
                ),
            }
        }
    };
    SubmissionDto {
        id: submission.id.to_string(),
        path: submission.lesson_path.clone(),
        language: submission.language.clone(),
        source: submission.source.clone(),
        created_at: submission.created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        status: status.to_owned(),
        verdict: verdict.map(str::to_owned),
        passed,
        total,
        detail,
        first_failure,
        completed_at,
    }
}

/// `NotAProblem`/`UnknownSubmission`→404 · `NotYours`→403 · `SubmitRequiresSignIn`→401 ·
/// `NotAllowlisted`→403 · `InvalidSuite`/`StoreFailed`→500. The allowlist copy is fixed
/// wording — the workbench renders `error — detail` verbatim.
pub fn to_error(error: &SubmissionError) -> (StatusCode, Json<ApiError>) {
    // The common shape: a headline, with the variant's own Display as the detail.
    let plain = |status: StatusCode, headline: &str| {
        (
            status,
            ApiError {
                error: headline.to_owned(),
                detail: Some(error.to_string()),
                hint: None,
            },
        )
    };

    // TOTAL, and deliberately so: every variant decides its status AND its body in exactly one
    // arm. Answering a variant before the match would leave a second, unreachable arm here —
    // and then dropping that early return would silently downgrade a refusal to a 500 with the
    // exhaustiveness check none the wiser.
    let (status, body) = match error {
        SubmissionError::NotAProblem(_) => plain(StatusCode::NOT_FOUND, "Not a problem"),
        SubmissionError::UnknownSubmission(_) => plain(StatusCode::NOT_FOUND, "Unknown submission"),
        SubmissionError::NotYours(_) => plain(StatusCode::FORBIDDEN, "Not your submission"),
        SubmissionError::InvalidSuite { .. } => {
            plain(StatusCode::INTERNAL_SERVER_ERROR, "The authored suite is invalid")
        }
        SubmissionError::StoreFailed(_) => {
            plain(StatusCode::INTERNAL_SERVER_ERROR, "Submission store failed")
        }

        // The two refusals answer "why not?" rather than "what broke?", so they carry fixed copy
        // the workbench renders verbatim instead of the Display string.
        SubmissionError::SubmitRequiresSignIn => (
            StatusCode::UNAUTHORIZED,
            ApiError {
                error: "Sign in to submit".to_owned(),
                detail: Some(
                    "Submitting runs your code against every hidden case and saves the attempt".to_owned(),
                ),
                hint: None,
            },
        ),
        SubmissionError::NotAllowlisted(username) => (
            StatusCode::FORBIDDEN,
            ApiError {
                error: "Submitting is allow-listed on this deployment".to_owned(),
                detail: Some(format!(
                    "'{username}' isn't on the allowlist yet — saving uses shared compute + storage"
                )),
                hint: Some("Request access from the operator, or self-host your own instance".to_owned()),
            },
        ),
    };
    (status, Json(body))
}

pub fn bad_id(raw: &str) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiError {
            error: "Not a submission id".to_owned(),
            detail: Some(format!("'{raw}' is not a UUID")),
            hint: None,
        }),
    )
}

#[cfg(test)]
#[path = "dto_tests.rs"]
mod tests;
