//! The typed API client (oracle: `api/ApiClient.scala`) — same-origin fetches decoding the
//! SHARED wire DTOs; errors surface as the `ApiError` envelope's message when the server sent
//! one, the transport error otherwise.

use serde::Serialize;
use serde::de::DeserializeOwned;
use synapse_shared::api::ApiError;
use synapse_shared::blog::{BlogPostDto, BlogSummaryDto};
use synapse_shared::catalog::{LessonPayloadDto, SynapseIndexDto};
use synapse_shared::execution::{RunRequest, RunResult};
use synapse_shared::identity::{AuthConfigDto, MeDto};
use synapse_shared::submission::{SubmissionAcceptedDto, SubmissionDto, SubmitRequestDto};

thread_local! {
    /// The bearer seam (oracle: `ApiClient.installTokenProvider`): identity installs it, every
    /// request reads it, the default stays anonymous — api remains feature-agnostic.
    static TOKEN_PROVIDER: std::cell::RefCell<fn() -> Option<String>> =
        const { std::cell::RefCell::new(|| None) };
}

pub fn set_token_provider(provider: fn() -> Option<String>) {
    TOKEN_PROVIDER.with_borrow_mut(|p| *p = provider);
}

fn bearer() -> Option<String> {
    TOKEN_PROVIDER.with_borrow(|p| p())
}

/// A fetch's reactive lifecycle (oracle: `AsyncResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncResult<T> {
    Loading,
    Loaded(T),
    Failed(String),
}

pub async fn index() -> Result<SynapseIndexDto, String> {
    fetch_json("/api/synapse/index").await
}

pub async fn lesson(path: &[String]) -> Result<LessonPayloadDto, String> {
    fetch_json(&format!("/api/synapse/{}", path.join("/"))).await
}

/// A LikeC4 component's tutorial doc, co-located next to the lesson (`_c4-docs/`).
pub async fn c4_doc(
    element_id: &str,
    lesson: &[String],
) -> Result<synapse_shared::catalog::ComponentDocDto, String> {
    fetch_json(&format!(
        "/api/synapse/c4-doc/{element_id}?lesson={}",
        lesson.join("/")
    ))
    .await
}

/// Run one snippet in the sandbox — a badly-running program is an `Ok(RunResult)`, exactly as
/// the server promises.
pub async fn run(request: &RunRequest) -> Result<RunResult, String> {
    post_json("/api/run", request).await
}

/// Submit a solution — the 202 hands back the id the poll loop watches.
pub async fn submit(request: &SubmitRequestDto) -> Result<SubmissionAcceptedDto, String> {
    post_json("/api/submissions", request).await
}

/// One poll tick.
/// The caller's OWN submissions for a lesson, newest first (anonymous → []).
pub async fn submissions_for(path: &[String]) -> Result<Vec<SubmissionDto>, String> {
    fetch_json(&format!("/api/submissions?path={}", path.join("/"))).await
}

pub async fn submission(id: &str) -> Result<SubmissionDto, String> {
    fetch_json(&format!("/api/submissions/{id}")).await
}

/// The blog listing, newest first.
pub async fn blog_list() -> Result<Vec<BlogSummaryDto>, String> {
    fetch_json("/api/blog").await
}

/// One post with body + neighbours.
pub async fn blog_post(slug: &str) -> Result<BlogPostDto, String> {
    fetch_json(&format!("/api/blog/{slug}")).await
}

/// The SPA's Keycloak coordinates.
pub async fn auth_config() -> Result<AuthConfigDto, String> {
    fetch_json("/api/auth/config").await
}

/// The verified caller — the bearer seam supplies the token.
pub async fn me() -> Result<MeDto, String> {
    fetch_json("/api/me").await
}

/// Erase every submission of the caller ("reset my data").
pub async fn erase_submissions() -> Result<synapse_shared::submission::DeleteResultDto, String> {
    delete_json("/api/submissions").await
}

/// Remove the caller's sign-in (the Keycloak account). App data is the separate verb above —
/// the account page orchestrates erase → delete.
pub async fn delete_account() -> Result<serde_json::Value, String> {
    delete_json("/api/me").await
}

/// The coach's coordinates — always answers; `enabled: false` is the off state.
pub async fn tutor_config() -> Result<synapse_shared::tutor::TutorConfigDto, String> {
    fetch_json("/api/tutor/config").await
}

/// One coaching turn — the full transcript up, one reply back (a 404 means the coach is off).
pub async fn tutor_chat(
    request: &synapse_shared::tutor::TutorChatRequestDto,
) -> Result<synapse_shared::tutor::TutorChatResponseDto, String> {
    post_json("/api/tutor/chat", request).await
}

/// The admin allowlist (server re-checks admin per call — these 401/403 for everyone else).
pub async fn allowlist() -> Result<Vec<synapse_shared::submission::AllowlistEntryDto>, String> {
    fetch_json("/api/admin/allowlist").await
}

pub async fn allowlist_grant(
    request: &synapse_shared::submission::GrantRequestDto,
) -> Result<synapse_shared::submission::AllowlistEntryDto, String> {
    post_json("/api/admin/allowlist", request).await
}

/// `Ok(())` on 204; a 404 surfaces as the `ApiError` message.
pub async fn allowlist_revoke(username: &str) -> Result<(), String> {
    let mut request = gloo_net::http::Request::delete(&format!("/api/admin/allowlist/{username}"));
    if let Some(token) = bearer() {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    if response.ok() {
        return Ok(());
    }
    match response.json::<ApiError>().await {
        Ok(envelope) => Err(envelope
            .detail
            .map_or(envelope.error.clone(), |d| format!("{}: {d}", envelope.error))),
        Err(_) => Err(format!("HTTP {}", response.status())),
    }
}

// The transport chokepoint carries the per-request DEBUG trace (oracle: ApiClient logs each
// endpoint) — one line per wire call, localhost-only, so a dev session reads as a flow.
async fn delete_json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    crate::log::debug(&format!("DELETE {url}"));
    let mut request = gloo_net::http::Request::delete(url);
    if let Some(token) = bearer() {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    decode(response).await
}

async fn post_json<B: Serialize, T: DeserializeOwned>(url: &str, body: &B) -> Result<T, String> {
    crate::log::debug(&format!("POST {url}"));
    let mut request = gloo_net::http::Request::post(url);
    if let Some(token) = bearer() {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let response = request
        .json(body)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    decode(response).await
}

async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    crate::log::debug(&format!("GET {url}"));
    let mut request = gloo_net::http::Request::get(url);
    if let Some(token) = bearer() {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    decode(response).await
}

/// Non-2xx → the `ApiError` envelope's message when the server sent one, `HTTP n` otherwise.
async fn decode<T: DeserializeOwned>(response: gloo_net::http::Response) -> Result<T, String> {
    if !response.ok() {
        let fallback = format!("HTTP {}", response.status());
        return Err(match response.json::<ApiError>().await {
            Ok(envelope) => envelope
                .detail
                .map_or(envelope.error.clone(), |d| format!("{}: {d}", envelope.error)),
            Err(_) => fallback,
        });
    }
    response.json().await.map_err(|error| error.to_string())
}
