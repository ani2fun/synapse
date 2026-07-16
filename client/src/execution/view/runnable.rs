//! One runnable code block (oracle: `Workbench`, now at step-15 scope): toolbar (eyebrow · lang
//! pill · Edit · Run · Submit), the Monaco island, the tests panel when the block carries an
//! authored suite, the output panel (judged against the active case), and the verdict panel.
//! Identity later gates Edit/Submit on sign-in, exactly as the oracle staged it.

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::execution::{RunResult, TestSpec, Verdict, judge, stdin_for};

use crate::execution::logic::{self, RunState, Variant};
use crate::execution::state::{BlockStore, SubmitState, SubmitStore};
use crate::execution::view::workbench::{TestsPanel, TestsState, VerdictPanel};
use crate::identity::state::AuthStore;
use crate::islands::editor::{self, MountedEditor};

// Component props are moved by design (leptos owns them for the view's lifetime); the length
// is the component's cohesive wiring — props → stores → callbacks → view — and splitting it
// would hide the flow (the oracle keeps `Workbench.apply` as one unit too).
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
#[component]
pub fn RunnableBlock(
    variant: Variant,
    spec: Option<TestSpec>,
    lesson_path: Vec<String>,
    // Passed through hydration: blocks mount OUT-OF-TREE (`mount_to` starts a fresh root
    // owner), so App's context is out of reach — the reader captures the store in-tree.
    auth: AuthStore,
) -> impl IntoView {
    let store = BlockStore::new(&variant.source);
    let submit = SubmitStore::new();
    let language = variant.language.clone();
    let authored = StoredValue::new(variant.source.clone());
    let spec = spec.map(StoredValue::new);
    let tests = spec.map(|s| TestsState::new(&s.read_value()));
    let lesson_path = StoredValue::new(lesson_path);
    let mounted: StoredValue<Option<MountedEditor>, LocalStorage> = StoredValue::new_local(None);
    let editor_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    // The Run seam: with a suite, stdin is the active case's values through the SHARED shape.
    let stdin = move || match (spec, tests) {
        (Some(spec), Some(tests)) => Some(stdin_for(&spec.read_value().args, &tests.values.get_untracked())),
        _ => None,
    };
    let run_lang = language.clone();
    let run = move || store.launch(run_lang.clone(), stdin());
    let submit_lang = language.clone();
    let do_submit = move || {
        submit.submit(
            lesson_path.read_value().clone(),
            submit_lang.clone(),
            store.state.get_untracked().code,
        );
    };

    let run_click = run.clone();
    let submit_click = do_submit.clone();

    // Mount monaco once the container exists; the handle + closures live in `mounted` and are
    // dropped (→ disposed) when the block unmounts.
    Effect::new(move |_| {
        let Some(node) = editor_ref.get() else { return };
        if mounted.read_value().is_some() {
            return;
        }
        let value = store.state.get_untracked().code;
        let lang = language.clone();
        let run = run.clone();
        let do_submit = do_submit.clone();
        spawn_local(async move {
            let callbacks = editor::EditorCallbacks {
                on_change: Box::new(move |code: String| {
                    store.state.update(|s| *s = s.set_code(&code));
                }),
                on_run: Box::new(run),
                on_toggle_edit: Box::new(move || {
                    // The auth gate (oracle: canEditSignal = authed && unlocked).
                    if !auth.authed() {
                        return;
                    }
                    store.toggle_edit(&authored.read_value());
                    sync_editor(mounted, store);
                }),
                on_submit: spec
                    .is_some()
                    .then(move || Box::new(do_submit) as Box<dyn FnMut()>),
            };
            match editor::mount(&node, &value, &lang, true, callbacks).await {
                Ok(handle) => mounted.set_value(Some(handle)),
                Err(error) => leptos::logging::error!("monaco island failed: {error:?}"),
            }
        });
    });
    on_cleanup(move || mounted.set_value(None));

    let running = Memo::new(move |_| store.state.read().run_state == RunState::Running);
    let judging = Memo::new(move |_| matches!(submit.state.get(), SubmitState::Judging(_)));
    let unlocked = store.unlocked;
    let pill = logic::display_lang(&variant.language);
    let height = format!("height: {}px;", editor::default_height_px(&variant.source));

    let toolbar = view! {
        <div class="runnable__bar">
            <span class="wb__eyebrow"><span class="wb__prompt">">_"</span>" CODE"</span>
            <span class="wb__actions">
                <span class="wb__lang-pill">{pill}</span>
                <span
                    class="wb__tip"
                    data-tip=move || {
                        if !auth.authed() {
                            "Sign in to edit this code"
                        } else if unlocked.get() {
                            "Editing — your changes stay on this page (⌘E toggles)"
                        } else {
                            "Edit this code — changes stay on this page (⌘E)"
                        }
                    }
                >
                    <button
                        class="wb__ghost"
                        class:wb__ghost--live=move || unlocked.get()
                        prop:disabled=move || !auth.authed()
                        on:click=move |_| {
                            store.toggle_edit(&authored.read_value());
                            sync_editor(mounted, store);
                        }
                    >
                        {move || if unlocked.get() { "Editing" } else { "Edit" }}
                    </button>
                </span>
                {spec.is_some().then(|| view! {
                    <button
                        class="wb__submit"
                        title="Submit against the hidden suite (⇧⌘⏎)"
                        prop:disabled=move || judging.get()
                        on:click=move |_| submit_click()
                    >
                        {move || if judging.get() { "Judging…" } else { "Submit" }}
                    </button>
                })}
                <button
                    class="runnable__run"
                    title="Run (⌘⏎)"
                    prop:disabled=move || running.get()
                    on:click=move |_| run_click()
                >
                    {move || if running.get() { "Running…" } else { "▶ Run" }}
                </button>
            </span>
        </div>
    };

    view! {
        <div class="runnable not-prose">
            {toolbar}
            <div class="runnable__editor" node_ref=editor_ref style=height></div>
            {match (spec, tests) {
                (Some(spec), Some(tests)) => Some(view! { <TestsPanel spec=spec tests=tests /> }),
                _ => None,
            }}
            <Output store=store spec=spec tests=tests />
            <VerdictPanel submit=submit />
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
fn Output(
    store: BlockStore,
    spec: Option<StoredValue<TestSpec>>,
    tests: Option<TestsState>,
) -> impl IntoView {
    view! {
        {move || {
            let state = store.state.get();
            if let Some(error) = &state.error {
                return error_panel(error).into_any();
            }
            if let Some(result) = &state.result {
                let expected = match (spec, tests) {
                    (Some(spec), Some(tests)) => {
                        logic::expected_for(&spec.read_value(), tests.active_case.get())
                    }
                    _ => None,
                };
                return result_panel(result, expected.as_deref()).into_any();
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

/// With an expected output the stdout is JUDGED (the wb-legend tint); without one it renders
/// plain — exactly the oracle's split.
fn result_panel(result: &RunResult, expected: Option<&str>) -> impl IntoView + use<> {
    let verdict = expected.map(|e| judge(result, Some(e)));
    let (badge_label, badge_ok) = match verdict {
        Some(Verdict::Accepted) => ("Accepted ✓".to_owned(), true),
        Some(Verdict::WrongAnswer) => ("Wrong answer ✗".to_owned(), false),
        _ => (result.status.label().to_owned(), result.status.is_success()),
    };
    let badge_class = if badge_ok {
        "runnable__badge runnable__badge--ok"
    } else {
        "runnable__badge runnable__badge--fail"
    };
    let stdout_class = match verdict {
        Some(Verdict::Accepted) => "runnable__stdout wb-legend--ok",
        Some(Verdict::WrongAnswer) => "runnable__stdout wb-legend--err",
        _ => "runnable__stdout",
    };
    let time = result.time_seconds.map(|s| format!("{s:.3} s"));
    let memory = result.memory_kb.map(|kb| format!("{} MB", kb / 1024));
    let stdout = result.stdout.clone();
    view! {
        <div class="runnable__out">
            <div class="runnable__status">
                <span class=badge_class>{badge_label}</span>
                {time.map(|t| view! { <span class="runnable__meta">{t}</span> })}
                {memory.map(|m| view! { <span class="runnable__meta">{m}</span> })}
            </div>
            {stream_block("compile output", &result.compile_output)}
            {stream_block("stderr", &result.stderr)}
            {if stdout.is_empty() {
                view! { <p class="runnable__empty">"(no output)"</p> }.into_any()
            } else {
                view! { <pre class=stdout_class>{stdout}</pre> }.into_any()
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
