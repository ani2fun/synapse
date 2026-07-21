//! The tutoring use case: `TutoringService` + `TutorError` + the `TutorClient` port. The
//! service's whole job is the SYSTEM PROMPT: steering (hints over solutions), never
//! scoring — a learner can never be blocked by it. History passes through untouched.

use synapse_shared::tutor::ChatMessage;

/// Machinery-only (mirrors `ExecutionError`); deliberately NO `Disabled` case — disabled is
/// a structural 404 at the http layer, not an error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TutorError {
    #[error("tutor backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("tutor backend failed: {0}")]
    BackendFailed(String),
}

/// The driven port: one completed reply per call (non-streaming by design).
pub trait TutorClient: Send + Sync {
    fn chat(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
    ) -> impl Future<Output = Result<String, TutorError>> + Send;
}

/// What the learner is looking at, folded into the system prompt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatContext {
    pub problem_path: Option<String>,
    pub code: Option<String>,
    pub language: Option<String>,
}

/// The base steering prompt — its EXACT wording is load-bearing; do not paraphrase it.
const BASE_PROMPT: &str = "You are a patient coding coach guiding a learner through a \
data-structures-and-algorithms problem. Ask questions and give hints that nudge them toward \
their OWN solution — never hand over a complete, working answer outright. Point at the \
specific idea or line that's off, suggest what to try next, and check their understanding \
before moving on. Keep replies short — a few sentences.";

pub struct TutoringService<C> {
    client: C,
}

impl<C: TutorClient> TutoringService<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub async fn chat(&self, context: &ChatContext, history: &[ChatMessage]) -> Result<String, TutorError> {
        self.client.chat(&system_prompt_for(context), history).await
    }
}

/// Base prompt + the problem line + the fenced current code (blank code = absent — no empty
/// fence; code without a language is dropped too).
pub fn system_prompt_for(context: &ChatContext) -> String {
    use std::fmt::Write;
    let mut prompt = BASE_PROMPT.to_owned();
    if let Some(path) = &context.problem_path {
        let _ = write!(prompt, "\n\nThe learner is working on: {path}");
    }
    if let (Some(language), Some(code)) = (&context.language, &context.code)
        && !code.is_empty()
    {
        let _ = write!(
            prompt,
            "\n\nTheir current {language} code:\n```{language}\n{code}\n```"
        );
    }
    prompt
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
