//! The submission wire contract (oracle: `shared/submission/SubmissionApi.scala`, code-first).
//! Wire-shaped, not domain: status/verdict travel as plain strings, the outcome flattened, and
//! only the FIRST failing case ever crosses.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// `POST /api/submissions` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SubmitRequestDto {
    pub path: Vec<String>,
    pub language: String,
    pub source: String,
}

/// The 202 body — poll `GET /api/submissions/{id}` until `status == "completed"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SubmissionAcceptedDto {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FailedCaseDto {
    pub index: usize,
    pub args: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub run_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SubmissionDto {
    pub id: String,
    pub path: Vec<String>,
    pub language: String,
    pub source: String,
    /// ISO-8601 instant.
    pub created_at: String,
    /// `"pending" | "judging" | "completed"`.
    pub status: String,
    /// `"accepted" | "rejected" | "judge-failed"` — present when completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passed: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    /// The `judge-failed` machinery message only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Rejections only — the one revealed case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_failure: Option<FailedCaseDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

/// Delete/erase result — 0/1 for a single delete, N for erase-all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeleteResultDto {
    pub deleted: usize,
}

/// One allowlist grant (`/api/admin/allowlist`, step 21).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AllowlistEntryDto {
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// ISO-8601 instant.
    pub granted_at: String,
}

/// The grant request — username is trimmed + lowercased server-side (canonical, matching the
/// verifier); blank is a 400.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct GrantRequestDto {
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
