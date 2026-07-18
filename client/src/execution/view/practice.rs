//! The embedded practice-problem widget (oracle: `PracticeProblem.scala` +
//! `MarkdownView.mountBlocks`, docs/embedded-practice-problems.md — the FINAL post-33
//! design, grown here with APPROACH TABS): a two-pane `.pwb--embedded` card inline at the
//! reading-column width — Description/Editorial tabs on the left (the editorial itself
//! splitting into Brute Force / Optimal approach tabs when authored), the reused workbench
//! (Run only — no Submit) on the right, a draggable splitter, and an Enlarge toggle that
//! CSS-promotes the SAME live panes to a near-fullscreen modal (Monaco and all state
//! survive). Copy-to-editor in a solution lands in the workbench tab MATCHING the
//! solution's language.

use std::any::Any;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::execution::logic::{self, PracticeSpec};
use crate::execution::view::RunnableBlock;
use crate::identity::state::AuthStore;
use crate::islands::{editor, markdown};

// ─────────────────────────────────────────────────────────────────────────────
// DISCOVERY
// One mount per `.practice-problem` placeholder; the title comes from the nearest
// preceding heading's "Practice: <Topic>" tail (fallbacks: the heading, "Your Turn").
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)] // the out-of-tree store caravan
pub fn hydrate_practices(
    root: &web_sys::HtmlElement,
    lesson_path: &[String],
    auth: AuthStore,
    code_sink: RwSignal<(String, String)>,
    theme: crate::shell::theme::ThemeStore,
    viz_modal: crate::viz::modal::VizModalStore,
    codebench: super::CodebenchStore,
) -> Vec<Box<dyn Any>> {
    let mut handles: Vec<Box<dyn Any>> = Vec::new();
    let Ok(nodes) = root.query_selector_all("div.practice-problem") else {
        return handles;
    };
    for index in 0..nodes.length() {
        let Some(node) = nodes.get(index) else { continue };
        let Ok(element) = node.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        let attr = |name: &str| {
            element
                .get_attribute(name)
                .and_then(|encoded| js_sys::decode_uri_component(&encoded).ok())
                .map(String::from)
        };
        let (Some(problem), Some(variants)) = (attr("data-problem"), attr("data-variants")) else {
            continue;
        };
        let Some(spec) = logic::decode_practice(
            &problem,
            &variants,
            attr("data-spec").as_deref(),
            attr("data-editorials").as_deref(),
        ) else {
            continue;
        };
        let title = practice_title(&element);
        let path = lesson_path.to_vec();
        let handle = leptos::mount::mount_to(element, move || {
            view! {
                <PracticeProblem
                    spec=spec
                    title=title
                    lesson_path=path
                    auth=auth
                    code_sink=code_sink
                    theme=theme
                    viz_modal=viz_modal
                    codebench=codebench
                />
            }
        });
        handles.push(Box::new(handle));
    }
    handles
}

/// Walk back to the nearest heading; take the text after "Practice:", else the heading,
/// else "Your Turn".
fn practice_title(element: &web_sys::HtmlElement) -> String {
    let mut cur = element.previous_element_sibling();
    while let Some(el) = cur {
        if matches!(el.tag_name().as_str(), "H1" | "H2" | "H3" | "H4" | "H5" | "H6") {
            let text = el.text_content().unwrap_or_default();
            let title = text
                .split_once("Practice:")
                .map_or_else(|| text.trim().to_owned(), |(_, tail)| tail.trim().to_owned());
            return if title.is_empty() {
                "Your Turn".to_owned()
            } else {
                title
            };
        }
        cur = el.previous_element_sibling();
    }
    "Your Turn".to_owned()
}

// ─────────────────────────────────────────────────────────────────────────────
// THE WIDGET
// ─────────────────────────────────────────────────────────────────────────────

// Component props are moved by design (leptos owns them for the view's lifetime).
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
#[component]
pub fn PracticeProblem(
    spec: PracticeSpec,
    title: String,
    lesson_path: Vec<String>,
    auth: AuthStore,
    code_sink: RwSignal<(String, String)>,
    theme: crate::shell::theme::ThemeStore,
    viz_modal: crate::viz::modal::VizModalStore,
    codebench: super::CodebenchStore,
) -> impl IntoView {
    let expanded = RwSignal::new(false);
    // 0 = Description; 1.. = the editorial approaches.
    let tab = RwSignal::new(0_usize);
    let seen = RwSignal::new(1_usize); // panes 0..seen have mounted (editorials are lazy)
    // Copy-to-editor seam: (tick, language, code) — the tick makes re-copies fire.
    let load_code = RwSignal::new((0_u32, String::new(), String::new()));
    let left_pct = RwSignal::new(46.0_f64);
    let panes_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    // Escape collapses only an actually-open modal (per instance — widgets don't interfere).
    let esc = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" && expanded.get_untracked() {
            expanded.set(false);
        }
    });
    on_cleanup(move || esc.remove());

    // The splitter drag: document-level move/up so the pointer can outrun the 9px rail.
    let dragging = StoredValue::new(false);
    let moved = window_event_listener(leptos::ev::pointermove, move |event| {
        if !dragging.get_value() {
            return;
        }
        let Some(panes) = panes_ref.get_untracked() else {
            return;
        };
        let rect = panes.get_bounding_client_rect();
        if rect.width() <= 0.0 {
            return;
        }
        let pct = (f64::from(event.client_x()) - rect.left()) / rect.width() * 100.0;
        left_pct.set(pct.clamp(28.0, 64.0));
    });
    let released = window_event_listener(leptos::ev::pointerup, move |_| dragging.set_value(false));
    on_cleanup(move || {
        moved.remove();
        released.remove();
    });

    let approaches = StoredValue::new(spec.editorials.clone());
    let approach_count = spec.editorials.len();
    let variants = spec.variants.clone();
    let tests = spec.spec.clone();
    let select = move |i: usize| {
        tab.set(i);
        seen.update(|s| *s = (*s).max(i + 1));
    };

    // The editorial tab label: bare single editorial reads "Editorial"; approach-tagged ones
    // become their own tabs (Brute Force · Optimal · …).
    let approach_tabs: Vec<_> = (0..approach_count)
        .map(|i| {
            let label = approaches.read_value()[i].label.clone();
            view! {
                <button
                    class="problem-tab"
                    class:problem-tab--active=move || tab.get() == i + 1
                    on:click=move |_| select(i + 1)
                >
                    {bulb_icon()}
                    {label}
                </button>
            }
        })
        .collect();

    view! {
        <div class="pwb pwb--embedded" class:pwb--expanded=move || expanded.get()>
            <div class="pwb__scrim" on:click=move |_| expanded.set(false)></div>
            <div class="pwb__panes" node_ref=panes_ref>
                <div class="pwb__left" style=move || format!("width: {:.2}%", left_pct.get())>
                    <div class="pwb__head">
                        <span class="pwb__badge">"PRACTICE"</span>
                        <div class="pwb__title">{title}</div>
                    </div>
                    <div class="problem-tabs">
                        <button
                            class="problem-tab"
                            class:problem-tab--active=move || tab.get() == 0
                            on:click=move |_| select(0)
                        >
                            {book_icon()}
                            "Description"
                        </button>
                        {approach_tabs}
                    </div>
                    <div class="pwb__pane-scroll">
                        <div class="pwb__pane" class:hidden=move || tab.get() != 0>
                            {markdown_pane(spec.problem_md.clone(), None, theme, codebench)}
                        </div>
                        // Lazy: each editorial approach (and its Monaco solution viewers)
                        // mounts on first open, then only toggles visibility.
                        {(0..approach_count)
                            .map(|i| {
                                view! {
                                    {move || (seen.get() > i + 1).then(|| {
                                        let md = approaches.read_value()[i].md.clone();
                                        view! {
                                            <div class="pwb__pane" class:hidden=move || tab.get() != i + 1>
                                                {markdown_pane(md, Some(load_code), theme, codebench)}
                                            </div>
                                        }
                                    })}
                                }
                            })
                            .collect::<Vec<_>>()}
                    </div>
                </div>
                <div
                    class="wb-split"
                    on:pointerdown=move |event| {
                        event.prevent_default();
                        dragging.set_value(true);
                    }
                >
                    <div class="wb-split__grip"><span></span><span></span><span></span></div>
                </div>
                <div class="pwb__right">
                    <RunnableBlock
                        variants=variants
                        spec=tests
                        lesson_path=lesson_path
                        auth=auth
                        code_sink=code_sink
                        theme=theme
                        viz_modal=viz_modal
                        practice=true
                        load_code=load_code
                    />
                </div>
                <button
                    class="pwb__enlarge"
                    aria-label=move || {
                        if expanded.get() { "Close fullscreen" } else { "Enlarge to fullscreen" }
                    }
                    on:click=move |_| expanded.update(|e| *e = !*e)
                >
                    {move || if expanded.get() { "✕ Close" } else { "⤢ Enlarge" }}
                </button>
            </div>
        </div>
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MARKDOWN PANES
// The statement/editorial render through the SAME TS pipeline as the lesson body; an
// editorial pane additionally hydrates its `.solution-block` placeholders as revealed
// solution viewers (oracle: `revealSolutions = true`).
// ─────────────────────────────────────────────────────────────────────────────

fn markdown_pane(
    md: String,
    reveal_solutions: Option<RwSignal<(u32, String, String)>>,
    theme: crate::shell::theme::ThemeStore,
    codebench: super::CodebenchStore,
) -> impl IntoView {
    let node_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let mounts: StoredValue<Vec<Box<dyn Any>>, LocalStorage> = StoredValue::new_local(Vec::new());
    Effect::new(move |ran: Option<bool>| {
        if ran == Some(true) {
            return true;
        }
        let Some(node) = node_ref.get() else { return false };
        let md = md.clone();
        spawn_local(async move {
            match markdown::render(&md).await {
                Ok(html) => {
                    node.set_inner_html(&html);
                    if let Some(load_code) = reveal_solutions {
                        mounts.update_value(|m| m.extend(mount_solutions(&node, load_code, theme)));
                    }
                    mounts.update_value(|m| {
                        m.extend(super::hydrate_codebench_pills(&node, codebench));
                    });
                }
                Err(error) => {
                    leptos::logging::error!("practice markdown failed: {error:?}");
                    node.set_text_content(Some(&md));
                }
            }
        });
        true
    });
    on_cleanup(move || mounts.set_value(Vec::new()));
    view! { <div class="pwb__md" node_ref=node_ref></div> }
}

/// Every `.solution-block` in an editorial becomes a revealed read-only viewer with the
/// complexity chips and the Copy-to-editor seam.
pub(crate) fn mount_solutions(
    root: &web_sys::HtmlElement,
    load_code: RwSignal<(u32, String, String)>,
    theme: crate::shell::theme::ThemeStore,
) -> Vec<Box<dyn Any>> {
    let mut handles: Vec<Box<dyn Any>> = Vec::new();
    let Ok(nodes) = root.query_selector_all("div.solution-block") else {
        return handles;
    };
    for index in 0..nodes.length() {
        let Some(node) = nodes.get(index) else { continue };
        let Ok(element) = node.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        let attr = |name: &str| {
            element
                .get_attribute(name)
                .and_then(|encoded| js_sys::decode_uri_component(&encoded).ok())
                .map(String::from)
        };
        let Some(variants) = attr("data-variants").and_then(|json| logic::parse_variants(&json)) else {
            continue;
        };
        // A solution group's languages share ONE viewer behind the same language dropdown as
        // the editor pane — Copy-to-editor stays language-exact (it sends the ACTIVE tab).
        // The fence metas' time/space chips deliberately stay off the header row: the
        // editorial's Complexity Analysis section already states them (user rule).
        let handle = leptos::mount::mount_to(element, move || {
            view! { <SolutionViewer variants=variants load_code=load_code theme=theme /> }.into_any()
        });
        handles.push(Box::new(handle));
    }
    handles
}

#[allow(clippy::needless_pass_by_value)]
#[component]
fn SolutionViewer(
    variants: Vec<logic::Variant>,
    load_code: RwSignal<(u32, String, String)>,
    theme: crate::shell::theme::ThemeStore,
) -> impl IntoView {
    let node_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let mounted: StoredValue<Option<editor::MountedEditor>, LocalStorage> = StoredValue::new_local(None);
    let active = RwSignal::new(0_usize);
    let lang_count = variants.len();
    let first = variants[0].clone();
    let variants = StoredValue::new(variants);
    let variant_at = move |i: usize| variants.read_value()[i.min(lang_count - 1)].clone();

    let mount_source = first.source.clone();
    let mount_lang = first.language.clone();
    Effect::new(move |_| {
        let Some(node) = node_ref.get() else { return };
        if mounted.read_value().is_some() {
            return;
        }
        let value = mount_source.clone();
        let lang = mount_lang.clone();
        let dark = theme.is_dark();
        spawn_local(async move {
            let callbacks = editor::EditorCallbacks {
                on_change: Box::new(|_| {}),
                on_run: Box::new(|| {}),
                on_toggle_edit: Box::new(|| {}),
                on_submit: None,
            };
            match editor::mount(&node, &value, &lang, true, dark, callbacks).await {
                Ok(handle) => mounted.set_value(Some(handle)),
                Err(error) => leptos::logging::error!("solution viewer monaco failed: {error:?}"),
            }
        });
    });
    // A viewer inside a collapsed editorial section mounts 0×0 and renders no lines. The
    // section pills broadcast on reveal; re-measuring here is what makes the code appear.
    let relayout = window_event_listener_untyped(editor::RELAYOUT_EVENT, move |_| {
        mounted.with_value(|editor| {
            if let Some(editor) = editor {
                editor.relayout();
            }
        });
    });
    on_cleanup(move || relayout.remove());
    on_cleanup(move || mounted.set_value(None));

    // Switching languages swaps the ONE read-only Monaco in place (the editor pane's
    // pattern): buffer + tokenizer follow the picked variant.
    let switch_to = move |i: usize| {
        if i == active.get_untracked() {
            return;
        }
        active.set(i);
        let variant = variant_at(i);
        mounted.with_value(|editor| {
            if let Some(editor) = editor {
                editor.set_value(&variant.source);
                editor.set_language(&variant.language);
            }
        });
    };

    // The SAME dropdown chrome as the workbench's language pill — one look for "pick the
    // language" on both sides of the split.
    let lang_chrome = if lang_count > 1 {
        let menu_open = RwSignal::new(false);
        view! {
            <div class="wb__lang">
                <button
                    class="wb__lang-pill wb__lang-pill--btn"
                    aria-label="Solution language"
                    on:click=move |_| menu_open.update(|o| *o = !*o)
                >
                    <span>{move || logic::display_lang(&variant_at(active.get()).language)}</span>
                    {super::icons::icon_chevron_down()}
                </button>
                {move || {
                    menu_open.get().then(|| {
                        let options: Vec<_> = (0..lang_count)
                            .map(|i| {
                                let label = logic::display_lang(&variant_at(i).language);
                                view! {
                                    <button
                                        class="wb__lang-opt"
                                        class:wb__lang-opt--active=move || active.get() == i
                                        on:click=move |_| {
                                            switch_to(i);
                                            menu_open.set(false);
                                        }
                                    >
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
        view! { <span class="wb__lang-pill">{pill}</span> }.into_any()
    };

    // The editor grows to the largest variant so switching never clips or reflows the page.
    let height = variants
        .read_value()
        .iter()
        .map(|v| editor::default_height_px(&v.source))
        .max()
        .unwrap_or_else(|| editor::default_height_px(&first.source));
    let height = format!("height: {height}px;");
    view! {
        <div class="runnable not-prose solution">
            <div class="runnable__bar">
                <span class="wb__eyebrow"><span class="wb__prompt">"✓"</span>" SOLUTION"</span>
                <span class="wb__actions">
                    {lang_chrome}
                    <button
                        class="wb__ghost"
                        title="Load this solution into its language tab on the right"
                        on:click=move |_| {
                            let variant = variant_at(active.get_untracked());
                            load_code.update(|(tick, slot_lang, slot)| {
                                *tick += 1;
                                *slot_lang = variant.language;
                                *slot = variant.source;
                            });
                        }
                    >
                        "Copy to editor"
                    </button>
                </span>
            </div>
            <div class="runnable__editor" style=height node_ref=node_ref></div>
        </div>
    }
}

fn book_icon() -> impl IntoView {
    view! {
        <svg class="problem-tab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"></path>
            <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"></path>
        </svg>
    }
}

fn bulb_icon() -> impl IntoView {
    view! {
        <svg class="problem-tab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M15 14c.2-1 .7-1.7 1.5-2.5a6 6 0 1 0-9 0c.8.8 1.3 1.5 1.5 2.5"></path>
            <path d="M9 18h6"></path>
            <path d="M10 22h4"></path>
        </svg>
    }
}
