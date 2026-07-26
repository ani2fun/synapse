//! The `EditRequest` aggregate: one contributor's proposed change to one page, and the branch +
//! pull request carrying it.

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditRequestId(pub Uuid);

impl std::fmt::Display for EditRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Where a proposal stands on the forge. `Open` is the ONLY reusable state — the whole
/// "another edit becomes another commit" rule turns on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditRequestState {
    Open,
    Merged,
    Closed,
}

impl EditRequestState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }

    /// Anything unrecognised reads as `Closed`: a row we cannot interpret must not be reused,
    /// and refusing to reuse it costs one extra branch, while wrongly reusing it would push
    /// commits onto a proposal nobody is watching.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "open" => Self::Open,
            "merged" => Self::Merged,
            _ => Self::Closed,
        }
    }

    pub fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }
}

/// The pull request a proposal lives on. Absent on a dry run — the branch is still recorded so
/// the flow is exercisable without credentials, but there is no pull request to point at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestRef {
    pub number: u64,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditRequest {
    pub id: EditRequestId,
    /// Lowercase IdP username — the allowlist key and the branch's owner segment.
    pub username: String,
    /// The URL path, joined (`category…/book/chapter…/lesson`).
    pub lesson_path: String,
    /// The path inside that repository.
    pub file_path: String,
    /// The repository this proposal was opened against, `owner/name`. Recorded rather than
    /// re-derived: a revision must follow the branch to where it actually lives, even if the book
    /// has since moved to a different source.
    pub repo: String,
    pub branch: String,
    /// 1 for the first proposal on this page, 2 after the first was merged or closed, and so on.
    /// It is what puts the `-2`/`-3` suffix on the branch.
    pub attempt: u32,
    pub pull_request: Option<PullRequestRef>,
    pub state: EditRequestState,
    /// How many commits this branch has carried; 2+ means a revision of an open proposal.
    pub commits: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Where a proposal lives: the page it edits, the file behind it, and the branch in the repository
/// it was opened against. Grouped because they travel together and are only meaningful together —
/// a branch without its repository does not identify anything now that a book can have its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalLocation {
    pub lesson_path: String,
    pub file_path: String,
    pub repo: String,
    pub branch: String,
}

impl EditRequest {
    /// A freshly-allocated proposal: one commit, open, no pull request attached yet.
    pub fn opened(
        id: EditRequestId,
        username: String,
        location: ProposalLocation,
        attempt: u32,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            username,
            lesson_path: location.lesson_path,
            file_path: location.file_path,
            repo: location.repo,
            branch: location.branch,
            attempt,
            pull_request: None,
            state: EditRequestState::Open,
            commits: 1,
            created_at: at,
            updated_at: at,
        }
    }

    /// Attach the pull request the forge just opened.
    #[must_use]
    pub fn with_pull_request(mut self, pull_request: PullRequestRef, at: DateTime<Utc>) -> Self {
        self.pull_request = Some(pull_request);
        self.updated_at = at;
        self
    }

    /// A revision landed on this proposal's branch.
    #[must_use]
    pub fn revised(mut self, at: DateTime<Utc>) -> Self {
        self.commits = self.commits.saturating_add(1);
        self.updated_at = at;
        self
    }

    /// The forge says this proposal is no longer open — record it so the next edit allocates a
    /// fresh branch instead of committing onto something nobody is reviewing.
    #[must_use]
    pub fn settled(mut self, state: EditRequestState, at: DateTime<Utc>) -> Self {
        self.state = state;
        self.updated_at = at;
        self
    }
}
