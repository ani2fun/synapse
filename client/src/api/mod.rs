//! The typed API client (oracle: `api/ApiClient.scala`) — same-origin fetches decoding the
//! SHARED wire DTOs; errors surface as the `ApiError` envelope's message when the server sent
//! one, the transport error otherwise.

use serde::de::DeserializeOwned;
use synapse_shared::api::ApiError;
use synapse_shared::catalog::{LessonPayloadDto, SynapseIndexDto};

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

async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    let response = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
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
