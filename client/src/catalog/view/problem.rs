//! The standalone problem page (oracle: `ProblemWorkbench` + `SubmissionsFeed`, steps
//! 16/23 final form): a `kind: problem` lesson renders the two-pane workbench instead of
//! the prose column — breadcrumbs, then the left pane (title · lede ·
//! Description | Editorial | Coach | Submissions tabs · the difficulty badge) beside the
//! right pane (the workbench with its tests panel and the anonymous sign-in note). The
//! FIRST workbench fence is EXTRACTED out of the description into the right pane; panes
//! mount once and toggle `.hidden`, so editor state survives tab switches.

use std::any::Any;

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::catalog::LessonPayloadDto;
use synapse_shared::execution::TestSpec;
use synapse_shared::submission::SubmissionDto;
use wasm_bindgen::JsCast;

use crate::api;
use crate::catalog::logic;
use crate::execution::logic::Variant;
use crate::execution::view::RunnableBlock;
use crate::islands::editor::RELAYOUT_EVENT;
use crate::islands::markdown;

fn humanize(slug: &str) -> String {
    slug.split('-')
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or_else(String::new, |f| {
                f.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Description,
    Editorial,
    Coach,
    Submissions,
}

const TABS: [(Tab, &str); 4] = [
    (Tab::Description, "Description"),
    (Tab::Editorial, "Editorial"),
    (Tab::Coach, "Coach"),
    (Tab::Submissions, "Submissions"),
];

#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
#[component]
pub fn ProblemWorkbench(payload: LessonPayloadDto, segments: Vec<String>) -> impl IntoView {
    let auth = crate::identity::state::AuthStore::from_context();
    let theme = crate::shell::theme::ThemeStore::from_context();
    let viz_modal = crate::viz::modal::VizModalStore::from_context();
    let codebench = crate::execution::view::CodebenchStore::from_context();

    let tab = RwSignal::new(Tab::Description);
    let subs_seen = RwSignal::new(false);
    let left_pct = RwSignal::new(46.0_f64);
    let panes_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    // The extracted FIRST workbench (variants + suite) — the right pane renders it.
    let wb_spec: RwSignal<Option<(Vec<Variant>, Option<TestSpec>)>> = RwSignal::new(None);
    // Copy-to-editor from the editorial's solutions: (tick, language, code).
    let load_code = RwSignal::new((0_u32, String::new(), String::new()));
    // Bumped by the workbench when a submit completes → the feed refetches.
    let submitted = RwSignal::new(0_u32);
    let code_ctx = RwSignal::new((String::new(), String::new()));

    // The splitter drag (28–64%), document-level listeners like the practice widget.
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

    let (desc_md, inline_editorial) = logic::problem_content_split(&payload.raw);
    // The inline (post-<details>) editorial wins; else the co-located sidecar
    // (`<lesson>.editorial.md`, already in the payload).
    let editorial_md = if inline_editorial.trim().is_empty() {
        payload.editorial.clone().unwrap_or_default()
    } else {
        inline_editorial
    };
    let title = payload.frontmatter.title.clone();
    let lede = payload.frontmatter.summary.clone();
    let difficulty = payload.frontmatter.difficulty.clone();
    let book_name = humanize(&payload.book.slug);
    let lesson_title = payload.frontmatter.title.clone();
    let segments = StoredValue::new(segments);

    let tab_buttons: Vec<_> = TABS
        .iter()
        .map(|(t, label)| {
            let t = *t;
            view! {
                <button
                    class="problem-tab"
                    class:problem-tab--active=move || tab.get() == t
                    on:click=move |_| {
                        tab.set(t);
                        if t == Tab::Submissions {
                            subs_seen.set(true);
                        }
                    }
                >
                    {*label}
                </button>
            }
        })
        .collect();

    view! {
        <div class="pwb not-prose">
            <nav class="pwb__crumbs" aria-label="Breadcrumb">
                <a class="pwb__crumb" href="/">"Home"</a>
                <span class="pwb__crumb-sep">"›"</span>
                <span class="pwb__crumb">{book_name}</span>
                <span class="pwb__crumb-sep">"›"</span>
                <span class="pwb__crumb pwb__crumb--current">{lesson_title}</span>
            </nav>
            <div class="pwb__panes" node_ref=panes_ref>
                <div class="pwb__left" style=move || format!("width: {:.2}%", left_pct.get())>
                    <div class="pwb__head">
                        <h1 class="pwb__title">{title}</h1>
                        {lede.map(|l| view! { <p class="pwb__lede">{l}</p> })}
                    </div>
                    <div class="problem-tabs">
                        {tab_buttons}
                        {difficulty.map(|d| {
                            let class = format!("problem-diff problem-diff--{d}");
                            view! { <span class=class>{d.clone()}</span> }
                        })}
                    </div>
                    <div class="pwb__pane-scroll synapse-prose">
                        <div class="pwb__pane" class:hidden=move || tab.get() != Tab::Description>
                            {description_pane(desc_md, wb_spec, segments.read_value().clone(), auth, code_ctx, theme, viz_modal, codebench)}
                        </div>
                        <div class="pwb__pane" class:hidden=move || tab.get() != Tab::Editorial>
                            {editorial_pane(editorial_md, load_code, theme, codebench)}
                        </div>
                        <div class="pwb__pane" class:hidden=move || tab.get() != Tab::Coach>
                            <crate::tutoring::CoachPane
                                problem=Some(segments.read_value().join("/"))
                                code_ctx=code_ctx
                            />
                        </div>
                        <div class="pwb__pane" class:hidden=move || tab.get() != Tab::Submissions>
                            {move || subs_seen.get().then(|| view! {
                                <SubmissionsFeed
                                    path=segments.read_value().clone()
                                    refetch=submitted
                                    load_code=load_code
                                />
                            })}
                        </div>
                    </div>
                </div>
                <div
                    class="wb-split"
                    aria-label="Resize the panes"
                    on:pointerdown=move |event| {
                        event.prevent_default();
                        dragging.set_value(true);
                    }
                >
                    <div class="wb-split__grip"><span></span><span></span><span></span></div>
                </div>
                <div class="pwb__right">
                    {move || match wb_spec.get() {
                        Some((variants, spec)) => view! {
                            <div>
                                {(!auth.authed()).then(|| view! {
                                    <div class="wb__edit-bar">
                                        <span class="wb__edit-status">
                                            <span class="wb__edit-dot"></span>
                                            "Sign in to edit and submit — you can still Run the starter"
                                        </span>
                                    </div>
                                })}
                                <RunnableBlock
                                    variants=variants
                                    spec=spec
                                    lesson_path=segments.read_value().clone()
                                    auth=auth
                                    code_sink=code_ctx
                                    theme=theme
                                    viz_modal=viz_modal
                                    load_code=load_code
                                    submitted=submitted
                                    fill=true
                                />
                            </div>
                        }
                        .into_any(),
                        None => view! { <div class="pwb__nowb">"Loading the workbench…"</div> }.into_any(),
                    }}
                </div>
            </div>
        </div>
    }
}

/// The description: rendered markdown with the FIRST workbench placeholder EXTRACTED into
/// the right pane; everything else hydrates in place.
#[allow(clippy::too_many_arguments)] // the out-of-tree store caravan (same as RunnableBlock)
fn description_pane(
    md: String,
    wb_spec: RwSignal<Option<(Vec<Variant>, Option<TestSpec>)>>,
    segments: Vec<String>,
    auth: crate::identity::state::AuthStore,
    code_ctx: RwSignal<(String, String)>,
    theme: crate::shell::theme::ThemeStore,
    viz_modal: crate::viz::modal::VizModalStore,
    codebench: crate::execution::view::CodebenchStore,
) -> impl IntoView {
    let node_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let mounts: StoredValue<Vec<Box<dyn Any>>, LocalStorage> = StoredValue::new_local(Vec::new());
    Effect::new(move |ran: Option<bool>| {
        if ran == Some(true) {
            return true;
        }
        let Some(node) = node_ref.get() else { return false };
        let md = md.clone();
        let segments = segments.clone();
        spawn_local(async move {
            let Ok(html) = markdown::render(&md).await else {
                node.set_text_content(Some(&md));
                return;
            };
            node.set_inner_html(&html);
            // Extract the FIRST workbench for the right pane, then hydrate the rest.
            if let Ok(Some(first)) = node.query_selector("div.workbench") {
                let decode = |name: &str| {
                    first
                        .get_attribute(name)
                        .and_then(|encoded| js_sys::decode_uri_component(&encoded).ok())
                        .map(String::from)
                };
                let variants = decode("data-variants")
                    .and_then(|json| crate::execution::logic::parse_variants(&json))
                    .filter(|v| !v.is_empty());
                if let Some(variants) = variants {
                    let spec = decode("data-spec").and_then(|json| serde_json::from_str(&json).ok());
                    first.remove();
                    wb_spec.set(Some((variants, spec)));
                }
            }
            let mut handles = crate::execution::view::hydrate_workbenches(
                &node, &segments, auth, code_ctx, theme, viz_modal,
            );
            handles.extend(crate::catalog::view::diagrams::hydrate_diagrams(&node));
            handles.extend(crate::execution::view::hydrate_codebench_pills(&node, codebench));
            handles.extend(crate::viz::blocks::mount_widgets(&node));
            mounts.set_value(handles);
        });
        true
    });
    on_cleanup(move || mounts.set_value(Vec::new()));
    view! { <div class="pwb-description" node_ref=node_ref></div> }
}

/// The editorial: solutions reveal directly (this tab IS the answer); Copy-to-editor routes
/// into the right pane's matching language tab. The `##` headings become a SECOND row of
/// section pills (Intuition · Approach · Solution · …) — everything stays mounted (Monaco
/// state survives; `automaticLayout` re-measures on reveal), switching only toggles CSS.
fn editorial_pane(
    md: String,
    load_code: RwSignal<(u32, String, String)>,
    theme: crate::shell::theme::ThemeStore,
    codebench: crate::execution::view::CodebenchStore,
) -> impl IntoView {
    if md.trim().is_empty() {
        return view! { <p class="psub__note">"No editorial yet for this problem."</p> }.into_any();
    }
    let node_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let mounts: StoredValue<Vec<Box<dyn Any>>, LocalStorage> = StoredValue::new_local(Vec::new());
    let section_labels: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let active_section = RwSignal::new(0_usize);
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
                    // Open the authored <details> spoilers — this tab is the answer.
                    if let Ok(all) = node.query_selector_all("details") {
                        for i in 0..all.length() {
                            if let Some(d) = all.get(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok()) {
                                let _ = d.set_attribute("open", "");
                            }
                        }
                    }
                    section_labels.set(sectionize_editorial(&node));
                    mounts.update_value(|m| {
                        m.extend(crate::execution::view::mount_solutions(&node, load_code, theme));
                        m.extend(crate::execution::view::hydrate_codebench_pills(&node, codebench));
                        // An editorial is authored markdown like any lesson: its diagrams and
                        // widgets need the same hydration the reader and description panes do.
                        m.extend(crate::catalog::view::diagrams::hydrate_diagrams(&node));
                        m.extend(crate::viz::blocks::mount_widgets(&node));
                    });
                }
                Err(_) => node.set_text_content(Some(&md)),
            }
        });
        true
    });
    // Switching pills toggles `.hidden` on the section wrappers directly — they are raw DOM,
    // not Leptos views, so the signal drives them by hand.
    Effect::new(move |_| {
        let active = active_section.get();
        let Some(node) = node_ref.get_untracked() else {
            return;
        };
        let Ok(sections) = node.query_selector_all(".pwb-esec") else {
            return;
        };
        for i in 0..sections.length() {
            if let Some(section) = sections
                .get(i)
                .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
            {
                let list = section.class_list();
                let _ = if (i as usize) == active {
                    list.remove_1("hidden")
                } else {
                    list.add_1("hidden")
                };
            }
        }
        // A revealed section's monaco mounted inside `display: none` and measured 0×0 — it
        // renders no lines until told to re-measure. The viewers listen for this rather than
        // being reached into, so the switcher stays ignorant of what it just revealed.
        if let (Some(window), Ok(event)) = (web_sys::window(), web_sys::Event::new(RELAYOUT_EVENT)) {
            let _ = window.dispatch_event(&event);
        }
    });
    on_cleanup(move || mounts.set_value(Vec::new()));
    view! {
        {move || {
            let labels = section_labels.get();
            (labels.len() > 1).then(|| view! {
                <div class="pwb-esec-tabs">
                    {labels
                        .into_iter()
                        .enumerate()
                        .map(|(i, label)| {
                            view! {
                                <button
                                    class="pwb-esec-tab"
                                    class:pwb-esec-tab--active=move || active_section.get() == i
                                    on:click=move |_| active_section.set(i)
                                >
                                    {label}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            })
        }}
        <div class="pwb-editorial" node_ref=node_ref></div>
    }
    .into_any()
}

/// Group the rendered editorial's DOM into `.pwb-esec` wrappers, one per `h2` (falling back
/// to `h1` when the author used top-level headings): the heading plus everything until the
/// next one. Prose BEFORE the first heading stays put — always visible above the sections.
/// Returns the section labels (empty/one section → no pills, nothing wrapped is hidden).
fn sectionize_editorial(node: &web_sys::HtmlElement) -> Vec<String> {
    let heading_tag = match node.query_selector("h2") {
        Ok(Some(_)) => "H2",
        _ => "H1",
    };
    let children = node.children();
    let snapshot: Vec<web_sys::Element> = (0..children.length()).filter_map(|i| children.item(i)).collect();
    let mut labels: Vec<String> = Vec::new();
    let mut current: Option<web_sys::Element> = None;
    let Some(document) = node.owner_document() else {
        return labels;
    };
    for child in snapshot {
        if child.tag_name() == heading_tag {
            let label = child.text_content().unwrap_or_default().trim().to_owned();
            let Ok(wrapper) = document.create_element("div") else {
                continue;
            };
            wrapper.set_class_name("pwb-esec");
            let _ = node.append_child(&wrapper);
            labels.push(if label.is_empty() {
                format!("Section {}", labels.len() + 1)
            } else {
                label
            });
            current = Some(wrapper);
        }
        if let Some(wrapper) = &current {
            let _ = wrapper.append_child(&child);
        }
    }
    if labels.len() > 1 {
        // Show the first section; hide the rest (the effect keeps this in sync afterwards).
        if let Ok(sections) = node.query_selector_all(".pwb-esec") {
            for i in 1..sections.length() {
                if let Some(section) = sections
                    .get(i)
                    .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
                {
                    let _ = section.class_list().add_1("hidden");
                }
            }
        }
    }
    labels
}

// ─────────────────────────────────────────────────────────────────────────────
// SUBMISSIONS FEED — the caller's own list, auth-gated, refetched on submit
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn SubmissionsFeed(
    path: Vec<String>,
    refetch: RwSignal<u32>,
    load_code: RwSignal<(u32, String, String)>,
) -> impl IntoView {
    let auth = crate::identity::state::AuthStore::from_context();
    let rows: RwSignal<Option<Result<Vec<SubmissionDto>, String>>> = RwSignal::new(None);
    let selected: RwSignal<Option<String>> = RwSignal::new(None);
    let path = StoredValue::new(path);
    Effect::new(move |_| {
        refetch.track();
        if !auth.authed() {
            return;
        }
        let path = path.read_value().clone();
        spawn_local(async move {
            rows.set(Some(api::submissions_for(&path).await));
        });
    });
    view! {
        <div class="psub not-prose">
            {move || {
                if !auth.authed() {
                    return view! {
                        <p class="psub__note">"Sign in to see your submissions — they're private to you."</p>
                    }
                    .into_any();
                }
                match rows.get() {
                    None => view! { <p class="psub__note">"Loading your submissions…"</p> }.into_any(),
                    Some(Err(message)) => view! {
                        <p class="psub__note psub__note--error">"Couldn't load submissions — " {message}</p>
                    }
                    .into_any(),
                    Some(Ok(list)) if list.is_empty() => view! {
                        <p class="psub__note">"No submissions yet — solve it and hit Submit."</p>
                    }
                    .into_any(),
                    Some(Ok(list)) => {
                        let count = list.len();
                        let current: Vec<_> = list.iter().take(1).cloned().collect();
                        let code = selected
                            .get()
                            .and_then(|id| list.iter().find(|d| d.id == id).cloned());
                        view! {
                            <h3 class="psub__section">"Current submission"</h3>
                            {subs_table(&current, selected)}
                            <h3 class="psub__section">"All submissions"</h3>
                            {subs_table(&list, selected)}
                            <p class="psub__note psub__count">{format!("Showing {count} submission(s)")}</p>
                            {code.map(|dto| view! { <CodeCard dto=dto selected=selected load_code=load_code /> })}
                        }
                        .into_any()
                    }
                }
            }}
        </div>
    }
}

fn subs_table(list: &[SubmissionDto], selected: RwSignal<Option<String>>) -> impl IntoView + use<> {
    let rows: Vec<_> = list
        .iter()
        .enumerate()
        .map(|(i, dto)| {
            let (badge_class, badge_text) = match dto.verdict.as_deref() {
                Some("accepted") => ("subs__status subs__status--ok", "Accepted"),
                Some("rejected") => ("subs__status subs__status--fail", "Wrong answer"),
                Some("judge-failed") => ("subs__status subs__status--warn", "Judge failed"),
                _ => ("subs__status", "pending"),
            };
            let cases = match (dto.passed, dto.total) {
                (Some(p), Some(t)) => Some(format!("{p}/{t} cases")),
                _ => None,
            };
            let time = dto.created_at.chars().take(19).collect::<String>().replace('T', " ");
            let id = dto.id.clone();
            let eye_id = dto.id.clone();
            view! {
                <tr class="subs__row">
                    <td class="subs__cell subs__cell--no">{i + 1}</td>
                    <td class="subs__cell">
                        <span class=badge_class>{badge_text}</span>
                        {cases.map(|c| view! { <span class="subs__meta">{c}</span> })}
                        <span class="subs__time">{time}</span>
                    </td>
                    <td class="subs__cell"><span class="subs__lang">{dto.language.clone()}</span></td>
                    <td class="subs__cell subs__cell--action">
                        <button
                            class="subs__icon-btn"
                            class:subs__icon-btn--on=move || selected.get().as_deref() == Some(eye_id.as_str())
                            title="View the code"
                            on:click=move |_| {
                                let id = id.clone();
                                selected.update(|s| {
                                    *s = if s.as_deref() == Some(id.as_str()) { None } else { Some(id) };
                                });
                            }
                        >
                            "👁"
                        </button>
                    </td>
                </tr>
            }
        })
        .collect();
    view! {
        <table class="subs__table">
            <thead>
                <tr>
                    <th>"No."</th>
                    <th>"Status"</th>
                    <th>"Language"</th>
                    <th>"Code"</th>
                </tr>
            </thead>
            <tbody>{rows}</tbody>
        </table>
    }
}

/// A revealed submission's source: shiki-highlighted like every other code block on the site,
/// and loadable straight into the right pane's matching language tab so a past attempt can be
/// re-run without a copy-paste round trip.
#[component]
fn CodeCard(
    dto: SubmissionDto,
    selected: RwSignal<Option<String>>,
    load_code: RwSignal<(u32, String, String)>,
) -> impl IntoView {
    let title = format!("Submission {} · {}", &dto.id[..8.min(dto.id.len())], dto.language);
    let source = StoredValue::new(dto.source.clone());
    let language = StoredValue::new(dto.language.clone());
    let node_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    // Highlight through the same island the markdown pipeline uses, so a submission reads
    // exactly like the editorial's code. A failure falls back to the plain text it replaced.
    Effect::new(move |ran: Option<bool>| {
        if ran == Some(true) {
            return true;
        }
        let Some(node) = node_ref.get() else { return false };
        let (code, lang) = (source.read_value().clone(), language.read_value().clone());
        spawn_local(async move {
            match markdown::highlight(&code, &lang).await {
                Ok(html) => node.set_inner_html(&html),
                Err(error) => {
                    crate::log::warn(&format!("submission highlight failed: {error:?}"));
                    node.set_text_content(Some(&code));
                }
            }
        });
        true
    });
    view! {
        <div class="subs__code">
            <div class="subs__code-head">
                <span class="subs__code-title">{title}</span>
                <span class="subs__code-actions">
                    <button
                        class="wb__ghost"
                        title="Load this submission into its language tab on the right"
                        on:click=move |_| {
                            load_code.update(|(tick, slot_lang, slot)| {
                                *tick += 1;
                                slot_lang.clone_from(&language.read_value());
                                slot.clone_from(&source.read_value());
                            });
                        }
                    >
                        "Copy to editor"
                    </button>
                    <button class="subs__code-close" aria-label="Close" on:click=move |_| selected.set(None)>
                        "×"
                    </button>
                </span>
            </div>
            <div class="subs__pre" node_ref=node_ref></div>
        </div>
    }
}
