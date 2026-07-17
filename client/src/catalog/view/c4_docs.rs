//! The C4 docs panel (oracle: `C4DocsPanel`, ADR-S032): click a component in an embedded
//! LikeC4 diagram and its tutorial doc — a co-located `_c4-docs/*.md` next to the lesson —
//! slides in from the right. Clicking another component switches context; ✕/Esc close.
//! (RS deviation, on purpose: a fixed right-side panel instead of the oracle's JS grid
//! collapse — the reader column stays put, no inline-!important surgery.)

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::catalog::ComponentDocDto;

use crate::api;
use crate::islands::markdown;

#[derive(Clone, PartialEq)]
enum DocState {
    Loading,
    Ready(Box<ComponentDocDto>),
    Failed(String),
}

#[component]
pub fn C4DocsPanel(selected: RwSignal<Option<String>>, lesson: Vec<String>) -> impl IntoView {
    let state: RwSignal<DocState> = RwSignal::new(DocState::Loading);
    let lesson = StoredValue::new(lesson);

    // Fetch on every selection change; a stale reply for a superseded selection is dropped.
    Effect::new(move |_| {
        let Some(id) = selected.get() else { return };
        state.set(DocState::Loading);
        let path = lesson.read_value().clone();
        crate::log::debug(&format!("c4 docs: open {id} (lesson {})", path.join("/")));
        let current = id.clone();
        spawn_local(async move {
            let result = api::c4_doc(&current, &path).await;
            if selected.get_untracked().as_deref() != Some(current.as_str()) {
                return; // superseded while in flight
            }
            match result {
                Ok(doc) => state.set(DocState::Ready(Box::new(doc))),
                Err(message) => state.set(DocState::Failed(message)),
            }
        });
    });

    let esc = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" && selected.get_untracked().is_some() {
            selected.set(None);
        }
    });
    on_cleanup(move || esc.remove());

    view! {
        {move || selected.get().map(|id| view! {
            <aside class="c4-docs">
                <div class="c4-docs__head">
                    <span class="c4-docs__eyebrow">"COMPONENT GUIDE"</span>
                    <button class="c4-docs__close" aria-label="Close" on:click=move |_| selected.set(None)>
                        "✕"
                    </button>
                </div>
                {move || match state.get() {
                    DocState::Loading => view! {
                        <p class="c4-docs__status">"Loading the guide…"</p>
                    }
                    .into_any(),
                    DocState::Failed(message) => view! {
                        <div class="c4-docs__missing">
                            <p><b>{id.clone()}</b>" has no guide here yet."</p>
                            <p class="c4-docs__status">{message.clone()}</p>
                        </div>
                    }
                    .into_any(),
                    DocState::Ready(doc) => doc_view(&doc).into_any(),
                }}
            </aside>
        })}
    }
}

fn doc_view(doc: &ComponentDocDto) -> impl IntoView + use<> {
    let chips: Vec<_> = [doc.kind.clone(), doc.technology.clone()]
        .into_iter()
        .flatten()
        .map(|c| view! { <span class="c4-docs__chip">{c}</span> })
        .collect();
    let title = doc.title.clone();
    let body = doc.body.clone();
    let node_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    Effect::new(move |_| {
        let Some(node) = node_ref.get() else { return };
        let md = body.clone();
        spawn_local(async move {
            match markdown::render(&md).await {
                Ok(html) => node.set_inner_html(&html),
                Err(_) => node.set_text_content(Some(&md)),
            }
        });
    });
    view! {
        {title.map(|t| view! { <h2 class="c4-docs__title">{t}</h2> })}
        {(!chips.is_empty()).then(|| view! { <div class="c4-docs__chips">{chips}</div> })}
        <div class="c4-docs__body" node_ref=node_ref></div>
    }
}
