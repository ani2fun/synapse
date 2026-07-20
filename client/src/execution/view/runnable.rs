//! One runnable code block (oracle: `Workbench`): toolbar (eyebrow · language tabs · Edit ·
//! Run · Submit), the Monaco island, the tests panel when the block carries an authored
//! suite, the output panel (judged against the active case), and the verdict panel.
//! Multi-variant (step 30): adjacent run fences are LANGUAGE TABS over ONE Monaco —
//! each variant keeps its own buffer/run state in its own `BlockStore`; switching swaps the
//! editor's value + tokenizer in place. Practice mode drops Submit (embedded practice is
//! Edit + Run only) and accepts the editorial's Copy-to-editor feed, which lands in the tab
//! MATCHING the solution's language.

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::execution::{TestSpec, judge, stdin_for};

use crate::execution::logic::{self, ExecutorState, RunHandle, RunState, Variant};
use crate::execution::state::{BlockStore, SubmitState, SubmitStore, lang_pref};
use crate::execution::view::icons::{
    icon_chevron_down, icon_eye, icon_lock, icon_play, icon_reset, icon_rocket,
};
use crate::execution::view::lazy;
use crate::execution::view::output::Output;
use crate::execution::view::workbench::{CaseRequest, TestsPanel, TestsState, VerdictPanel, no_case_yet};
use crate::hydration::IslandStores;
use crate::islands::editor::{self, MountedEditor};

// Component props are moved by design (leptos owns them for the view's lifetime); the length
// is the component's cohesive wiring — props → stores → callbacks → view — and splitting it
// would hide the flow (the oracle keeps `Workbench.apply` as one unit too).
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
#[component]
pub fn RunnableBlock(
    variants: Vec<Variant>,
    spec: Option<TestSpec>,
    lesson_path: Vec<String>,
    // The Coach's snapshot of what the learner sees: (source, language), kept current on
    // every edit; the pane reads it only at send time.
    code_sink: RwSignal<(String, String)>,
    // Captured in-tree, carried out-of-tree — see `crate::hydration::IslandStores`.
    stores: IslandStores,
    /// Embedded practice (step 30): Run only — the Submit verb never renders.
    #[prop(optional)]
    practice: bool,
    /// The editorial's Copy-to-editor feed: `(tick, language, code)` — the tick makes
    /// re-copies of the same code fire; the language picks (and switches to) its tab.
    #[prop(optional)]
    load_code: Option<RwSignal<(u32, String, String)>>,
    /// Bumped when a submit lifecycle completes — the Submissions tab refetches on it.
    #[prop(optional)]
    submitted: Option<RwSignal<u32>>,
    /// The problem page's Submissions rows push failing inputs here. Absent on lesson pages,
    /// where the verdict panel's own button is the only producer, so the block mints its own.
    #[prop(optional)]
    use_case: Option<RwSignal<CaseRequest>>,
    /// The problem page's right pane: the editor FILLS the pane's free height by default
    /// (the `--fill` class the CSS targets) until the resize strip pins an explicit height.
    #[prop(optional)]
    fill: bool,
) -> impl IntoView {
    let IslandStores {
        auth,
        theme,
        viz_modal,
        ..
    } = stores;
    let block_stores: Vec<BlockStore> = variants.iter().map(|v| BlockStore::new(&v.source)).collect();
    // The reader's remembered language, resolved ONCE — `first` then carries it to the shiki
    // placeholder and the default height, which both used to assume variant 0.
    let start = lang_pref::index_for(&variants);
    let active = RwSignal::new(start);
    code_sink.set((variants[start].source.clone(), variants[start].language.clone()));
    let submit = SubmitStore::new();
    let first = variants[start].clone();
    let variants = StoredValue::new(variants);
    // A SIGNAL, not a StoredValue: the panel's suite is the authored cases PLUS any the learner
    // appends from a failed submission (step 63), so the chip row has to react to its length.
    let spec = spec.map(RwSignal::new);
    let tests = spec.map(|s| s.with_untracked(TestsState::new));
    let use_case = use_case.unwrap_or_else(|| RwSignal::new(no_case_yet()));
    let has_submit = spec.is_some() && !practice;
    let lesson_path = StoredValue::new(lesson_path);
    let mounted: StoredValue<Option<MountedEditor>, LocalStorage> = StoredValue::new_local(None);
    let editor_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    // Viewport-lazy state (qna Q1, option B): who's near, who asked for the editor, whether
    // Monaco is actually up, and the shiki placeholder shown until it is.
    let near = RwSignal::new(false);
    let wants_editor = RwSignal::new(false);
    let is_mounted = RwSignal::new(false);
    let preview_html: RwSignal<Option<String>> = RwSignal::new(None);

    let store_at = {
        let block_stores = block_stores.clone();
        move |i: usize| block_stores[i.min(block_stores.len() - 1)]
    };
    let active_store = {
        let store_at = store_at.clone();
        move || store_at(active.get_untracked())
    };
    let variant_at = move |i: usize| variants.read_value()[i].clone();

    // The Run seam: with a suite, stdin is the active case's values through the SHARED shape.
    let stdin = move || match (spec, tests) {
        (Some(spec), Some(tests)) => Some(stdin_for(
            &spec.read_untracked().args,
            &tests.values.get_untracked(),
        )),
        _ => None,
    };
    let run = {
        let active_store = active_store.clone();
        move || {
            // Pin the case this launch answers for — the arriving result is judged against
            // IT, not against whichever chip is selected by the time the reply lands.
            if let Some(tests) = tests {
                tests.ran_case.set(Some(tests.active_case.get_untracked()));
            }
            active_store().launch(variant_at(active.get_untracked()).language, stdin());
        }
    };
    // Record the per-case verdict the moment a judged result lands (oracle: the run callback's
    // `verdicts.update`): sparse map — only cases actually Run ever carry a chip badge.
    if let (Some(spec), Some(tests)) = (spec, tests) {
        let store_at = store_at.clone();
        Effect::new(move |last: Option<Option<RunHandle>>| {
            let state = store_at(active.get()).state.get();
            let seen = last.flatten();
            if state.run_state != RunState::Done || seen == Some(state.run_id) {
                return seen;
            }
            if let (Some(result), Some(case)) = (&state.result, tests.ran_case.get_untracked()) {
                // UNTRACKED — a tracked read would make this Effect a dependent of the spec and
                // re-run it on every appended case.
                let expected = logic::expected_for(&spec.read_untracked(), case);
                let verdict = judge(result, expected.as_deref());
                tests.verdicts.update(|map| {
                    map.insert(case, verdict);
                });
            }
            Some(state.run_id)
        });
    }
    let do_submit = {
        let active_store = active_store.clone();
        move || {
            // The auth gate covers the ⇧⌘⏎ keymap path too — the button is merely disabled.
            if !has_submit || !auth.authed() {
                return;
            }
            submit.submit(
                lesson_path.read_value().clone(),
                variant_at(active.get_untracked()).language,
                active_store().state.get_untracked().code,
            );
        }
    };

    let run_click = run.clone();
    let submit_click = do_submit.clone();

    // Switching tabs swaps the ONE Monaco in place: buffer, tokenizer, read-only state —
    // each variant's edits live on in its own store.
    let switch_to = {
        let store_at = store_at.clone();
        move |i: usize| {
            if i == active.get_untracked() {
                return;
            }
            wants_editor.set(true); // picking a language is an interaction — mount for real
            active.set(i);
            let store = store_at(i);
            let variant = variant_at(i);
            let code = store.state.get_untracked().code;
            mounted.with_value(|editor| {
                if let Some(editor) = editor {
                    editor.set_value(&code);
                    editor.set_language(&variant.language);
                    editor.set_read_only(!store.unlocked.get_untracked());
                }
            });
            code_sink.set((code, variant.language));
        }
    };

    // Visualise (step 28): a Python/Java variant with a viz= hint traces through the SAME
    // /api/run and plays in the modal. The stdin snapshot mirrors Run's.
    let visualisable = Memo::new(move |_| logic::can_visualise(&variants.read_value()[active.get()]));
    let open_visualise = {
        let active_store = active_store.clone();
        move |_| {
            let variant = variant_at(active.get_untracked());
            let Some(hint) = variant.viz else { return };
            let Some((structure, root)) = crate::viz::engine::vocabulary::VizStructure::parse(&hint) else {
                return;
            };
            let key = crate::viz::session::Key {
                language: variant.language,
                source: active_store().state.get_untracked().code,
                structure,
                root,
                stdin: stdin().unwrap_or_default(),
            };
            viz_modal.open(crate::viz::session::obtain(key));
        }
    };

    // Mount monaco once the container exists; the handle + closures live in `mounted` and are
    // dropped (→ disposed) when the block unmounts.
    //
    // VIEWPORT-LAZY (qna Q1, option B): Monaco mounts only when the block is NEAR the
    // viewport (or on first interaction) — until then the container shows the shiki
    // placeholder. A page-level cap evicts the oldest FAR editor; the store keeps all state,
    // so re-approaching re-mounts losslessly over the ACTIVE variant's live buffer.
    let root_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let watch: StoredValue<Option<lazy::NearWatch>, LocalStorage> = StoredValue::new_local(None);
    let registry_id: StoredValue<Option<u64>> = StoredValue::new(None);
    {
        let source = first.source.clone();
        let language = first.language.clone();
        spawn_local(async move {
            if let Ok(html) = crate::islands::markdown::highlight(&source, &language).await {
                preview_html.set(Some(html));
            }
        });
    }
    Effect::new(move |_| {
        let Some(node) = root_ref.get() else { return };
        if watch.read_value().is_none() {
            watch.set_value(lazy::watch_near(&node, near));
        }
    });
    Effect::new(move |_| {
        if near.get() && !is_mounted.get() {
            wants_editor.set(true);
        }
    });
    on_cleanup(move || {
        if let Some(id) = registry_id.get_value() {
            lazy::deregister(id);
        }
        watch.set_value(None);
    });
    {
        let run = run.clone();
        let do_submit = do_submit.clone();
        let store_at = store_at.clone();
        let evict_store_at = store_at.clone();
        // Eviction: drop the editor (Drop disposes monaco), refresh the placeholder from the
        // LIVE buffer, and re-arm the lazy mount for the next approach.
        let evict = Callback::new(move |()| {
            mounted.set_value(None);
            is_mounted.set(false);
            wants_editor.set(false);
            registry_id.set_value(None);
            let i = active.get_untracked();
            let code = evict_store_at(i).state.get_untracked().code;
            let lang = variants.read_value()[i].language.clone();
            spawn_local(async move {
                if let Ok(html) = crate::islands::markdown::highlight(&code, &lang).await {
                    preview_html.set(Some(html));
                }
            });
        });
        Effect::new(move |_| {
            if !wants_editor.get() {
                return;
            }
            let Some(node) = editor_ref.get() else { return };
            if mounted.read_value().is_some() {
                return;
            }
            // The ACTIVE variant, not variant 0 — a re-mount after tab switch + eviction
            // must restore what the reader was on.
            let i = active.get_untracked();
            let store = store_at(i);
            let value = store.state.get_untracked().code;
            let lang = variant_at(i).language;
            let read_only = !store.unlocked.get_untracked();
            let run = run.clone();
            let do_submit = do_submit.clone();
            let store_at = store_at.clone();
            spawn_local(async move {
                let change_store = store_at.clone();
                let toggle_store = store_at.clone();
                let callbacks = editor::EditorCallbacks {
                    on_change: Box::new(move |code: String| {
                        change_store(active.get_untracked())
                            .state
                            .update(|s| *s = s.set_code(&code));
                        code_sink.update(|(source, _)| *source = code);
                    }),
                    on_run: Box::new(run),
                    on_toggle_edit: Box::new(move || {
                        // The auth gate (oracle: canEditSignal = authed && unlocked).
                        if !auth.authed() {
                            return;
                        }
                        let i = active.get_untracked();
                        let store = toggle_store(i);
                        store.toggle_edit(&variants.read_value()[i].source);
                        sync_editor(mounted, store);
                    }),
                    on_submit: has_submit.then(move || Box::new(do_submit) as Box<dyn FnMut()>),
                };
                let dark = theme.is_dark();
                match editor::mount(&node, &value, &lang, read_only, dark, callbacks).await {
                    Ok(handle) => {
                        mounted.set_value(Some(handle));
                        is_mounted.set(true);
                        registry_id.set_value(Some(lazy::register(near, evict)));
                    }
                    Err(error) => leptos::logging::error!("monaco island failed: {error:?}"),
                }
            });
        });
    }
    // Monaco paints its own canvas — the toggle re-themes it (setTheme is global+idempotent).
    Effect::new(move |_| {
        let dark = theme.mode.get() == crate::shell::theme::Mode::Dark;
        mounted.with_value(|editor| {
            if let Some(editor) = editor {
                editor.set_theme(dark);
            }
        });
    });
    // The editorial's Copy-to-editor: land in the tab matching the solution's language
    // (fall back to the active tab), overwrite that buffer, and show it.
    if let Some(load) = load_code {
        let store_at = store_at.clone();
        let switch_to = switch_to.clone();
        Effect::new(move |prev: Option<u32>| {
            let (tick, lang, code) = load.get();
            if tick == 0 || prev == Some(tick) {
                return tick;
            }
            // Canonical, not raw: a `python3` solution must find the `py` tab. Guarded on
            // `is_some` so two UNKNOWN languages don't both read as `None` and match.
            let wanted = logic::canonical_lang(&lang);
            let target = variants
                .read_value()
                .iter()
                .position(|v| wanted.is_some() && logic::canonical_lang(&v.language) == wanted)
                .unwrap_or_else(|| active.get_untracked());
            crate::log::debug(&format!(
                "solution copied into the {} tab",
                variants.read_value()[target].language
            ));
            switch_to(target);
            store_at(target).state.update(|s| *s = s.set_code(&code));
            mounted.with_value(|editor| {
                if let Some(editor) = editor {
                    editor.set_value(&code);
                }
            });
            code_sink.set((code, variant_at(target).language));
            tick
        });
    }
    on_cleanup(move || mounted.set_value(None));

    let running = {
        let store_at = store_at.clone();
        Memo::new(move |_| store_at(active.get()).state.read().run_state == RunState::Running)
    };
    let judging = Memo::new(move |_| matches!(submit.state.get(), SubmitState::Judging(_)));
    if let Some(submitted) = submitted {
        Effect::new(move |was: Option<bool>| {
            let done = matches!(submit.state.get(), SubmitState::Done(_));
            if done && was != Some(true) {
                submitted.update(|n| *n += 1);
            }
            done
        });
    }
    let unlocked = {
        let store_at = store_at.clone();
        Memo::new(move |_| store_at(active.get()).unlocked.get())
    };
    // The editor's height story: a source-derived default; in `fill` mode the CSS stretches
    // it into the pane's free height until the resize strip pins an explicit height (the
    // pinned value drops the `--fill` class, so the inline style wins again).
    let default_height = editor::default_height_px(&first.source);
    let pinned_height: RwSignal<Option<f64>> = RwSignal::new(None);
    let height = move || {
        format!(
            "height: {}px;",
            pinned_height.get().unwrap_or(f64::from(default_height))
        )
    };
    // The horizontal resize strip's drag (the wb-split pattern, turned 90°): document-level
    // move/up so the pointer can outrun the 9px rail; the grab records the editor's live
    // height so the drag is relative, never a jump.
    let drag_from: StoredValue<Option<(f64, f64)>> = StoredValue::new(None);
    let resize_moved = window_event_listener(leptos::ev::pointermove, move |event| {
        let Some((start_y, start_h)) = drag_from.get_value() else {
            return;
        };
        let next = (start_h + (f64::from(event.client_y()) - start_y)).clamp(140.0, 900.0);
        pinned_height.set(Some(next));
    });
    let resize_released = window_event_listener(leptos::ev::pointerup, move |_| drag_from.set_value(None));
    on_cleanup(move || {
        resize_moved.remove();
        resize_released.remove();
    });
    let active_state: Signal<ExecutorState> = {
        let store_at = store_at.clone();
        Signal::derive(move || store_at(active.get()).state.get())
    };

    // The language pill (oracle B.1): ▶ name (+ chevron dropdown when multi-variant).
    let lang_count = variants.read_value().len();
    let lang_chrome = if lang_count > 1 {
        let menu_open = RwSignal::new(false);
        let menu_switch = switch_to.clone();
        view! {
            <div class="wb__lang">
                <button
                    class="wb__lang-pill wb__lang-pill--btn"
                    aria-label="Language"
                    on:click=move |_| menu_open.update(|o| *o = !*o)
                >
                    {icon_play("wb__lang-play")}
                    <span>{move || logic::display_lang(&variants.read_value()[active.get()].language)}</span>
                    {icon_chevron_down()}
                </button>
                {move || {
                    let menu_switch = menu_switch.clone();
                    menu_open.get().then(|| {
                        let options: Vec<_> = (0..lang_count)
                            .map(|i| {
                                let label = logic::display_lang(&variants.read_value()[i].language);
                                let menu_switch = menu_switch.clone();
                                view! {
                                    <button
                                        class="wb__lang-opt"
                                        class:wb__lang-opt--active=move || active.get() == i
                                        on:click=move |_| {
                                            lang_pref::store(&variants.read_value()[i].language);
                                            menu_switch(i);
                                            menu_open.set(false);
                                        }
                                    >
                                        {icon_play("wb__lang-play")}
                                        {label}
                                    </button>
                                }
                            })
                            .collect();
                        view! {
                            <div>
                                <div class="wb__lang-scrim" on:click=move |_| menu_open.set(false)></div>
                                <div class="wb__lang-menu">{options}</div>
                            </div>
                        }
                    })
                }}
            </div>
        }
        .into_any()
    } else {
        let pill = logic::display_lang(&first.language);
        view! { <span class="wb__lang-pill">{icon_play("wb__lang-play")}<span>{pill}</span></span> }
            .into_any()
    };

    let toggle_store = store_at.clone();
    let reset_store = store_at.clone();
    let toolbar = view! {
        <div class="runnable__bar">
            <span class="wb__eyebrow"><span class="wb__prompt">">_"</span>" CODE"</span>
            <span class="wb__actions">
                {lang_chrome}
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
                            wants_editor.set(true); // editing needs the real editor
                            let i = active.get_untracked();
                            let store = toggle_store(i);
                            store.toggle_edit(&variants.read_value()[i].source);
                            sync_editor(mounted, store);
                        }
                    >
                        {move || if unlocked.get() {
                            view! { <span>"Editing"</span> }.into_any()
                        } else {
                            view! { {icon_lock()}<span>"Edit"</span> }.into_any()
                        }}
                    </button>
                </span>
                // ↺ Reset — restores the starter, only meaningful while editing.
                {move || (unlocked.get() && auth.authed()).then(|| {
                    let reset_store = reset_store.clone();
                    view! {
                        <button
                            class="wb__ghost wb__ghost--live wb__ghost--icon"
                            title="Restore the starter code"
                            aria-label="Reset"
                            on:click=move |_| {
                                let i = active.get_untracked();
                                let store = reset_store(i);
                                let authored = variants.read_value()[i].source.clone();
                                store.state.update(|s| *s = s.set_code(&authored));
                                mounted.with_value(|editor| {
                                    if let Some(editor) = editor {
                                        editor.set_value(&authored);
                                    }
                                });
                            }
                        >
                            {icon_reset()}
                        </button>
                    }
                })}
                {has_submit.then(|| view! {
                    // Anonymous readers see WHY it's off (the step-20 allowlist hint); the
                    // server re-enforces regardless — this is UX, not the gate.
                    <span
                        class="wb__tip"
                        data-tip=move || {
                            if auth.authed() {
                                "Submit against the hidden suite (⇧⌘⏎)"
                            } else {
                                "Sign in to submit. Submit runs your code against every hidden \
                                 test and saves your attempt. Saving needs an approved account \
                                 — ask the operator for access."
                            }
                        }
                    >
                        <button
                            class="wb__submit"
                            prop:disabled=move || !auth.authed() || judging.get()
                            on:click=move |_| submit_click()
                        >
                            {icon_rocket()}
                            <span>{move || if judging.get() { "Judging…" } else { "Submit" }}</span>
                        </button>
                    </span>
                })}
                {move || visualisable.get().then(|| view! {
                    <button
                        class="wb__ghost"
                        title="Trace this code and watch the structure animate"
                        on:click=open_visualise.clone()
                    >
                        {icon_eye()}
                        <span>"Visualise"</span>
                    </button>
                })}
                <button
                    class="runnable__run"
                    title="Run (⌘⏎)"
                    prop:disabled=move || running.get()
                    on:click=move |_| {
                        wants_editor.set(true); // running is an interaction — bring the editor up
                        run_click();
                    }
                >
                    {icon_play("runnable__run-ic")}
                    <span>{move || if running.get() { "Running…" } else { "Run" }}</span>
                </button>
            </span>
        </div>
    };

    view! {
        <div class="runnable not-prose" node_ref=root_ref>
            {toolbar}
            <div
                class=move || {
                    if fill && pinned_height.get().is_none() {
                        "runnable__editor runnable__editor--fill"
                    } else {
                        "runnable__editor"
                    }
                }
                node_ref=editor_ref
                style=height
            >
                // The shiki placeholder — shown until Monaco is up (near-viewport or first
                // interaction); clicking into the code IS an interaction.
                {move || (!is_mounted.get()).then(|| view! {
                    <div
                        class="runnable__preview"
                        on:click=move |_| wants_editor.set(true)
                        inner_html=preview_html.get().unwrap_or_default()
                    ></div>
                })}
                {copy_button(mounted, code_sink)}
            </div>
            // The horizontal resize strip — drag to grow/shrink the editor against the
            // panels below (double-click restores the fill/default height).
            {spec.is_some().then(|| view! {
                <div
                    class="wb-hsplit"
                    title="Drag to resize the editor — double-click to reset"
                    on:pointerdown=move |event| {
                        event.prevent_default();
                        let live = editor_ref
                            .get_untracked()
                            .map_or(f64::from(default_height), |node| {
                                node.get_bounding_client_rect().height()
                            });
                        drag_from.set_value(Some((f64::from(event.client_y()), live)));
                    }
                    on:dblclick=move |_| pinned_height.set(None)
                >
                    <div class="wb-hsplit__grip"><span></span><span></span><span></span></div>
                </div>
            })}
            {match (spec, tests) {
                (Some(spec), Some(tests)) => {
                    // Chip switch clears every variant's stale run output (oracle: switchCase
                    // resets the FSM) — the chips keep their earlier ✓/✗ badges.
                    let clear_stores = block_stores.clone();
                    let on_switch = Callback::new(move |_case: usize| {
                        for store in &clear_stores {
                            store.state.update(|s| *s = s.clear_outcome());
                        }
                    });
                    Some(
                        view! { <TestsPanel spec=spec tests=tests on_switch=on_switch use_case=use_case /> },
                    )
                }
                _ => None,
            }}
            <Output state=active_state spec=spec tests=tests />
            <VerdictPanel submit=submit spec=spec use_case=use_case />
        </div>
    }
}

/// The floating copy-code button (oracle: `MonacoEditor.copyButton`): reads the LIVE
/// buffer, swaps to a check for 1.4 s.
fn copy_button(
    mounted: StoredValue<Option<MountedEditor>, LocalStorage>,
    code_sink: RwSignal<(String, String)>,
) -> impl IntoView {
    let copied = RwSignal::new(false);
    view! {
        <button
            class="editor-copy"
            class:editor-copy--done=move || copied.get()
            aria-label="Copy code"
            title="Copy code"
            on:click=move |_| {
                // Live Monaco buffer when mounted; the store's code (same text) before the
                // lazy mount — the placeholder's copy must work too.
                let code = mounted
                    .with_value(|e| e.as_ref().map(MountedEditor::get_value))
                    .or_else(|| Some(code_sink.get_untracked().0));
                if let Some(code) = code {
                    if let Some(window) = web_sys::window() {
                        let _ = window.navigator().clipboard().write_text(&code);
                    }
                    copied.set(true);
                    gloo_timers::callback::Timeout::new(1_400, move || copied.set(false)).forget();
                }
            }
        >
            {move || if copied.get() {
                view! {
                    <svg class="editor-copy__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                         stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                        <path d="M20 6 9 17l-5-5"></path>
                    </svg>
                }
                .into_any()
            } else {
                view! {
                    <svg class="editor-copy__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                         stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                        <rect x="8" y="8" width="14" height="14" rx="2" ry="2"></rect>
                        <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>
                    </svg>
                }
                .into_any()
            }}
        </button>
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
