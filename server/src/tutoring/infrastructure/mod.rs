//! The OpenAI-compatible chat adapter (oracle: `OllamaTutorClient` + `TutorWire`): one
//! non-streaming `POST {base}/v1/chat/completions` — Ollama, LM Studio, and vLLM all speak
//! it. Wire shaping is adapter-owned (never shared); HTTP/1.1 forced (local model servers
//! are plain HTTP/1.1 — the go-judge h2c lesson); 60 s per request (local CPU inference is
//! slow — generous, not infinite), 10 s connect.

use synapse_shared::tutor::ChatMessage;

use crate::tutoring::application::{TutorClient, TutorError};

pub struct OllamaTutorClient {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaTutorClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        let client = reqwest::Client::builder()
            .http1_only()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_mins(1))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
            model: model.to_owned(),
        }
    }
}

/// `{"model", "messages": [system, ...history]}` — no stream, no options; the system turn
/// always leads.
pub fn build_request_body(model: &str, system_prompt: &str, history: &[ChatMessage]) -> serde_json::Value {
    let mut messages = vec![serde_json::json!({ "role": "system", "content": system_prompt })];
    messages.extend(
        history
            .iter()
            .map(|m| serde_json::json!({ "role": m.role, "content": m.content })),
    );
    serde_json::json!({ "model": model, "messages": messages })
}

/// `choices[0].message.content` of an OpenAI-shaped completion; anything else fails loudly.
pub fn parse_reply(body: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("tutor reply is not JSON: {e}"))?;
    value
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "tutor reply carried no choices[0].message.content".to_owned())
}

impl TutorClient for OllamaTutorClient {
    async fn chat(&self, system_prompt: &str, history: &[ChatMessage]) -> Result<String, TutorError> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = build_request_body(&self.model, system_prompt, history);
        let response = self.client.post(&url).json(&body).send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                TutorError::BackendUnavailable(format!("no model server at {}: {e}", self.base_url))
            } else {
                TutorError::BackendFailed(e.to_string())
            }
        })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| TutorError::BackendFailed(format!("tutor reply unreadable: {e}")))?;
        if !status.is_success() {
            tracing::warn!(%status, "tutor backend answered non-2xx");
            return Err(TutorError::BackendFailed(format!(
                "tutor backend returned {status}: {text}"
            )));
        }
        let reply = parse_reply(&text).map_err(TutorError::BackendFailed)?;
        tracing::debug!(model = self.model, chars = reply.len(), "tutor replied");
        Ok(reply)
    }
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
