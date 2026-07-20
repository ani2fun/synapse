//! The standalone problem page (oracle: `ProblemWorkbench` + `SubmissionsFeed`, steps
//! 16/23 final form): a `kind: problem` lesson renders the two-pane workbench instead of
//! the prose column — breadcrumbs, then the left pane (title · lede ·
//! Description | Editorial | Coach | Submissions tabs · the difficulty badge) beside the
//! right pane (the workbench with its tests panel and the anonymous sign-in note). The
//! FIRST workbench fence is EXTRACTED out of the description into the right pane; panes
//! mount once and toggle `.hidden`, so editor state survives tab switches.

use std::any::Any;

use crate::api;
use crate::catalog::logic;
use crate::catalog::logic::pane::{self, TABS, Tab};
use crate::catalog::state;
use crate::execution::logic::Variant;
use crate::execution::view::RunnableBlock;
use crate::islands::markdown;
use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::catalog::LessonPayloadDto;
use synapse_shared::execution::TestSpec;
use synapse_shared::submission::SubmissionDto;

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

// ─────────────────────────────────────────────────────────────────────────────
// THE NAVIGATION BAR — a docked row, not three pills over the panes
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Prev,
    Next,
}

impl Step {
    const fn label(self) -> &'static str {
        match self {
            Self::Prev => "Previous",
            Self::Next => "Next",
        }
    }
}

/// Opens the reader's nav drawer — the SAME one the mobile FAB drives, so the book's reading
/// order stays a single component. Absent outside the reader shell (nothing else provides
/// `ChromeState`), which keeps the button from lying about what it can do.
fn contents_button() -> Option<impl IntoView> {
    let chrome = use_context::<super::chrome::ChromeState>()?;
    Some(view! {
        <button
            class="pwb__contents"
            aria-label="Contents — the book's lessons and problems"
            on:click=move |_| chrome.nav_open.set(true)
        >
            <svg class="pwb__contents-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <rect width="18" height="18" x="3" y="3" rx="2"></rect>
                <path d="M9 3v18 M14 9l3 3-3 3"></path>
            </svg>
            <span>"Contents"</span>
        </button>
    })
}

/// "PROBLEM 5 / 6" over a dot per problem in this chapter — each dot a link, so the row is a
/// map you can travel rather than a readout.
///
/// The dots and the arrows deliberately answer DIFFERENT questions. The dots say where you are
/// in this problem set; the arrows say where you go next in the BOOK, which is why they keep the
/// server's reading order and may cross into prose at a chapter edge. Scoping the arrows to
/// problems instead would dead-end the learner at every boundary.
fn problem_counter(problems: &[String], at: usize) -> impl IntoView + use<> {
    let dots: Vec<_> = problems
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let title = humanize(path.rsplit('/').next().unwrap_or(path));
            view! {
                <a
                    class="pwb__dot"
                    class:pwb__dot--current=i == at
                    href=format!("/synapse/{path}")
                    title=title
                    aria-label=format!("Problem {}", i + 1)
                ></a>
            }
        })
        .collect();
    view! {
        <span class="pwb__nav-count">{format!("Problem {} / {}", at + 1, problems.len())}</span>
        <div class="pwb__nav-dots">{dots}</div>
    }
}

/// The tab bar's glyphs (Lucide `file-text` · `book-open` · `graduation-cap` · `history`).
/// `stroke="currentColor"` is the point: each icon inherits its tab's `--tab-hue` for free,
/// so the colour lives entirely in CSS and this fn never learns about theming.
fn tab_icon(tab: Tab) -> AnyView {
    match tab {
        Tab::Description => view! {
            <svg class="problem-tab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z"></path>
                <path d="M14 2v5h5 M16 13H8 M16 17H8 M10 9H8"></path>
            </svg>
        }
        .into_any(),
        Tab::Editorial => view! {
            <svg class="problem-tab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M12 7v14 M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4"></path>
                <path d="M21 18a1 1 0 0 0 1-1V4a1 1 0 0 0-1-1h-5a4 4 0 0 0-4 4"></path>
            </svg>
        }
        .into_any(),
        Tab::Coach => view! {
            <svg class="problem-tab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M22 10v6 M2 10l10-5 10 5-10 5z"></path>
                <path d="M6 12v5c3 3 9 3 12 0v-5"></path>
            </svg>
        }
        .into_any(),
        Tab::Submissions => view! {
            <svg class="problem-tab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M3 3v5h5"></path>
                <path d="M3.05 13A9 9 0 1 0 6 5.3L3 8 M12 7v5l4 2"></path>
            </svg>
        }
        .into_any(),
    }
}

/// One step through the book's reading order. `prev`/`next` already ride on the payload — the
/// prose reader spends them on its pager cards, and a problem page has no room for those.
fn step_link(target: Option<&str>, step: Step) -> Option<impl IntoView + use<>> {
    let path = target?.to_owned();
    let title = humanize(path.rsplit('/').next().unwrap_or(&path));
    Some(view! {
        <a
            class="pwb__step"
            class:pwb__step--prev=step == Step::Prev
            class:pwb__step--next=step == Step::Next
            href=format!("/synapse/{path}")
        >
            {(step == Step::Prev).then(|| view! { <span class="pwb__step-chev">"‹"</span> })}
            <span class="pwb__step-text">
                <span class="pwb__step-label">{step.label()}</span>
                <span class="pwb__step-title">{title}</span>
            </span>
            {(step == Step::Next).then(|| view! { <span class="pwb__step-chev">"›"</span> })}
        </a>
    })
}

/// The bar itself: Contents left, the counter centred, the two steps right. Mounts as the LAST
/// flex child of `.pwb`, so it takes its height from the panes instead of floating over them.
fn nav_bar(
    prev: Option<&str>,
    next: Option<&str>,
    counter: Option<(Vec<String>, usize)>,
) -> impl IntoView + use<> {
    view! {
        <nav class="pwb__nav" aria-label="Problem navigation">
            <div class="pwb__nav-left">{contents_button()}</div>
            <div class="pwb__nav-mid">
                {counter.map(|(problems, at)| problem_counter(&problems, at))}
            </div>
            <div class="pwb__nav-right">
                {step_link(prev, Step::Prev)}
                {step_link(next, Step::Next)}
            </div>
        </nav>
    }
}

#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
#[component]
pub fn ProblemWorkbench(payload: LessonPayloadDto, segments: Vec<String>) -> impl IntoView {
    // Captured IN-TREE — hydrated islands can't reach App context; see `IslandStores`.
    let stores = crate::hydration::IslandStores::capture();

    let restored = state::pane_prefs();
    let left_pct = RwSignal::new(restored.left_pct);
    let panes_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    // The extracted FIRST workbench (variants + suite) — the right pane renders it.
    let wb_spec: RwSignal<Option<(Vec<Variant>, Option<TestSpec>)>> = RwSignal::new(None);
    // Copy-to-editor from the editorial's solutions: (tick, language, code).
    let load_code = RwSignal::new((0_u32, String::new(), String::new()));
    // Use-this-test-case from a rejected submission: the same tick trick, so pulling the SAME
    // failing input twice fires twice. The right pane's tests panel is the only consumer.
    let use_case = RwSignal::new(crate::execution::view::no_case_yet());
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
        left_pct.set(pct.clamp(pane::MIN_LEFT_PCT, pane::MAX_LEFT_PCT));
    });
    // Persist on RELEASE, not on move — this listener fires for every window pointerup, so it
    // is gated on `dragging` rather than writing storage at pointer rate.
    let released = window_event_listener(leptos::ev::pointerup, move |_| {
        if dragging.get_value() {
            state::set_pane_left_pct(left_pct.get_untracked());
        }
        dragging.set_value(false);
    });
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
    // The remembered tab, but never onto a tab this problem can't honour — a restored Editorial
    // on a problem without one would strand the reader on "No editorial yet".
    let landing = if restored.tab == Tab::Editorial && editorial_md.trim().is_empty() {
        Tab::Description
    } else {
        restored.tab
    };
    let tab = RwSignal::new(landing);
    // Seeded, not defaulted: the lazily-mounted Submissions pane is gated on this, so restoring
    // that tab without seeding renders it empty.
    let subs_seen = RwSignal::new(landing == Tab::Submissions);

    let title = payload.frontmatter.title.clone();
    let lede = payload.frontmatter.summary.clone();
    let difficulty = payload.frontmatter.difficulty.clone();
    let book_name = humanize(&payload.book.slug);
    let lesson_title = payload.frontmatter.title.clone();
    let prev_path = payload.prev.clone();
    let next_path = payload.next.clone();
    let segments = StoredValue::new(segments);

    // "Problem N / M" for this chapter. Read REACTIVELY: the index may still be loading when the
    // page mounts, and the bar simply has no centre until it lands. `ProblemWorkbench` renders
    // in-tree, so app context is reachable here — the same reason `contents_button` works.
    let counter = move || {
        let path = segments.read_value().join("/");
        match crate::catalog::state::CatalogStore::from_context().index().get() {
            crate::api::AsyncResult::Loaded(index) => logic::book_of(&index, &segments.read_value())
                .and_then(|book| logic::chapter_problems(book, &path)),
            _ => None,
        }
    };

    let tab_buttons: Vec<_> = TABS
        .into_iter()
        .map(|t| {
            view! {
                <button
                    class=format!("problem-tab problem-tab--{}", t.slug())
                    class:problem-tab--active=move || tab.get() == t
                    on:click=move |_| {
                        tab.set(t);
                        state::set_pane_tab(t);
                        if t == Tab::Submissions {
                            subs_seen.set(true);
                        }
                    }
                >
                    {tab_icon(t)}
                    {t.label()}
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
                    // Each pane owns its own scroll region (step 57): the editorial's
                    // stepper strip must sit BETWEEN the tab bar and the scrolling
                    // content, which one shared scroll wrapper cannot give it. A side
                    // benefit: per-tab scroll positions stop bleeding into each other.
                    <div class="pwb__pane-host">
                        <div class="pwb__pane" class:hidden=move || tab.get() != Tab::Description>
                            <div class="pwb__pane-scroll synapse-prose">
                                {description_pane(desc_md, wb_spec, segments.read_value().clone(), code_ctx, stores)}
                            </div>
                        </div>
                        <div class="pwb__pane" class:hidden=move || tab.get() != Tab::Editorial>
                            {super::editorial::editorial_pane(&editorial_md, load_code, stores)}
                        </div>
                        <div class="pwb__pane" class:hidden=move || tab.get() != Tab::Coach>
                            <div class="pwb__pane-scroll synapse-prose">
                                <crate::tutoring::CoachPane
                                    problem=Some(segments.read_value().join("/"))
                                    code_ctx=code_ctx
                                />
                            </div>
                        </div>
                        <div class="pwb__pane" class:hidden=move || tab.get() != Tab::Submissions>
                            <div class="pwb__pane-scroll synapse-prose">
                                {move || subs_seen.get().then(|| view! {
                                    <SubmissionsFeed
                                        path=segments.read_value().clone()
                                        refetch=submitted
                                        load_code=load_code
                                        use_case=use_case
                                        wb_spec=wb_spec
                                    />
                                })}
                            </div>
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
                                {(!stores.auth.authed()).then(|| view! {
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
                                    code_sink=code_ctx
                                    stores=stores
                                    load_code=load_code
                                    submitted=submitted
                                    use_case=use_case
                                    fill=true
                                />
                            </div>
                        }
                        .into_any(),
                        None => view! { <div class="pwb__nowb">"Loading the workbench…"</div> }.into_any(),
                    }}
                </div>
            </div>
            {move || nav_bar(prev_path.as_deref(), next_path.as_deref(), counter())}
        </div>
    }
}

/// The description: rendered markdown with the FIRST workbench placeholder EXTRACTED into
/// the right pane; everything else hydrates in place.
fn description_pane(
    md: String,
    wb_spec: RwSignal<Option<(Vec<Variant>, Option<TestSpec>)>>,
    segments: Vec<String>,
    code_ctx: RwSignal<(String, String)>,
    stores: crate::hydration::IslandStores,
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
            let mut handles = crate::execution::view::hydrate_workbenches(&node, &segments, code_ctx, stores);
            handles.extend(crate::catalog::view::diagrams::hydrate_diagrams(&node));
            handles.extend(crate::execution::view::hydrate_fence_groups(
                &node,
                stores.codebench,
            ));
            handles.extend(crate::viz::blocks::mount_widgets(&node));
            mounts.set_value(handles);
        });
        true
    });
    on_cleanup(move || mounts.set_value(Vec::new()));
    view! { <div class="pwb-description" node_ref=node_ref></div> }
}

// ─────────────────────────────────────────────────────────────────────────────
// SUBMISSIONS FEED — the caller's own list, auth-gated, refetched on submit
// ─────────────────────────────────────────────────────────────────────────────

#[component]
fn SubmissionsFeed(
    path: Vec<String>,
    refetch: RwSignal<u32>,
    load_code: RwSignal<(u32, String, String)>,
    /// Where a rejected row's "Use this test case" pushes the failing input — the right pane's
    /// tests panel consumes it.
    use_case: RwSignal<crate::execution::view::CaseRequest>,
    /// The extracted workbench's suite, read only for the reproducibility guard.
    wb_spec: RwSignal<Option<(Vec<Variant>, Option<TestSpec>)>>,
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
                        let visible = wb_spec.get().and_then(|(_, spec)| spec);
                        let current: Vec<_> = list.iter().take(1).cloned().collect();
                        let code = selected
                            .get()
                            .and_then(|id| list.iter().find(|d| d.id == id).cloned());
                        view! {
                            <h3 class="psub__section">"Current submission"</h3>
                            {subs_table(&current, selected, use_case, visible.as_ref())}
                            <h3 class="psub__section">"All submissions"</h3>
                            {subs_table(&list, selected, use_case, visible.as_ref())}
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

fn subs_table(
    list: &[SubmissionDto],
    selected: RwSignal<Option<String>>,
    use_case: RwSignal<crate::execution::view::CaseRequest>,
    visible: Option<&TestSpec>,
) -> impl IntoView + use<> {
    let rows: Vec<_> = list
        .iter()
        .enumerate()
        .map(|(i, dto)| {
            // Only a rejection has a revealed failing case to pull forward.
            let pull = dto
                .first_failure
                .as_ref()
                .map(|f| crate::execution::view::use_case_button(f, visible, use_case));
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
                        {pull}
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
