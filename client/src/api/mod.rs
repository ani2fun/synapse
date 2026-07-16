//! The typed API client (oracle: `api/ApiClient.scala`) — same-origin fetches decoding the
//! SHARED wire DTOs; errors surface as the `ApiError` envelope's message when the server sent
//! one, the transport error otherwise.

use serde::Serialize;
use serde::de::DeserializeOwned;
use synapse_shared::api::ApiError;
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
pub async fn submission(id: &str) -> Result<SubmissionDto, String> {
    fetch_json(&format!("/api/submissions/{id}")).await
}

/// The SPA's Keycloak coordinates.
pub async fn auth_config() -> Result<AuthConfigDto, String> {
    fetch_json("/api/auth/config").await
}

/// The verified caller — the bearer seam supplies the token.
pub async fn me() -> Result<MeDto, String> {
    fetch_json("/api/me").await
}

async fn post_json<B: Serialize, T: DeserializeOwned>(url: &str, body: &B) -> Result<T, String> {
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
