//! Domain → wire flatten (oracle: `SubmissionDtos`). Only the FIRST failing case crosses;
//! verdict strings are the oracle's exact vocabulary.

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

/// `NotAProblem`/`UnknownSubmission`→404 · `NotYours`→403 · `InvalidSuite`/`StoreFailed`→500.
pub fn to_error(error: &SubmissionError) -> (StatusCode, Json<ApiError>) {
    let (status, message) = match error {
        SubmissionError::NotAProblem(_) => (StatusCode::NOT_FOUND, "Not a problem"),
        SubmissionError::UnknownSubmission(_) => (StatusCode::NOT_FOUND, "Unknown submission"),
        SubmissionError::NotYours(_) => (StatusCode::FORBIDDEN, "Not your submission"),
        SubmissionError::InvalidSuite { .. } => {
            (StatusCode::INTERNAL_SERVER_ERROR, "The authored suite is invalid")
        }
        SubmissionError::StoreFailed(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Submission store failed"),
    };
    (
        status,
        Json(ApiError {
            error: message.to_owned(),
            detail: Some(error.to_string()),
            hint: None,
        }),
    )
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
