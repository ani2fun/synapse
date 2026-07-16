//! Oracle: `TutoringServiceSpec` — the prompt folding over a capturing fake client.

#![allow(clippy::unwrap_used)]

use std::sync::Mutex;

use super::*;

/// Records the prompt + history it was handed, replies with a canned line.
#[derive(Default)]
struct CapturingClient {
    seen: Mutex<Option<(String, Vec<ChatMessage>)>>,
}

impl TutorClient for &CapturingClient {
    async fn chat(&self, system_prompt: &str, history: &[ChatMessage]) -> Result<String, TutorError> {
        *self.seen.lock().unwrap() = Some((system_prompt.to_owned(), history.to_vec()));
        Ok("try two pointers".to_owned())
    }
}

fn message(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_owned(),
        content: content.to_owned(),
    }
}

#[tokio::test]
async fn a_bare_context_sends_only_the_base_prompt_history_untouched() {
    let client = CapturingClient::default();
    let service = TutoringService::new(&client);
    let history = vec![message("user", "I'm stuck")];
    service.chat(&ChatContext::default(), &history).await.unwrap();
    let (prompt, seen_history) = client.seen.lock().unwrap().clone().unwrap();
    assert!(!prompt.contains("working on:"));
    assert!(!prompt.contains("```"));
    assert_eq!(seen_history, history, "history passes through untouched");
}

#[tokio::test]
async fn a_problem_path_folds_into_the_prompt() {
    let context = ChatContext {
        problem_path: Some("dsa/arrays/two-sum".to_owned()),
        ..ChatContext::default()
    };
    assert!(system_prompt_for(&context).contains("The learner is working on: dsa/arrays/two-sum"));
}

#[tokio::test]
async fn code_and_language_fold_in_as_a_fenced_block() {
    let context = ChatContext {
        code: Some("def f(): pass".to_owned()),
        language: Some("python".to_owned()),
        ..ChatContext::default()
    };
    let prompt = system_prompt_for(&context);
    assert!(prompt.contains("```python"));
    assert!(prompt.contains("def f(): pass"));
}

#[tokio::test]
async fn blank_code_is_absent_no_empty_fence() {
    let context = ChatContext {
        code: Some(String::new()),
        language: Some("python".to_owned()),
        ..ChatContext::default()
    };
    assert!(!system_prompt_for(&context).contains("```"));
}

#[tokio::test]
async fn a_backend_failure_propagates_untouched() {
    struct DownClient;
    impl TutorClient for DownClient {
        async fn chat(&self, _p: &str, _h: &[ChatMessage]) -> Result<String, TutorError> {
            Err(TutorError::BackendUnavailable("down".to_owned()))
        }
    }
    let service = TutoringService::new(DownClient);
    assert_eq!(
        service.chat(&ChatContext::default(), &[]).await.unwrap_err(),
        TutorError::BackendUnavailable("down".to_owned())
    );
}
