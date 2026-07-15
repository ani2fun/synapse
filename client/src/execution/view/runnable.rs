//! One runnable code block (oracle: `Workbench` reduced to step 11): toolbar (eyebrow · lang
//! pill · Edit · Run), a Monaco editor across the `@editor` island, and the output panel.
//! The ⌘⏎/⌘E keymap arrives through monaco actions wired at mount; ⇧⌘⏎ Submit joins with the
//! submissions step, and the identity step adds the auth gate on Edit.

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::execution::RunResult;

use crate::execution::logic::{self, RunState, Variant};
use crate::execution::state::BlockStore;
use crate::islands::editor::{self, MountedEditor};

// Component props are moved by design (leptos owns them for the view's lifetime).
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn RunnableBlock(variant: Variant) -> impl IntoView {
    let store = BlockStore::new(&variant.source);
    let language = variant.language.clone();
    let authored = StoredValue::new(variant.source.clone());
    let mounted: StoredValue<Option<MountedEditor>, LocalStorage> = StoredValue::new_local(None);
    let editor_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    // Mount monaco once the container exists; the handle + closures live in `mounted` and are
    // dropped (→ disposed) when the block unmounts.
    Effect::new(move |_| {
        let Some(node) = editor_ref.get() else { return };
        if mounted.read_value().is_some() {
            return;
        }
        let value = store.state.get_untracked().code;
        let lang = language.clone();
        spawn_local(async move {
            let on_change = move |code: String| store.state.update(|s| *s = s.set_code(&code));
            let run_lang = lang.clone();
            let on_run = move || store.launch(run_lang.clone());
            let on_toggle = move || {
                store.toggle_edit(&authored.read_value());
                sync_editor(mounted, store);
            };
            match editor::mount(&node, &value, &lang, true, on_change, on_run, on_toggle).await {
                Ok(handle) => mounted.set_value(Some(handle)),
                Err(error) => leptos::logging::error!("monaco island failed: {error:?}"),
            }
        });
    });
    on_cleanup(move || mounted.set_value(None));

    let running = Memo::new(move |_| store.state.read().run_state == RunState::Running);
    let unlocked = store.unlocked;
    let pill = logic::display_lang(&variant.language);
    let run_lang = variant.language.clone();
    let height = format!("height: {}px;", editor::default_height_px(&variant.source));

    view! {
        <div class="runnable not-prose">
            <div class="runnable__bar">
                <span class="wb__eyebrow"><span class="wb__prompt">">_"</span>" CODE"</span>
                <span class="wb__actions">
                    <span class="wb__lang-pill">{pill}</span>
                    <span
                        class="wb__tip"
                        data-tip=move || {
                            if unlocked.get() {
                                "Editing — your changes stay on this page (⌘E toggles)"
                            } else {
                                "Edit this code — changes stay on this page (⌘E)"
                            }
                        }
                    >
                        <button
                            class="wb__ghost"
                            class:wb__ghost--live=move || unlocked.get()
                            on:click=move |_| {
                                store.toggle_edit(&authored.read_value());
                                sync_editor(mounted, store);
                            }
                        >
                            {move || if unlocked.get() { "Editing" } else { "Edit" }}
                        </button>
                    </span>
                    <button
                        class="runnable__run"
                        title="Run (⌘⏎)"
                        prop:disabled=move || running.get()
                        on:click=move |_| store.launch(run_lang.clone())
                    >
                        {move || if running.get() { "Running…" } else { "▶ Run" }}
                    </button>
                </span>
            </div>
            <div class="runnable__editor" node_ref=editor_ref style=height></div>
            <Output store=store />
        </div>
    }
}

/// Locking/unlocking must reach monaco too: read-only flips in place, and a revert rewrites
/// the buffer.
fn sync_editor(mounted: StoredValue<Option<MountedEditor>, LocalStorage>, store: BlockStore) {
    mounted.with_value(|editor| {
        if let Some(editor) = editor {
            let state = store.state.get_untracked();
            editor.set_read_only(!store.unlocked.get_untracked());
            if editor.get_value() != state.code {
                editor.set_value(&state.code);
            }
        }
    });
}

#[component]
fn Output(store: BlockStore) -> impl IntoView {
    view! {
        {move || {
            let state = store.state.get();
            if let Some(error) = &state.error {
                return error_panel(error).into_any();
            }
            if let Some(result) = &state.result {
                return result_panel(result).into_any();
            }
            if state.run_state == RunState::Running {
                return view! { <div class="runnable__out runnable__out--running">"Running…"</div> }
                    .into_any();
            }
            ().into_any()
        }}
    }
}

fn error_panel(error: &str) -> impl IntoView + use<> {
    view! {
        <div class="runnable__out runnable__out--error">
            <div class="runnable__status"><span class="runnable__badge runnable__badge--fail">"Error"</span></div>
            <pre class="runnable__stream">{error.to_owned()}</pre>
        </div>
    }
}

fn result_panel(result: &RunResult) -> impl IntoView + use<> {
    let badge_class = if result.status.is_success() {
        "runnable__badge runnable__badge--ok"
    } else {
        "runnable__badge runnable__badge--fail"
    };
    let time = result.time_seconds.map(|s| format!("{s:.3} s"));
    let memory = result.memory_kb.map(|kb| format!("{} MB", kb / 1024));
    let stdout = result.stdout.clone();
    view! {
        <div class="runnable__out">
            <div class="runnable__status">
                <span class=badge_class>{result.status.label()}</span>
                {time.map(|t| view! { <span class="runnable__meta">{t}</span> })}
                {memory.map(|m| view! { <span class="runnable__meta">{m}</span> })}
            </div>
            {stream_block("compile output", &result.compile_output)}
            {stream_block("stderr", &result.stderr)}
            {if stdout.is_empty() {
                view! { <p class="runnable__empty">"(no output)"</p> }.into_any()
            } else {
                view! { <pre class="runnable__stdout">{stdout}</pre> }.into_any()
            }}
        </div>
    }
}

fn stream_block(label: &'static str, content: &str) -> Option<impl IntoView + use<>> {
    if content.is_empty() {
        return None;
    }
    let content = content.to_owned();
    Some(view! {
        <details class="runnable__details" open>
            <summary class="runnable__details-label">{label}</summary>
            <pre class="runnable__stream">{content}</pre>
        </details>
    })
}
