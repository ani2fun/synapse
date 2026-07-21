//! The tutor wire contract. `ChatMessage` is a pure shared model — it flows client ↔ server
//! ↔ LLM untouched.

use serde::{Deserialize, Serialize};

/// One chat turn. `role` is `"user" | "assistant"` (the server prepends its own system turn).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TutorConfigDto {
    pub enabled: bool,
    pub model: String,
}

/// The whole conversation each turn — the server is stateless; the transcript lives in the
/// client and dies with the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TutorChatRequestDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TutorChatResponseDto {
    pub content: String,
}
