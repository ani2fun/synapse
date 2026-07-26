//! The in-memory doubles `service_tests` drives `ProposeEdit` through — a lesson source whose
//! file can move mid-test, an allowlist, a store, and two forges: one that records what it was
//! asked to do, and one that panics if it is reached at all.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// The fakes mirror the ports' Result shapes on purpose, even where they cannot fail.
#![allow(clippy::unnecessary_wraps)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::authoring::application::{
    AuthoringError, ContentEditorEntry, ContentEditors, ContentForge, EditRequestRepository, Editor,
    ForgePrState, ForgeTarget, Forges, LessonFile, LessonSource, ProposeEdit,
};
use crate::authoring::domain::validation::fingerprint;
use crate::authoring::domain::{EditRequest, PullRequestRef};
use crate::catalog::domain::content_tree::PRIMARY_SOURCE_ID;

pub const PAGE: &str = "system-design-from-first-principles/foundations/thinking-in-tradeoffs";
pub const FILE: &str = "system-design-from-first-principles/01-foundations/01-thinking-in-tradeoffs.md";
pub const ORIGINAL: &str = "---\ntitle: Thinking in Tradeoffs\n---\n\nThe original prose.\n";
pub const EDITED: &str = "---\ntitle: Thinking in Tradeoffs\n---\n\nThe sharpened prose.\n";

pub fn page() -> Vec<String> {
    PAGE.split('/').map(str::to_owned).collect()
}

pub fn ani2fun() -> Editor {
    Editor {
        username: "ani2fun".to_owned(),
    }
}

pub fn editor(username: &str) -> Editor {
    Editor {
        username: username.to_owned(),
    }
}

/// The fingerprint the editor would have been handed for `ORIGINAL`.
pub fn base() -> String {
    fingerprint(ORIGINAL)
}

/// A proposal that differs from both `ORIGINAL` and `EDITED`.
pub fn revision(prose: &str) -> String {
    format!("---\ntitle: Thinking in Tradeoffs\n---\n\n{prose}\n")
}

// ── the lesson source ─────────────────────────────────────────────────────────

pub struct FakeSource(Mutex<Option<String>>);

impl FakeSource {
    pub fn holding(source: &str) -> Self {
        Self(Mutex::new(Some(source.to_owned())))
    }
    pub fn missing() -> Self {
        Self(Mutex::new(None))
    }
    /// Simulate the file moving under an open editor (a merge landing, git-sync advancing).
    pub fn moves_to(&self, source: &str) {
        *self.0.lock().unwrap() = Some(source.to_owned());
    }
}

impl LessonSource for FakeSource {
    async fn file_for(&self, _path: &[String]) -> Result<Option<LessonFile>, AuthoringError> {
        Ok(self.0.lock().unwrap().clone().map(|source| LessonFile {
            source_id: PRIMARY_SOURCE_ID.to_owned(),
            file_path: FILE.to_owned(),
            source,
        }))
    }
    async fn content_version(&self) -> String {
        "sha-abc".to_owned()
    }
}

// ── the allowlist ─────────────────────────────────────────────────────────────

pub struct FakeEditors(pub Vec<String>);

impl ContentEditors for FakeEditors {
    async fn is_allowed(&self, username: &str) -> Result<bool, AuthoringError> {
        Ok(self.0.iter().any(|u| u == username))
    }
    async fn list(&self) -> Result<Vec<ContentEditorEntry>, AuthoringError> {
        Ok(Vec::new())
    }
    async fn grant(&self, _u: &str, _n: Option<&str>) -> Result<ContentEditorEntry, AuthoringError> {
        unreachable!("the service never grants")
    }
    async fn revoke(&self, _u: &str) -> Result<bool, AuthoringError> {
        unreachable!("the service never revokes")
    }
}

// ── the store ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeRepo {
    rows: Mutex<HashMap<String, EditRequest>>,
}

impl FakeRepo {
    /// Every row, oldest attempt first.
    pub fn all(&self) -> Vec<EditRequest> {
        let mut rows: Vec<EditRequest> = self.rows.lock().unwrap().values().cloned().collect();
        rows.sort_by_key(|r| r.attempt);
        rows
    }
}

impl EditRequestRepository for FakeRepo {
    async fn open_for(
        &self,
        username: &str,
        lesson_path: &str,
    ) -> Result<Option<EditRequest>, AuthoringError> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .find(|r| r.username == username && r.lesson_path == lesson_path && r.state.is_open())
            .cloned())
    }
    async fn highest_attempt(&self, username: &str, lesson_path: &str) -> Result<u32, AuthoringError> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .filter(|r| r.username == username && r.lesson_path == lesson_path)
            .map(|r| r.attempt)
            .max()
            .unwrap_or(0))
    }
    async fn save(&self, request: &EditRequest) -> Result<(), AuthoringError> {
        self.rows
            .lock()
            .unwrap()
            .insert(request.branch.clone(), request.clone());
        Ok(())
    }
    async fn update(&self, request: &EditRequest) -> Result<(), AuthoringError> {
        self.rows
            .lock()
            .unwrap()
            .insert(request.branch.clone(), request.clone());
        Ok(())
    }
    async fn list_for(&self, username: &str) -> Result<Vec<EditRequest>, AuthoringError> {
        let mut rows: Vec<EditRequest> = self
            .rows
            .lock()
            .unwrap()
            .values()
            .filter(|r| r.username == username)
            .cloned()
            .collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        Ok(rows)
    }
}

// ── the forges ────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeForge {
    pub commits: Mutex<Vec<(String, String)>>,
    pub opened: Mutex<Vec<String>>,
    /// pull-request number → what the forge says about it now.
    pub states: Mutex<HashMap<u64, ForgePrState>>,
    next_number: Mutex<u64>,
}

impl FakeForge {
    pub fn commits_on(&self, branch: &str) -> usize {
        self.commits
            .lock()
            .unwrap()
            .iter()
            .filter(|(b, _)| b == branch)
            .count()
    }
    pub fn commit_count(&self) -> usize {
        self.commits.lock().unwrap().len()
    }
    pub fn opened_count(&self) -> usize {
        self.opened.lock().unwrap().len()
    }
    pub fn merge(&self, number: u64) {
        self.states.lock().unwrap().insert(number, ForgePrState::Merged);
    }
    /// Delete a pull request outright — the fake then reports `Missing` for that number.
    pub fn forget(&self, number: u64) {
        self.states.lock().unwrap().remove(&number);
    }
}

impl ContentForge for FakeForge {
    fn mode(&self) -> &'static str {
        "fake"
    }
    async fn commit_file(
        &self,
        branch: &str,
        _file_path: &str,
        content: &str,
        _message: &str,
    ) -> Result<String, AuthoringError> {
        let mut commits = self.commits.lock().unwrap();
        commits.push((branch.to_owned(), content.to_owned()));
        Ok(format!("commit-{}", commits.len()))
    }
    async fn open_pull_request(
        &self,
        branch: &str,
        _title: &str,
        _body: &str,
    ) -> Result<Option<PullRequestRef>, AuthoringError> {
        self.opened.lock().unwrap().push(branch.to_owned());
        let mut next = self.next_number.lock().unwrap();
        *next += 1;
        self.states.lock().unwrap().insert(*next, ForgePrState::Open);
        Ok(Some(PullRequestRef {
            number: *next,
            url: format!("https://forge.test/pull/{next}"),
        }))
    }
    async fn pull_request_state(&self, number: u64) -> Result<ForgePrState, AuthoringError> {
        Ok(self
            .states
            .lock()
            .unwrap()
            .get(&number)
            .copied()
            .unwrap_or(ForgePrState::Missing))
    }
}

/// A forge that refuses everything — proves a rejected call never reaches it.
pub struct DeadForge;

impl ContentForge for DeadForge {
    fn mode(&self) -> &'static str {
        "dead"
    }
    async fn commit_file(&self, _b: &str, _f: &str, _c: &str, _m: &str) -> Result<String, AuthoringError> {
        panic!("the forge must not be reached")
    }
    async fn open_pull_request(
        &self,
        _b: &str,
        _t: &str,
        _y: &str,
    ) -> Result<Option<PullRequestRef>, AuthoringError> {
        panic!("the forge must not be reached")
    }
    async fn pull_request_state(&self, _n: u64) -> Result<ForgePrState, AuthoringError> {
        panic!("the forge must not be reached")
    }
}

// ── assembly ──────────────────────────────────────────────────────────────────

pub struct Harness<F: ContentForge> {
    pub service: ProposeEdit<FakeSource, FakeEditors, FakeRepo, OneForge<F>>,
    pub source: Arc<FakeSource>,
    pub repo: Arc<FakeRepo>,
    pub forge: Arc<F>,
}

/// One forge for every source — the single-repository shape the pre-satellite tests describe.
/// `Arc` rather than a clone so the double keeps recording through whichever handle is used.
pub struct OneForge<F>(Arc<F>, ForgeTarget);

impl<F: ContentForge> Forges for OneForge<F> {
    type Forge = Arc<F>;

    async fn target_for(&self, _source_id: &str) -> Option<ForgeTarget> {
        Some(self.1.clone())
    }

    async fn forge_for(&self, _repo: &str) -> Option<Self::Forge> {
        Some(Arc::clone(&self.0))
    }
}

pub fn harness_with<F: ContentForge>(source: FakeSource, forge: F, granted: &[&str]) -> Harness<F> {
    let source = Arc::new(source);
    let repo = Arc::new(FakeRepo::default());
    let forge = Arc::new(forge);
    let editors = Arc::new(FakeEditors(granted.iter().map(|u| (*u).to_owned()).collect()));
    let service = ProposeEdit::new(
        Arc::clone(&source),
        editors,
        Arc::clone(&repo),
        Arc::new(OneForge(
            Arc::clone(&forge),
            ForgeTarget {
                repo: "ani2fun/synapse-content".to_owned(),
                base_branch: "main".to_owned(),
                site_url: "https://synapse.kakde.eu".to_owned(),
            },
        )),
    );
    Harness {
        service,
        source,
        repo,
        forge,
    }
}

/// The common case: the file present, a recording forge, one granted contributor.
pub fn harness() -> Harness<FakeForge> {
    harness_with(FakeSource::holding(ORIGINAL), FakeForge::default(), &["ani2fun"])
}

/// A harness whose forge panics on contact — for the paths that must be refused first.
pub fn harness_without_forge(source: FakeSource) -> Harness<DeadForge> {
    harness_with(source, DeadForge, &["ani2fun"])
}
