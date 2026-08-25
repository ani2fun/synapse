//! The authoring use cases (`ProposeEdit`): fetch a lesson's editable source, and turn a proposed
//! rewrite into a commit on a per-contributor, per-page branch with a pull request behind it.
//!
//! THE REUSE RULE, which is the whole point of the context: a contributor who edits the same page
//! twice while their pull request is still open adds a COMMIT to it — not a second pull request.
//! Once that one is merged or closed, the next edit starts a fresh branch (`…-2`, `…-3`). The
//! stored state is only a cache; the forge is asked before anything is reused, because a proposal
//! can be merged or closed there without Synapse hearing a thing.

mod message;
mod ports;

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::authoring::domain::branch::branch_for;
use crate::authoring::domain::validation::{fingerprint, normalise, validate};
use crate::authoring::domain::{EditRequest, EditRequestId, ProposalLocation};
use crate::catalog::domain::content_tree::PRIMARY_SOURCE_ID;
use crate::identity::domain::Username;

pub use ports::{
    AuthoringError, ContentEditorEntry, ContentEditors, ContentForge, EditRequestRepository, Editor,
    ForgePrState, Forges, LessonFile, LessonSource,
};

/// A lesson's source plus the provenance the editor shows and hands back on submit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditableSource {
    pub lesson_path: String,
    /// Which repository an edit to THIS page opens a pull request against. Per-lesson because a
    /// satellite's book is edited in its own repository — a contributor cannot infer it from the
    /// URL, so the editor has to say.
    pub repo: String,
    pub base_branch: String,
    pub file_path: String,
    pub source: String,
    pub fingerprint: String,
    pub content_version: String,
}

/// What a submission produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub request: EditRequest,
    /// `true` when this landed on an already-open pull request.
    pub reused: bool,
    pub mode: &'static str,
}

/// Where the deployment proposes changes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ForgeTarget {
    /// `owner/name`.
    pub repo: String,
    pub base_branch: String,
    /// The public origin, for the live-page link in a pull-request body.
    pub site_url: String,
}

pub struct ProposeEdit<Source, Editors, Repo, Forge> {
    source: Arc<Source>,
    editors: Arc<Editors>,
    repo: Arc<Repo>,
    /// Every configured forge, selected per edit. A satellite guide repo's lesson opens its pull
    /// request against THAT repository, not the monorepo.
    forges: Arc<Forge>,
}

impl<Source, Editors, Repo, Forge> ProposeEdit<Source, Editors, Repo, Forge>
where
    Source: LessonSource,
    Editors: ContentEditors,
    Repo: EditRequestRepository,
    Forge: Forges,
{
    /// The forge a stored proposal's branch lives in. A row whose repository is no longer
    /// configured is an honest failure: the branch exists somewhere we can no longer reach.
    async fn forge_at(&self, repo: &str) -> Result<Forge::Forge, AuthoringError> {
        self.forges
            .forge_for(repo)
            .await
            .ok_or_else(|| AuthoringError::ContentUnreadable(format!("no forge configured for '{repo}'")))
    }

    /// Where a source's edits go.
    async fn target_of(&self, source_id: &str) -> Result<ForgeTarget, AuthoringError> {
        self.forges.target_for(source_id).await.ok_or_else(|| {
            AuthoringError::ContentUnreadable(format!("no forge configured for source '{source_id}'"))
        })
    }

    pub fn new(source: Arc<Source>, editors: Arc<Editors>, repo: Arc<Repo>, forges: Arc<Forge>) -> Self {
        Self {
            source,
            editors,
            repo,
            forges,
        }
    }

    /// The DEFAULT target, for `/api/edits/config` — the primary checkout's repository. A given
    /// lesson's actual target rides its `EditSourceDto`, because it depends on which source owns
    /// the page.
    pub async fn target(&self) -> ForgeTarget {
        self.forges
            .target_for(PRIMARY_SOURCE_ID)
            .await
            .unwrap_or_default()
    }

    pub async fn mode(&self) -> &'static str {
        let repo = self.target().await.repo;
        match self.forges.forge_for(&repo).await {
            Some(forge) => forge.mode(),
            None => "off",
        }
    }

    /// The UX bit behind `/api/edits/config`'s `canEdit`. Anonymous is `false`, never an error —
    /// the config endpoint answers for everyone.
    pub async fn may_edit(&self, editor: Option<&Editor>) -> Result<bool, AuthoringError> {
        match editor {
            None => Ok(false),
            Some(editor) => self.editors.is_allowed(&editor.username).await,
        }
    }

    /// Anonymous → 401, verified but not granted → 403. Runs FIRST on every verb, so a refusal
    /// never touches the content tree or the forge.
    async fn authorize(&self, editor: Option<&Editor>) -> Result<Username, AuthoringError> {
        let editor = editor.ok_or(AuthoringError::RequiresSignIn)?;
        if self.editors.is_allowed(&editor.username).await? {
            Ok(editor.username.clone())
        } else {
            Err(AuthoringError::NotAllowed(editor.username.to_string()))
        }
    }

    /// The file to edit, whole — frontmatter fence included.
    #[tracing::instrument(name = "authoring.source", skip(self, editor), fields(path = %lesson_path.join("/")))]
    pub async fn source_for(
        &self,
        editor: Option<&Editor>,
        lesson_path: &[String],
    ) -> Result<EditableSource, AuthoringError> {
        self.authorize(editor).await?;
        let joined = lesson_path.join("/");
        let file = self
            .source
            .file_for(lesson_path)
            .await?
            .ok_or_else(|| AuthoringError::NotEditable(joined.clone()))?;
        let target = self.target_of(&file.source_id).await?;
        Ok(EditableSource {
            lesson_path: joined,
            repo: target.repo,
            base_branch: target.base_branch,
            file_path: file.file_path,
            fingerprint: fingerprint(&file.source),
            source: file.source,
            content_version: self.source.content_version().await,
        })
    }

    /// Every proposal this contributor has made, newest first. The stored `state` is a cache
    /// refreshed on submit — listing does not spend a forge round-trip per row.
    pub async fn mine(&self, editor: Option<&Editor>) -> Result<Vec<EditRequest>, AuthoringError> {
        let username = self.authorize(editor).await?;
        self.repo.list_for(username.as_str()).await
    }

    /// Validate, then commit — reusing the open proposal for this page when there is one.
    ///
    /// ORDER: commit, open the pull request, THEN record. The three are not atomic and cannot be,
    /// so the order is chosen for what a failure between them leaves behind. Recording last means
    /// a store failure leaves no row claiming to be a proposal, and the next attempt re-derives
    /// the same branch, commits onto it, and gets the same pull request back (the forge port
    /// promises that idempotence) — so it self-heals rather than colliding.
    #[tracing::instrument(
        name = "authoring.propose",
        skip(self, editor, source, summary),
        fields(path = %lesson_path.join("/"), source_bytes = source.len())
    )]
    pub async fn propose(
        &self,
        editor: Option<&Editor>,
        lesson_path: &[String],
        source: &str,
        base_fingerprint: &str,
        summary: Option<&str>,
    ) -> Result<Proposal, AuthoringError> {
        let username = self.authorize(editor).await?;
        let joined = lesson_path.join("/");
        let file = self
            .source
            .file_for(lesson_path)
            .await?
            .ok_or_else(|| AuthoringError::NotEditable(joined.clone()))?;

        // The drift guard. The editor may have been open for an hour; committing a rewrite of a
        // file that moved in the meantime would silently discard whatever landed in between.
        if fingerprint(&file.source) != base_fingerprint {
            tracing::info!(path = %joined, "edit refused — the source moved under the editor");
            return Err(AuthoringError::SourceMoved(joined));
        }

        // `?` rather than a `map_err` that stringifies: which rule was broken survives to the
        // edge, where it becomes the sentence telling the contributor what to fix.
        let content = validate(&file.source, source)?;
        if content == normalise(&file.source) {
            return Err(AuthoringError::NoChange);
        }

        // Past the gate the name is a plain string again: what follows formats it into a commit
        // message and a branch segment, neither of which is a comparison the spelling can break.
        let username = username.as_str();
        let message = message::commit_message(&joined, username, summary);
        match self.reusable(username, &joined).await? {
            Some(existing) => self.revise(existing, &content, &message).await,
            None => {
                self.open_new(username, &joined, &file, &content, &message, summary)
                    .await
            }
        }
    }

    /// The contributor's open proposal for this page, if the FORGE still calls it open. A row
    /// whose pull request has since been merged or closed is settled here and not reused.
    async fn reusable(
        &self,
        username: &str,
        lesson_path: &str,
    ) -> Result<Option<EditRequest>, AuthoringError> {
        let Some(existing) = self.repo.open_for(username, lesson_path).await? else {
            return Ok(None);
        };
        // A dry-run row has no pull request to ask about; its branch is the only artifact, so it
        // stays reusable.
        let Some(number) = existing.pull_request().map(|pr| pr.number) else {
            return Ok(Some(existing));
        };
        let state = self
            .forge_at(&existing.location().repo)
            .await?
            .pull_request_state(number)
            .await?;
        if state == ForgePrState::Open {
            return Ok(Some(existing));
        }
        let settled = existing.settled(state.settled(), Utc::now());
        tracing::info!(
            branch = settled.location().branch,
            state = settled.state().as_str(),
            "the open proposal for this page is settled — the next edit starts a new branch"
        );
        self.repo.update(&settled).await?;
        Ok(None)
    }

    /// Another commit on an open proposal's branch — no second pull request.
    async fn revise(
        &self,
        existing: EditRequest,
        content: &str,
        message: &str,
    ) -> Result<Proposal, AuthoringError> {
        let forge = self.forge_at(&existing.location().repo).await?;
        forge
            .commit_file(
                &existing.location().branch,
                &existing.location().file_path,
                content,
                message,
            )
            .await?;
        let revised = existing.revised(Utc::now());
        self.repo.update(&revised).await?;
        tracing::info!(
            branch = revised.location().branch,
            commits = revised.commits(),
            "revised an open change request"
        );
        Ok(Proposal {
            request: revised,
            reused: true,
            mode: forge.mode(),
        })
    }

    /// A fresh branch and a new pull request. The attempt number comes from the store rather than
    /// from scanning the forge's refs, which keeps the common path one query instead of a listing.
    async fn open_new(
        &self,
        username: &str,
        lesson_path: &str,
        file: &LessonFile,
        content: &str,
        message: &str,
        summary: Option<&str>,
    ) -> Result<Proposal, AuthoringError> {
        let file_path = file.file_path.as_str();
        let target = self.target_of(&file.source_id).await?;
        let forge = self.forge_at(&target.repo).await?;
        let attempt = self
            .repo
            .highest_attempt(username, lesson_path)
            .await?
            .saturating_add(1);
        let branch = branch_for(username, lesson_path, attempt);
        forge.commit_file(&branch, file_path, content, message).await?;

        let title = message::pull_request_title(lesson_path);
        let body = message::pull_request_body(&target.site_url, lesson_path, file_path, username, summary);
        let pull_request = forge.open_pull_request(&branch, &title, &body).await?;

        let now = Utc::now();
        let mut request = EditRequest::opened(
            EditRequestId(Uuid::new_v4()),
            username.to_owned(),
            ProposalLocation {
                lesson_path: lesson_path.to_owned(),
                file_path: file_path.to_owned(),
                repo: target.repo.clone(),
                branch,
            },
            attempt,
            now,
        );
        if let Some(pull_request) = pull_request {
            request = request.with_pull_request(pull_request, now);
        }
        self.repo.save(&request).await?;
        tracing::info!(
            repo = %target.repo,
            branch = request.location().branch,
            attempt,
            pr = request.pull_request().map(|pr| pr.number),
            "opened a change request"
        );
        Ok(Proposal {
            request,
            reused: false,
            mode: forge.mode(),
        })
    }
}

#[cfg(test)]
mod tests;
