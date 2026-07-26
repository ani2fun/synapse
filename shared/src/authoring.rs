//! The content-editing wire contract: what the in-app markdown editor fetches, what it proposes,
//! and what came back. Wire-shaped, not domain — the request state travels as a plain string and
//! the forge's identity never crosses at all.
//!
//! The content-editor allowlist reuses `submission::AllowlistEntryDto` / `GrantRequestDto`: the
//! two lists differ in MEANING, not in shape, and one wire type keeps the admin panel's table a
//! single component rather than two identical ones.

use serde::{Deserialize, Serialize};

/// `GET /api/edits/config` — always answers, even when editing is off, so the client has one
/// place to ask instead of inferring capability from a 404.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EditConfigDto {
    /// `false` when the deployment has no forge configured — the editor never offers itself.
    pub enabled: bool,
    /// `"github"` (real pull requests) or `"dry-run"` (nothing leaves the process). The editor
    /// says which, plainly, rather than letting a contributor believe a dry run shipped.
    pub mode: String,
    /// `owner/name` of the PRIMARY content repository, and the branch it targets. A given
    /// lesson's actual target rides its `EditSourceDto` — this is the default the editor shows
    /// before a page is chosen.
    pub repo: String,
    pub base_branch: String,
    /// Whether THIS caller may propose edits — signed in and on the content-editor allowlist.
    /// UX only; every write re-checks server-side.
    pub can_edit: bool,
}

/// `GET /api/edits/source/{*path}` — the file as it is on disk RIGHT NOW.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EditSourceDto {
    /// The URL path (`category…/book/chapter…/lesson`).
    pub lesson_path: String,
    /// `owner/name` of the repository an edit to THIS page opens a pull request against, and the
    /// branch it targets. Per-lesson, not per-deployment: a satellite guide's book is edited in
    /// its own repository, and a contributor cannot infer that from the URL — so the editor says.
    pub repo: String,
    pub base_branch: String,
    /// The path INSIDE the content repository — real folders carry `NN-` order prefixes, so this
    /// is never derivable from `lesson_path` by the client.
    pub file_path: String,
    /// The WHOLE file, frontmatter fence included. Editing the reader's stripped body would
    /// delete the frontmatter on save.
    pub source: String,
    /// A digest of `source` the client hands back on submit, so an edit against a stale copy is
    /// refused instead of silently overwriting whatever landed in between.
    pub fingerprint: String,
    /// The content checkout's version (git SHA in prod) — shown as provenance.
    pub content_version: String,
}

/// `POST /api/edits` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProposeEditRequestDto {
    pub lesson_path: String,
    /// The proposed file, whole.
    pub source: String,
    /// The `fingerprint` that came with the source this edit started from.
    pub base_fingerprint: String,
    /// The contributor's own words — becomes the commit message body and the pull-request body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// One proposed change, as stored — the `POST` answer and the rows behind "My change requests".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct EditRequestDto {
    pub id: String,
    pub lesson_path: String,
    pub file_path: String,
    pub branch: String,
    /// `"open" | "merged" | "closed"`.
    pub state: String,
    /// Absent on a dry run — nothing was opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    /// How many commits this branch has carried — 2+ means the contributor revised an open
    /// proposal rather than opening a second one.
    pub commits: u32,
    /// `true` when this submission landed on an ALREADY-OPEN pull request.
    pub reused: bool,
    /// The forge that handled it (`"github"` / `"dry-run"`), so the result copy can be honest.
    pub mode: String,
    /// ISO-8601 instants.
    pub created_at: String,
    pub updated_at: String,
}
