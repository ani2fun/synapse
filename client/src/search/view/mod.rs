//! The ⌘K palette (oracle: `SearchPalette`) — a singleton modal mounted once in the shell:
//! global ⌘K/Ctrl-K toggles it, the query ranks the flattened library live, arrows + Enter
//! drive it from the keyboard. The library loads lazily on FIRST open (the palette costs
//! nothing until used); both fetches ride the shared app-level stores.

use leptos::prelude::*;

use crate::api::AsyncResult;
use crate::search::logic::{self, Kind, SearchEntry};
use crate::search::state::SearchStore;

#[component]
pub fn SearchPalette() -> impl IntoView {
    let store = SearchStore::from_context();
    let catalog = crate::catalog::state::CatalogStore::from_context();
    let blog = crate::blog::state::BlogStore::from_context();
    let navigate = leptos_router::hooks::use_navigate();
    let input_ref: NodeRef<leptos::html::Input> = NodeRef::new();

    // Nothing fetches until the palette first opens; once both stores land, the flatten runs.
    let entries = Memo::new(move |_| {
        if !store.is_open.get() {
            return Vec::new();
        }
        let AsyncResult::Loaded(index) = catalog.index().get() else {
            return Vec::new();
        };
        match blog.list().get() {
            AsyncResult::Loaded(posts) => logic::entries(&index, &posts),
            _ => logic::entries(&index, &[]),
        }
    });
    let results = Memo::new(move |_| logic::search(&store.query.get(), &entries.get()));

    // The global toggle — ⌘K / Ctrl-K anywhere.
    let toggle_handle = window_event_listener(leptos::ev::keydown, move |event| {
        if (event.meta_key() || event.ctrl_key()) && event.key().to_lowercase() == "k" {
            event.prevent_default();
            store.toggle();
        }
    });
    on_cleanup(move || toggle_handle.remove());

    // Focus the input as soon as the panel exists.
    Effect::new(move |_| {
        if store.is_open.get()
            && let Some(input) = input_ref.get()
        {
            let _ = input.focus();
        }
    });

    let handle_key = move |event: leptos::ev::KeyboardEvent| {
        let count = results.read_untracked().len();
        match event.key().as_str() {
            "Escape" => store.close(),
            "ArrowDown" => {
                event.prevent_default();
                store.selected.update(|i| *i = clamp(*i + 1, count));
            }
            "ArrowUp" => {
                event.prevent_default();
                store.selected.update(|i| *i = clamp(i.saturating_sub(1), count));
            }
            "Enter" => {
                event.prevent_default();
                let active = clamp(store.selected.get_untracked(), count);
                if let Some(entry) = results.read_untracked().get(active) {
                    navigate(&entry.page.url(), leptos_router::NavigateOptions::default());
                    store.close();
                }
            }
            _ => {}
        }
    };

    view! {
        {move || {
            // Cloned per render: `navigate` inside makes the handler Clone-not-Copy.
            let handle_key = handle_key.clone();
            store.is_open.get().then(move || {
                view! {
                    <div
                        class="cmdk-scrim"
                        on:click=move |event| {
                            if event.target() == event.current_target() {
                                store.close();
                            }
                        }
                    >
                        <div class="cmdk" on:keydown=handle_key>
                            <input
                                class="cmdk__input"
                                node_ref=input_ref
                                placeholder="Search lessons, books, posts…"
                                prop:value=move || store.query.get()
                                on:input=move |event| {
                                    store.query.set(event_target_value(&event));
                                    store.selected.set(0);
                                }
                            />
                            <ul class="cmdk__results">
                                {move || {
                                    let rs = results.get();
                                    if rs.is_empty() {
                                        return vec![view! { <li class="cmdk__empty">"No matches."</li> }.into_any()];
                                    }
                                    let active = clamp(store.selected.get(), rs.len());
                                    rs.into_iter()
                                        .enumerate()
                                        .map(|(i, entry)| result_row(&entry, i == active, store).into_any())
                                        .collect::<Vec<_>>()
                                }}
                            </ul>
                        </div>
                    </div>
                }
            })
        }}
    }
}

fn result_row(entry: &SearchEntry, active: bool, store: SearchStore) -> impl IntoView + use<> {
    let class = if active {
        "cmdk__result cmdk__result--active"
    } else {
        "cmdk__result"
    };
    let sublabel = (!entry.sublabel.is_empty()).then(|| entry.sublabel.clone());
    view! {
        <li>
            <a class=class href=entry.page.url() on:click=move |_| store.close()>
                <span class="cmdk__result-kind">{kind_label(entry.kind)}</span>
                <span class="cmdk__result-text">
                    <span class="cmdk__result-label">{entry.label.clone()}</span>
                    {sublabel.map(|s| view! { <span class="cmdk__result-sub">{s}</span> })}
                </span>
            </a>
        </li>
    }
}

fn clamp(i: usize, count: usize) -> usize {
    if count == 0 { 0 } else { i.min(count - 1) }
}

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Lesson => "Lesson",
        Kind::Book => "Book",
        Kind::Blog => "Post",
    }
}

/// The header trigger — same singleton, no synthetic events.
#[component]
pub fn SearchButton() -> impl IntoView {
    let store = SearchStore::from_context();
    view! {
        <button
            class="header__search"
            aria-label="Search the library"
            title="Search the library (⌘K)"
            on:click=move |_| store.open()
        >
            // The glyph carries the button on narrow screens, where the label and the ⌘K hint
            // both stand down (there is no ⌘ on a phone to hint at).
            <svg class="header__search-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <circle cx="11" cy="11" r="8"></circle>
                <path d="m21 21-4.3-4.3"></path>
            </svg>
            <span class="header__search-text">"Search the library…"</span>
            <kbd class="header__search-kbd">"⌘K"</kbd>
        </button>
    }
}
