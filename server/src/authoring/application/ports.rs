//! The four ports the authoring use cases drive, and the error every one of them speaks.
//!
//! Each is use-case shaped rather than technology shaped: `ContentForge` says "commit this file,
//! open a pull request", not "PUT /contents, POST /pulls". That is what lets a dry-run adapter be
//! a first-class citizen instead of a mock, and what would let a GitHub App (or a different forge
//! entirely) slot in without the service noticing.

use std::sync::Arc;

use crate::authoring::application::ForgeTarget;
use crate::authoring::domain::validation::InvalidEdit;
use crate::authoring::domain::{EditRequest, EditRequestState, PullRequestRef};
use crate::identity::domain::Username;

/// HTTP mapping (at `http/`): `NotEditable`→404, `RequiresSignIn`→401, `NotAllowed`→403,
/// `Rejected`/`NoChange`→400, `SourceMoved`→409, `ForgeUnavailable`→502,
/// `StoreFailed`/`ContentUnreadable`→500.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthoringError {
    #[error("'{0}' is not an editable lesson")]
    NotEditable(String),
    #[error("proposing an edit requires signing in")]
    RequiresSignIn,
    #[error("'{0}' is not on the content-editor allowlist")]
    NotAllowed(String),
    /// A guardrail refused the text. The [`InvalidEdit`] is carried WHOLE rather than flattened
    /// to a sentence here, because each rule has a different remedy and only the edge knows how
    /// to say it: someone who deleted the frontmatter needs different words from someone who
    /// pasted a 300 KiB file. Flattening at the point of failure would leave the edge with one
    /// generic apology for four unrelated mistakes.
    #[error("the proposed edit is not valid: {0}")]
    Rejected(#[from] InvalidEdit),
    /// The proposal matches the file it started from. Not an [`InvalidEdit`] — that type judges a
    /// proposal against the rules, and this compares it against the current file, which only the
    /// use case can see.
    #[error("the proposed file is identical to the current one")]
    NoChange,
    /// The file changed on disk since the editor loaded it — committing would silently discard
    /// whatever landed in between, so the contributor is asked to reload instead.
    #[error("'{0}' changed since you started editing — reload the page and reapply your change")]
    SourceMoved(String),
    #[error("the content repository is unreachable: {0}")]
    ForgeUnavailable(String),
    #[error("the content tree could not be read: {0}")]
    ContentUnreadable(String),
    #[error("the edit-request store failed: {0}")]
    StoreFailed(String),
}

/// The verified caller, projected for authoring: `username` is the allowlist key and the
/// branch's owner segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    pub username: Username,
}

/// A lesson's editable source (`LessonSource`).
///
/// The WHOLE file, frontmatter fence included — the reader's payload carries the body with the
/// fence stripped, and editing that would delete the frontmatter on save. `None` means the path
/// is not a lesson the catalog serves, which is also what keeps `local-only/`, `_`-prefixed files
/// and the reserved aux dirs structurally uneditable.
pub struct LessonFile {
    /// Which content source owns this lesson — what decides the repository an edit targets.
    pub source_id: String,
    /// The path inside that source's repository, order prefixes and all.
    pub file_path: String,
    pub source: String,
}

pub trait LessonSource: Send + Sync {
    fn file_for(
        &self,
        lesson_path: &[String],
    ) -> impl Future<Output = Result<Option<LessonFile>, AuthoringError>> + Send;

    /// The content checkout's version (the git SHA in production). Shown to the contributor as
    /// provenance — which snapshot of the library they are editing against. It lives on this port
    /// rather than being threaded in from the catalog service because it describes the same tree
    /// `file_for` reads, and one port answering both keeps them from ever disagreeing.
    fn content_version(&self) -> impl Future<Output = String> + Send;
}

/// A shared forge is still a forge. `Forges::forge_for` hands one back BY VALUE, and the adapters
/// that mint a fresh one per call are cheap — but a recording test double is not, and neither is
/// anything that wants to keep state across calls. Delegating through `Arc` lets those be shared
/// instead of cloned.
impl<T: ContentForge> ContentForge for Arc<T> {
    fn mode(&self) -> &'static str {
        T::mode(self)
    }
    fn commit_file(
        &self,
        branch: &str,
        file_path: &str,
        content: &str,
        message: &str,
    ) -> impl Future<Output = Result<String, AuthoringError>> + Send {
        T::commit_file(self, branch, file_path, content, message)
    }
    fn open_pull_request(
        &self,
        branch: &str,
        title: &str,
        body: &str,
    ) -> impl Future<Output = Result<Option<PullRequestRef>, AuthoringError>> + Send {
        T::open_pull_request(self, branch, title, body)
    }
    fn pull_request_state(
        &self,
        number: u64,
    ) -> impl Future<Output = Result<ForgePrState, AuthoringError>> + Send {
        T::pull_request_state(self, number)
    }
}

/// Which forge an edit goes to.
///
/// Two lookups rather than one, and the difference is load-bearing. A NEW proposal is routed by
/// the source that currently owns the lesson; a REVISION follows the repository already recorded
/// on its row. If a book migrates out of the monorepo while a pull request is open, those two
/// disagree — and the open branch lives in the repository it was opened against, not the one that
/// now serves the page.
pub trait Forges: Send + Sync {
    type Forge: ContentForge;

    /// Where a content source's edits go. `None` for a source with no configured forge.
    fn target_for(&self, source_id: &str) -> impl Future<Output = Option<ForgeTarget>> + Send;

    /// The forge that commits to a repository.
    fn forge_for(&self, repo: &str) -> impl Future<Output = Option<Self::Forge>> + Send;
}

/// One grant, as stored. `username` stays a plain `String` — a row read back for display, whose
/// stored spelling is worth showing rather than quietly correcting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentEditorEntry {
    pub username: String,
    pub note: Option<String>,
    pub granted_at: chrono::DateTime<chrono::Utc>,
}

/// Who may propose content changes (`ContentEditors`) — deliberately NOT the submit allowlist.
/// Four verbs, one capability: the probe the propose gate rides plus the admin panel's management.
///
/// [`Username`] rather than `&str` for the same reason the submit allowlist takes one: a grant
/// and the probe that reads it must be spelled by the same constructor, or the row is written
/// somewhere the gate never looks.
pub trait ContentEditors: Send + Sync {
    fn is_allowed(&self, username: &Username) -> impl Future<Output = Result<bool, AuthoringError>> + Send;
    /// Newest grant first (`granted_at desc, username`).
    fn list(&self) -> impl Future<Output = Result<Vec<ContentEditorEntry>, AuthoringError>> + Send;
    /// Upsert — re-granting refreshes the note; returns the stored row.
    fn grant(
        &self,
        username: &Username,
        note: Option<&str>,
    ) -> impl Future<Output = Result<ContentEditorEntry, AuthoringError>> + Send;
    /// `false` when there was nothing to revoke.
    fn revoke(&self, username: &Username) -> impl Future<Output = Result<bool, AuthoringError>> + Send;
}

/// Where proposals are recorded (`EditRequestRepository`).
pub trait EditRequestRepository: Send + Sync {
    /// The contributor's still-open proposal for this page, if any — the reuse probe.
    fn open_for(
        &self,
        username: &str,
        lesson_path: &str,
    ) -> impl Future<Output = Result<Option<EditRequest>, AuthoringError>> + Send;
    /// The highest `attempt` this contributor has ever used on this page (0 when none), so the
    /// next branch can be allocated without scanning the forge's refs.
    fn highest_attempt(
        &self,
        username: &str,
        lesson_path: &str,
    ) -> impl Future<Output = Result<u32, AuthoringError>> + Send;
    fn save(&self, request: &EditRequest) -> impl Future<Output = Result<(), AuthoringError>> + Send;
    fn update(&self, request: &EditRequest) -> impl Future<Output = Result<(), AuthoringError>> + Send;
    /// Every proposal of one contributor, newest first — the account page's list.
    fn list_for(
        &self,
        username: &str,
    ) -> impl Future<Output = Result<Vec<EditRequest>, AuthoringError>> + Send;
}

/// What a forge reports about a pull request. `Missing` covers a pull request that was deleted
/// outright, and settles the same way a close does: do not reuse it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgePrState {
    Open,
    Merged,
    Closed,
    Missing,
}

impl ForgePrState {
    pub fn settled(self) -> EditRequestState {
        match self {
            Self::Open => EditRequestState::Open,
            Self::Merged => EditRequestState::Merged,
            Self::Closed | Self::Missing => EditRequestState::Closed,
        }
    }
}

/// The git host (`ContentForge`). `github` in production, `dry-run` where no token is configured.
pub trait ContentForge: Send + Sync {
    /// Which adapter answered — surfaced to the client so a dry run never reads as a real
    /// pull request.
    fn mode(&self) -> &'static str;

    /// Commit `content` at `file_path` on `branch`, creating the branch off the default branch
    /// when it does not exist yet. Returns the commit id.
    fn commit_file(
        &self,
        branch: &str,
        file_path: &str,
        content: &str,
        message: &str,
    ) -> impl Future<Output = Result<String, AuthoringError>> + Send;

    /// Open a pull request from `branch` into the default branch. `None` means the adapter opens
    /// nothing (the dry run) — the branch is still the artifact, there is just no pull request to
    /// point at.
    ///
    /// IDEMPOTENT: when a pull request for `branch` is already open, that one comes back rather
    /// than an error. The service commits before it records, so a store failure between the two
    /// must leave the next attempt able to recover rather than colliding forever.
    fn open_pull_request(
        &self,
        branch: &str,
        title: &str,
        body: &str,
    ) -> impl Future<Output = Result<Option<PullRequestRef>, AuthoringError>> + Send;

    /// Where a pull request stands now. The stored state is a cache; this is the authority,
    /// because a proposal can be merged or closed on the forge without Synapse ever hearing.
    fn pull_request_state(
        &self,
        number: u64,
    ) -> impl Future<Output = Result<ForgePrState, AuthoringError>> + Send;
}
