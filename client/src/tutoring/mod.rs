//! The Coach pane (oracle: `CoachPane`, step 20 / ADR-S025) — a flat thin feature (state +
//! view in one module; the server does the LLM work). The transcript is EPHEMERAL (a signal,
//! gone on navigation); config is fetched on mount and ANY failure falls to Off — never a
//! chat box that 404s. Off copy: "The coach is off / This feature is coming soon."

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::tutor::{ChatMessage, TutorChatRequestDto};

use crate::api;

#[derive(Clone, PartialEq, Eq)]
enum ConfigState {
    Loading,
    Off,
    On(String),
}

#[derive(Clone, PartialEq, Eq)]
enum SendState {
    Idle,
    Sending,
    Failed(String),
}

/// `code_ctx` mirrors the workbench editor `(source, language)` — read at SEND time only
/// (a snapshot, not a subscription).
#[component]
pub fn CoachPane(problem: Option<String>, code_ctx: RwSignal<(String, String)>) -> impl IntoView {
    let cfg = RwSignal::new(ConfigState::Loading);
    let messages: RwSignal<Vec<ChatMessage>> = RwSignal::new(Vec::new());
    let draft = RwSignal::new(String::new());
    let send_state = RwSignal::new(SendState::Idle);
    let problem = StoredValue::new(problem);

    spawn_local(async move {
        match api::tutor_config().await {
            Ok(config) if config.enabled => cfg.set(ConfigState::On(config.model)),
            _ => cfg.set(ConfigState::Off),
        }
    });

    let send = move || {
        let text = draft.get_untracked().trim().to_owned();
        if text.is_empty() || send_state.get_untracked() == SendState::Sending {
            return;
        }
        messages.update(|m| {
            m.push(ChatMessage {
                role: "user".to_owned(),
                content: text,
            });
        });
        draft.set(String::new());
        send_state.set(SendState::Sending);
        let (code, language) = code_ctx.get_untracked();
        let request = TutorChatRequestDto {
            problem_path: problem.get_value(),
            code: Some(code).filter(|c| !c.is_empty()),
            language: Some(language).filter(|l| !l.is_empty()),
            messages: messages.get_untracked(),
        };
        crate::log::info(&format!(
            "tutor: sending turn ({} message(s))",
            request.messages.len()
        ));
        spawn_local(async move {
            match api::tutor_chat(&request).await {
                Ok(reply) => {
                    crate::log::debug("tutor: reply received");
                    messages.update(|m| {
                        m.push(ChatMessage {
                            role: "assistant".to_owned(),
                            content: reply.content,
                        });
                    });
                    send_state.set(SendState::Idle);
                }
                Err(message) => {
                    crate::log::error(&format!("tutor: chat failed — {message}"));
                    send_state.set(SendState::Failed(message));
                }
            }
        });
    };

    view! {
        <div class="coach not-prose">
            {move || match cfg.get() {
                ConfigState::Loading => view! { <p class="coach__checking">"Checking the coach…"</p> }.into_any(),
                ConfigState::Off => view! {
                    <div class="coach__off">
                        <span class="coach__off-title">"The coach is off"</span>
                        <p class="coach__off-note">"This feature is coming soon."</p>
                    </div>
                }
                .into_any(),
                ConfigState::On(model) => chat_ui(model, messages, draft, send_state, send).into_any(),
            }}
        </div>
    }
}

fn chat_ui<S: Fn() + Copy + 'static>(
    model: String,
    messages: RwSignal<Vec<ChatMessage>>,
    draft: RwSignal<String>,
    send_state: RwSignal<SendState>,
    send: S,
) -> impl IntoView + use<S> {
    view! {
        <div class="coach__chat">
            <div class="coach__head">
                <span class="wb__eyebrow"><span class="wb__prompt">">_"</span>" COACH"</span>
                <span class="coach__model">{model}</span>
            </div>
            <div class="coach__log">
                {move || messages.get().into_iter().map(bubble).collect::<Vec<_>>()}
                {move || match send_state.get() {
                    SendState::Sending => view! {
                        <div class="coach__bubble coach__bubble--assistant coach__typing">"…"</div>
                    }
                    .into_any(),
                    SendState::Failed(message) => view! {
                        <div class="coach__error">"Couldn't reach the coach — " {message}</div>
                    }
                    .into_any(),
                    SendState::Idle => ().into_any(),
                }}
            </div>
            <div class="coach__composer">
                <textarea
                    class="coach__input"
                    placeholder="Ask for a hint…"
                    rows="2"
                    prop:value=move || draft.get()
                    on:input=move |event| draft.set(event_target_value(&event))
                    on:keydown=move |event| {
                        // Enter sends; Shift+Enter stays a newline.
                        if event.key() == "Enter" && !event.shift_key() {
                            event.prevent_default();
                            send();
                        }
                    }
                ></textarea>
                <button
                    class="wb__submit"
                    prop:disabled=move || send_state.get() == SendState::Sending
                    on:click=move |_| send()
                >
                    "Send"
                </button>
            </div>
        </div>
    }
}

fn bubble(message: ChatMessage) -> impl IntoView {
    let class = format!("coach__bubble coach__bubble--{}", message.role);
    view! { <div class=class><p>{message.content}</p></div> }
}
