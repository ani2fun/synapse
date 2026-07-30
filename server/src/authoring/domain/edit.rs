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

/// One contributor's proposal, opaque: read through the accessors, write only through the named
/// transitions below.
///
/// The fields fall into two classes and the store already knew it — `update` writes exactly five
/// columns and calls the rest "the row's identity". Everything above `pull_request` is fixed at
/// [`EditRequest::opened`] and never moves again; everything from it down belongs to a transition.
/// With the fields public that was a convention, and the difference between adding a commit and
/// silently repointing a branch at another repository was one assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditRequest {
    id: EditRequestId,
    /// The allowlist key and the branch's owner segment.
    username: String,
    /// The page, the file, the repository and the branch — kept as the value object they arrive
    /// as rather than splatted into four fields, because a branch without its repository does not
    /// identify anything.
    location: ProposalLocation,
    /// 1 for the first proposal on this page, 2 after the first was merged or closed, and so on.
    /// It is what puts the `-2`/`-3` suffix on the branch.
    attempt: u32,
    pull_request: Option<PullRequestRef>,
    state: EditRequestState,
    /// How many commits this branch has carried; 2+ means a revision of an open proposal.
    commits: u32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Where a proposal lives: the page it edits, the file behind it, and the branch in the repository
/// it was opened against. Grouped because they travel together and are only meaningful together —
/// a branch without its repository does not identify anything now that a book can have its own.
///
/// Fields stay public: this is a value object with no transitions to protect, and it is only ever
/// built whole and read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalLocation {
    /// The URL path, joined (`category…/book/chapter…/lesson`).
    pub lesson_path: String,
    /// The path inside the repository.
    pub file_path: String,
    /// `owner/name`. Recorded rather than re-derived: a revision must follow the branch to where
    /// it actually lives, even if the book has since moved to a different source.
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
            location,
            attempt,
            pull_request: None,
            state: EditRequestState::Open,
            commits: 1,
            created_at: at,
            updated_at: at,
        }
    }

    /// Put a rehydrated proposal back where its history left it. The store's door, and the only
    /// other way to reach the transition-owned fields.
    ///
    /// Four arguments of four different types on purpose: a flat constructor taking the whole row
    /// would sit `attempt` beside `commits` as two bare `u32`s, and swapping them compiles.
    #[must_use]
    pub fn restored(
        mut self,
        pull_request: Option<PullRequestRef>,
        state: EditRequestState,
        commits: u32,
        updated_at: DateTime<Utc>,
    ) -> Self {
        self.pull_request = pull_request;
        self.state = state;
        self.commits = commits;
        self.updated_at = updated_at;
        self
    }

    #[must_use]
    pub fn id(&self) -> EditRequestId {
        self.id
    }
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
    #[must_use]
    pub fn location(&self) -> &ProposalLocation {
        &self.location
    }
    #[must_use]
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
    #[must_use]
    pub fn pull_request(&self) -> Option<&PullRequestRef> {
        self.pull_request.as_ref()
    }
    #[must_use]
    pub fn state(&self) -> EditRequestState {
        self.state
    }
    #[must_use]
    pub fn commits(&self) -> u32 {
        self.commits
    }
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    #[must_use]
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
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
