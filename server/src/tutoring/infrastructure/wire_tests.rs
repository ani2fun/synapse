//! Tests for the `TutorWire` shape — the request shape and the loud reply parse.

#![allow(clippy::unwrap_used)]

use synapse_shared::tutor::ChatMessage;

use super::*;

fn message(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_owned(),
        content: content.to_owned(),
    }
}

#[test]
fn the_system_prompt_leads_and_history_follows_in_order() {
    let body = build_request_body(
        "llama3.1",
        "be a coach",
        &[
            message("user", "help"),
            message("assistant", "what have you tried?"),
        ],
    )
    .to_string();
    assert!(body.contains(r#""model":"llama3.1""#));
    let system = body.find(r#""role":"system""#).unwrap();
    let user = body.find(r#""role":"user""#).unwrap();
    let assistant = body.find(r#""role":"assistant""#).unwrap();
    assert!(system < user && user < assistant, "order is load-bearing");
    assert!(body.contains("be a coach"));
    assert!(body.contains("what have you tried?"));
}

#[test]
fn an_empty_history_still_carries_the_system_prompt_alone() {
    let body = build_request_body("llama3.1", "be a coach", &[]).to_string();
    assert!(body.contains(r#""role":"system""#));
    assert!(!body.contains(r#""role":"user""#));
}

#[test]
fn parse_reply_pulls_the_assistant_content() {
    let body = r#"{"choices":[{"message":{"role":"assistant","content":"try two pointers"}}]}"#;
    assert_eq!(parse_reply(body).unwrap(), "try two pointers");
}

#[test]
fn malformed_json_fails_loudly() {
    assert!(parse_reply("not json").is_err());
}

#[test]
fn valid_json_missing_the_shape_fails_loudly() {
    assert!(parse_reply(r#"{"choices":[]}"#).is_err());
}
