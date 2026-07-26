//! DTO↔domain mapping for the authoring surface, and the one place its errors become statuses.

use axum::Json;
use axum::http::StatusCode;
use chrono::SecondsFormat;
use synapse_shared::api::ApiError;
use synapse_shared::authoring::{EditRequestDto, EditSourceDto};
use synapse_shared::submission::AllowlistEntryDto;

use crate::authoring::application::{AuthoringError, ContentEditorEntry, EditableSource, Proposal};
use crate::authoring::domain::EditRequest;

pub type Reject = (StatusCode, Json<ApiError>);

pub fn to_source(source: EditableSource) -> EditSourceDto {
    EditSourceDto {
        lesson_path: source.lesson_path,
        file_path: source.file_path,
        source: source.source,
        fingerprint: source.fingerprint,
        repo: source.repo,
        base_branch: source.base_branch,
        content_version: source.content_version,
    }
}

/// A stored request. `reused`/`mode` describe the SUBMISSION that produced it, so a row read back
/// from the store (the account list) reports `reused: false` and the deployment's current mode —
/// neither is a property of the row.
pub fn to_request(request: &EditRequest, reused: bool, mode: &str) -> EditRequestDto {
    EditRequestDto {
        id: request.id.to_string(),
        lesson_path: request.lesson_path.clone(),
        file_path: request.file_path.clone(),
        branch: request.branch.clone(),
        state: request.state.as_str().to_owned(),
        pr_number: request.pull_request.as_ref().map(|pr| pr.number),
        pr_url: request.pull_request.as_ref().map(|pr| pr.url.clone()),
        commits: request.commits,
        reused,
        mode: mode.to_owned(),
        created_at: request.created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        updated_at: request.updated_at.to_rfc3339_opts(SecondsFormat::Millis, true),
    }
}

pub fn to_proposal(proposal: &Proposal) -> EditRequestDto {
    to_request(&proposal.request, proposal.reused, proposal.mode)
}

pub fn to_editor(entry: &ContentEditorEntry) -> AllowlistEntryDto {
    AllowlistEntryDto {
        username: entry.username.clone(),
        note: entry.note.clone(),
        granted_at: entry.granted_at.to_rfc3339_opts(SecondsFormat::Millis, true),
    }
}

/// The status mapping, stated once. The `hint` is where a contributor is told what to DO — an
/// error a non-technical reader hits mid-edit is worth a sentence more than a code.
pub fn to_error(error: &AuthoringError) -> Reject {
    let (status, message, hint) = match error {
        AuthoringError::NotEditable(_) => (
            StatusCode::NOT_FOUND,
            "Not an editable page",
            Some("Only lessons the library serves can be edited."),
        ),
        AuthoringError::RequiresSignIn => (
            StatusCode::UNAUTHORIZED,
            "Editing requires signing in",
            Some("Sign in, then reopen the editor."),
        ),
        AuthoringError::NotAllowed(_) => (
            StatusCode::FORBIDDEN,
            "Not a content editor",
            Some("Ask an admin to add you to the content-editor list."),
        ),
        AuthoringError::Invalid(_) => (StatusCode::BAD_REQUEST, "The proposed edit is not valid", None),
        AuthoringError::SourceMoved(_) => (
            StatusCode::CONFLICT,
            "The page changed while you were editing",
            Some("Copy your changes, reload the editor, and reapply them."),
        ),
        AuthoringError::ForgeUnavailable(_) => (
            StatusCode::BAD_GATEWAY,
            "The content repository is unreachable",
            Some("Your draft is saved in this browser — try submitting again shortly."),
        ),
        AuthoringError::ContentUnreadable(_) | AuthoringError::StoreFailed(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Editing is unavailable", None)
        }
    };
    (
        status,
        Json(ApiError {
            error: message.to_owned(),
            detail: Some(error.to_string()),
            hint: hint.map(str::to_owned),
        }),
    )
}

#[cfg(test)]
#[path = "dto_tests.rs"]
mod tests;
