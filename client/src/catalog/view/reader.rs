//! The reader (oracle: the lesson page of steps 07–08/12): sidebar (the owning book's reading
//! order from the SHARED cached index) + the lesson body across the markdown island + prev/next.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use synapse_shared::catalog::LessonPayloadDto;

use crate::api::AsyncResult;
use crate::catalog::state;
use crate::islands;
use crate::router::page::Page;

#[component]
pub fn LessonPage() -> impl IntoView {
    let params = use_params_map();
    // The catch-all param is reactive: navigating lesson → lesson re-renders this memo, and
    // everything below keys off it (fetch-per-navigation, oracle semantics).
    let path = Memo::new(move |_| Page::segments_of(&params.read().get("path").unwrap_or_default()));

    // The shared chrome state (oracle: LessonPage's Vars): one scroll handler feeds the
    // progress bar, sticky bar, minimap, TOC, and the Compact rail's ring.
    let chrome = super::chrome::ChromeState::new();
    provide_context(chrome);
    let scrolled = window_event_listener(leptos::ev::scroll, move |_| chrome.recompute());
    on_cleanup(move || scrolled.remove());
    Effect::new(move |_| {
        chrome.headings.track();
        chrome.recompute();
    });

    let mode = RwSignal::new(super::sidebar::SidebarMode::load());

    view! {
        <super::chrome::ReadingProgress />
        <super::chrome::StickyBar />
        <div
            class="reader-layout"
            class:reader-layout--problem=move || chrome.is_problem.get()
            data-sidebar=move || mode.get().token()
        >
            <aside class="reader-sidebar">
                <super::sidebar::ReaderSidebar path=path mode=mode progress=chrome.progress />
            </aside>
            <article class="reader-layout__main">
                {move || {
                    let segments = path.get();
                    let lesson = state::load_lesson(segments.clone());
                    view! { <LessonBody lesson=lesson segments=segments /> }
                }}
            </article>
        </div>
        // The prose reader's chrome, ALL of it gated on kind != problem. A problem page has
        // no window scroll (the panes scroll internally) and no sidebar column, so the
        // minimap, scroll-to-top and sidebar-restore controls are inert there. The TOC FAB
        // already gated itself. They own the bottom corners the problem page now puts its
        // own navigation in.
        {move || (!chrome.is_problem.get()).then(|| view! {
            <super::chrome::MiniMap />
            <super::chrome::ScrollTop />
            // The floating expand affordance for the Hidden sidebar.
            <button
                class=move || {
                    if mode.get() == super::sidebar::SidebarMode::Hidden {
                        "reader-expand"
                    } else {
                        "reader-expand reader-expand--hidden"
                    }
                }
                aria-label="Show the sidebar"
                on:click=move |_| {
                    mode.set(super::sidebar::SidebarMode::Expanded);
                    super::sidebar::SidebarMode::Expanded.persist();
                }
            >
                <svg class="reader-expand__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                     stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <rect width="18" height="18" x="3" y="3" rx="2"></rect>
                    <path d="M9 3v18 M14 9l3 3-3 3"></path>
                </svg>
            </button>
        })}
        <super::chrome::TocFab />
        // OUTSIDE the grid on purpose (oracle step-38 prod bug): the drawer's in-flow
        // wrapper would otherwise become a phantom third grid item at desktop width.
        <ReaderNavDrawer path=path />
    }
}

/// The mobile navigation drawer (oracle: `ReaderNavDrawer`, step 38): a bottom-LEFT FAB
/// (<1024px only — the desktop sidebar hides there) opens an off-canvas drawer reusing the
/// SAME `Sidebar`, always expanded. Three closes: scrim, Escape, any nav-link tap.
///
/// The open state lives in `ChromeState` because the drawer has a SECOND caller: problem
/// pages hide the sidebar column at every width, so their Contents button drives this same
/// singleton. `--pinned` keeps it reachable above the 1024px breakpoint for them.
#[component]
fn ReaderNavDrawer(path: Memo<Vec<String>>) -> impl IntoView {
    let chrome = expect_context::<super::chrome::ChromeState>();
    let open = chrome.nav_open;
    let chrome_progress = chrome.progress;
    let drawer_mode = RwSignal::new(super::sidebar::SidebarMode::Expanded);
    let esc = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" && open.get_untracked() {
            open.set(false);
        }
    });
    on_cleanup(move || esc.remove());
    view! {
        <div class="reader-nav" class:reader-nav--pinned=move || chrome.is_problem.get()>
            <button
                class="reader-nav-fab"
                aria-label="Contents"
                aria-expanded=move || open.get().to_string()
                on:click=move |_| open.set(true)
            >
                <svg class="reader-nav-fab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                     stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <rect width="18" height="18" x="3" y="3" rx="2"></rect>
                    <path d="M9 3v18 M14 9l3 3-3 3"></path>
                </svg>
            </button>
            {move || open.get().then(|| view! {
                <div>
                    <div class="reader-nav-scrim" on:click=move |_| open.set(false)></div>
                    <aside
                        class="reader-nav-drawer"
                        // Close-on-navigate: any click landing on (or inside) an <a> closes;
                        // the link itself still routes.
                        on:click=move |event| {
                            use wasm_bindgen::JsCast;
                            let closes = event
                                .target()
                                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                                .and_then(|el| el.closest("a").ok().flatten())
                                .is_some();
                            if closes {
                                open.set(false);
                            }
                        }
                    >
                        <div class="reader-nav-drawer__head">
                            <span class="reader-nav-drawer__title">"Contents"</span>
                            <button
                                class="reader-nav-drawer__close"
                                aria-label="Close"
                                on:click=move |_| open.set(false)
                            >
                                "✕"
                            </button>
                        </div>
                        <super::sidebar::ReaderSidebar
                            path=path
                            mode=drawer_mode
                            progress=chrome_progress
                            in_drawer=true
                        />
                    </aside>
                </div>
            })}
        </div>
    }
}

#[component]
fn LessonBody(lesson: RwSignal<AsyncResult<LessonPayloadDto>>, segments: Vec<String>) -> impl IntoView {
    let segments = StoredValue::new(segments);
    view! {
        {move || match lesson.get() {
            AsyncResult::Loading => view! { <p class="muted">"Loading…"</p> }.into_any(),
            AsyncResult::Failed(message) => {
                view! { <p class="error">"Lesson failed to load: " {message}</p> }.into_any()
            }
            AsyncResult::Loaded(payload) => loaded_lesson(&payload, &segments.read_value()).into_any(),
        }}
    }
}

// The reader's cohesive hydration wiring: capture stores in-tree, render the body, then
// hand each placeholder family to its hydrator. One unit on purpose (the oracle keeps
// `MarkdownView.mountBlocks` whole too).
#[allow(clippy::too_many_lines)]
fn loaded_lesson(payload: &LessonPayloadDto, segments: &[String]) -> AnyView {
    // Captured IN-TREE — hydrated blocks mount out-of-tree and cannot reach App's context;
    // the bundle carries them (see `crate::hydration::IslandStores`).
    let stores = crate::hydration::IslandStores::capture();
    // The C4 click-to-guide seam: bridges in the embeds set it; the docs panel reads it.
    let c4_selected: RwSignal<Option<String>> = RwSignal::new(None);
    // The Coach's editor snapshot — filled by the hydrated workbench on mount + every edit.
    let code_ctx = RwSignal::new((String::new(), String::new()));
    // The body crosses the island bridge asynchronously; once the HTML lands, the interactive
    // placeholders hydrate (runnable blocks today; solutions/quizzes/diagrams with their
    // steps). The boxed unmount handles keep the mounts alive; clearing them (navigation /
    // unmount) tears the blocks down.
    let html = RwSignal::new(String::from("<p>rendering…</p>"));
    let body_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let mounts: StoredValue<Vec<Box<dyn std::any::Any>>, LocalStorage> = StoredValue::new_local(Vec::new());
    let chrome_ctx = use_context::<crate::catalog::view::chrome::ChromeState>();
    let raw = payload.raw.clone();
    let owned_segments = segments.to_vec();
    let panel_segments = segments.to_vec();
    let problem_path_source = segments.join("/");
    crate::log::debug(&format!("rendering markdown ({} chars)", raw.len()));
    spawn_local(async move {
        match islands::markdown::render(&raw).await {
            Ok(rendered) => {
                // The oracle's pattern (`el.innerHTML = html` then mount blocks): write the DOM
                // directly and hydrate in the same breath — no render-effect race. The signal
                // stays for the placeholder/error states only.
                // `try_`, not `get_untracked()`. This async block routinely OUTLIVES the render
                // that spawned it — navigating away, or any re-render of the lesson, disposes
                // this owner while the markdown island is still working. `get_untracked()`
                // PANICS on a disposed reactive value rather than returning `None`, and a panic
                // in wasm is a dead app: the body stays on "rendering…" forever and every
                // island on the page stops responding.
                //
                // The `else { return }` was always the intent — "this render is stale, do
                // nothing". `try_get_untracked` is what makes that true instead of aspirational.
                let Some(Some(body)) = body_ref.try_get_untracked() else {
                    crate::log::debug("markdown landed after the lesson was disposed — dropping it");
                    return;
                };
                body.set_inner_html(&rendered);
                if let Some(chrome) = chrome_ctx {
                    chrome
                        .headings
                        .set(crate::catalog::view::chrome::harvest_headings(&body));
                }
                let mut handles =
                    crate::execution::view::hydrate_workbenches(&body, &owned_segments, code_ctx, stores);
                handles.extend(crate::catalog::view::diagrams::hydrate_diagrams(&body));
                handles.extend(crate::quiz::hydrate_quizzes(&body));
                handles.extend(crate::execution::view::hydrate_fence_groups(
                    &body,
                    stores.codebench,
                ));
                handles.extend(crate::catalog::view::c4::hydrate_c4_embeds(&body, c4_selected));
                handles.extend(crate::execution::view::hydrate_practices(
                    &body,
                    &owned_segments,
                    code_ctx,
                    stores,
                ));
                // The viz widgets (step 27): every planted `div.viz-widget` mounts a host.
                handles.extend(crate::viz::blocks::mount_widgets(&body));
                crate::log::debug(&format!(
                    "markdown rendered; mounted {} interactive block(s)",
                    handles.len()
                ));
                // Same hazard: a disposed `StoredValue` panics on write. Reaching here means
                // the body WAS still alive a moment ago, but disposal can land between the two.
                let _ = mounts.try_set_value(handles);
            }
            Err(error) => {
                crate::log::error(&format!("markdown render failed: {error:?}"));
                // ...and again on the error path, which never went through the guard above.
                let _ = html.try_set(format!("<p>markdown island failed: {error:?}</p>"));
            }
        }
    });
    // `try_`: cleanup runs DURING disposal, so the value it is clearing may already be gone.
    on_cleanup(move || {
        let _ = mounts.try_set_value(Vec::new());
    });

    // Problem pages render the TWO-PANE workbench instead of the prose column (oracle:
    // ProblemWorkbench; the parity list's item 2) — full width, no TOC/prefs chrome.
    let is_problem = payload.frontmatter.kind.as_deref() == Some("problem");
    let _ = problem_path_source;
    if let Some(chrome) = use_context::<crate::catalog::view::chrome::ChromeState>() {
        chrome.title.set(payload.frontmatter.title.clone());
        chrome.is_problem.set(is_problem);
    }
    // The document head follows SPA navigation (step 50). The server rendered the head for the
    // URL the visitor LANDED on; without this it would still describe that page three lessons
    // later — in the tab, the history entry and any bookmark taken along the way.
    {
        let book = match crate::catalog::state::CatalogStore::from_context().index().get() {
            crate::api::AsyncResult::Loaded(index) => {
                crate::catalog::logic::book_of(&index, segments).map(|b| b.title.clone())
            }
            _ => None,
        };
        crate::seo::set_title(&crate::seo::title_for_lesson(
            book.as_deref(),
            &payload.frontmatter.title,
        ));
        if let Some(summary) = payload.frontmatter.summary.as_deref() {
            crate::seo::set_description(summary);
        }
    }

    // Reading progress (step 51). `visit` records where to resume; the effect marks the lesson
    // finished once the chrome latches `reached_end`.
    {
        let progress = crate::catalog::state::ProgressStore::from_context();
        let path = segments.join("/");
        progress.visit(&path);
        if let Some(chrome) = use_context::<crate::catalog::view::chrome::ChromeState>() {
            // Per-lesson reset: the signal is latched, and the chrome outlives the lesson.
            chrome.reached_end.set(false);
            let path = path.clone();
            Effect::new(move |_| {
                if chrome.reached_end.get() {
                    progress.set_done(&path, true);
                }
            });
        }
    }
    if is_problem {
        return view! {
            <super::problem::ProblemWorkbench payload=payload.clone() segments=segments.to_vec() />
        }
        .into_any();
    }
    view! {
        <div class="lesson" class:lesson--problem=is_problem>
            <header class="lesson-header">
                <a class="reader-prose__back" href="/">"← Library"</a>
                <h1 class="reader-prose__title">{payload.frontmatter.title.clone()}</h1>
                {payload.frontmatter.summary.clone().map(|s| view! { <p class="reader-prose__lede">{s}</p> })}
            </header>
            <div class="lesson-body synapse-prose" node_ref=body_ref inner_html=move || html.get()></div>
            <nav class="reader-pager">
                {pager_card(payload.prev.as_deref(), "Previous", false)}
                {pager_card(payload.next.as_deref(), "Next", true)}
            </nav>
            <super::c4_docs::C4DocsPanel selected=c4_selected lesson=panel_segments />
            <super::ReaderPrefsFab />
        </div>
    }
    .into_any()
}

/// A pager card: label eyebrow + the humanized target title (oracle: `.reader-pager__card`).
fn pager_card(target: Option<&str>, label: &'static str, next: bool) -> Option<impl IntoView + use<>> {
    let path = target?.to_owned();
    let title = path
        .rsplit('/')
        .next()
        .unwrap_or(&path)
        .split('-')
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or_else(String::new, |f| {
                f.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ");
    let class = if next {
        "reader-pager__card reader-pager__card--next"
    } else {
        "reader-pager__card"
    };
    Some(view! {
        <a class=class href=format!("/synapse/{path}")>
            <span class="reader-pager__label">{label}</span>
            <span class="reader-pager__title">{title}</span>
        </a>
    })
}
