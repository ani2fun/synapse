//! The Kokoro adapter: one unit in, one MP3 out, over the engine's OpenAI-compatible
//! `POST /v1/audio/speech`.
//!
//! Non-streaming on purpose. A unit is a few sentences, so its whole MP3 arrives in a second or
//! two, and having it whole is what lets the narrator cache it before handing it on.

use std::time::Duration;

use axum::body::Bytes;
use serde::Serialize;

use crate::listen::{ListenError, SpeechEngine, Voice};

/// A 400-character unit took up to ~20 s on two cores when measured; the margin is for an engine
/// that is cold or briefly starved, not for a unit that is longer.
const SYNTH_TIMEOUT: Duration = Duration::from_mins(1);

pub struct KokoroEngine {
    client: reqwest::Client,
    url: String,
}

#[derive(Serialize)]
struct SpeechRequest<'a> {
    model: &'static str,
    input: &'a str,
    voice: &'static str,
    response_format: &'static str,
    stream: bool,
}

impl KokoroEngine {
    pub fn new(base: &str) -> Self {
        let client = reqwest::Client::builder()
            .http1_only()
            .connect_timeout(Duration::from_secs(5))
            .timeout(SYNTH_TIMEOUT)
            .build()
            // Builder failure = TLS backend missing at boot — a config bug, not a request error.
            .unwrap_or_default();
        Self {
            client,
            url: format!("{}/v1/audio/speech", base.trim_end_matches('/')),
        }
    }
}

impl SpeechEngine for KokoroEngine {
    async fn synthesize(&self, voice: Voice, text: &str) -> Result<Bytes, ListenError> {
        let request = SpeechRequest {
            model: "kokoro",
            input: text,
            voice: voice.as_str(),
            response_format: "mp3",
            stream: false,
        };
        let started = std::time::Instant::now();
        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| ListenError::EngineFailed(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            // The engine's own message, trimmed: enough to diagnose, never a page of HTML.
            let detail: String = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            return Err(ListenError::EngineFailed(format!("{status}: {detail}")));
        }
        let audio = response
            .bytes()
            .await
            .map_err(|e| ListenError::EngineFailed(e.to_string()))?;
        tracing::debug!(
            voice = voice.as_str(),
            chars = text.chars().count(),
            bytes = audio.len(),
            ms = started.elapsed().as_millis(),
            "listen: unit synthesised"
        );
        Ok(audio)
    }
}
