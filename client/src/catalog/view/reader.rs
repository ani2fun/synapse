//! The reader (oracle: the lesson page of steps 07–08/12): sidebar (the owning book's reading
//! order from the SHARED cached index) + the lesson body across the markdown island + prev/next.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use synapse_shared::catalog::LessonPayloadDto;

use crate::api::AsyncResult;
use crate::catalog::{logic, state};
use crate::islands;
use crate::router::page::Page;

#[component]
pub fn LessonPage() -> impl IntoView {
    let params = use_params_map();
    // The catch-all param is reactive: navigating lesson → lesson re-renders this memo, and
    // everything below keys off it (fetch-per-navigation, oracle semantics).
    let path = Memo::new(move |_| Page::segments_of(&params.read().get("path").unwrap_or_default()));

    view! {
        <div class="reader">
            <aside class="reader-sidebar">
                <Sidebar path=path />
            </aside>
            <article class="reader-main">
                {move || {
                    let segments = path.get();
                    let lesson = state::load_lesson(segments.clone());
                    view! { <LessonBody lesson=lesson segments=segments /> }
                }}
            </article>
        </div>
    }
}

#[component]
fn Sidebar(path: Memo<Vec<String>>) -> impl IntoView {
    let index = state::CatalogStore::from_context().index();
    // The owning book changes only across books — memoized so lesson→lesson navigation inside
    // one book re-renders nothing but the per-item `current` classes below.
    let book = Memo::new(move |_| match index.get() {
        AsyncResult::Loaded(idx) => logic::book_of(&idx, &path.get()).cloned(),
        AsyncResult::Loading | AsyncResult::Failed(_) => None,
    });
    view! {
        {move || {
            book.get()
                .map(|book| {
                    let prefix = logic::book_prefix(&book);
                    let items = sidebar_entries(&book.entries, &prefix, path);
                    view! {
                        <nav>
                            <h2 class="sidebar-book">{book.title.clone()}</h2>
                            <ul class="sidebar-lessons">{items}</ul>
                        </nav>
                    }
                })
        }}
    }
}

/// One book-interior level: lessons link; chapters are COLLAPSIBLE groups (post-33 `a95e3fb`),
/// open when they contain the current lesson so navigation always lands unfolded.
fn sidebar_entries(
    entries: &[synapse_shared::catalog::BookEntryDto],
    prefix: &[String],
    path: Memo<Vec<String>>,
) -> Vec<AnyView> {
    use synapse_shared::catalog::BookEntryDto;
    entries
        .iter()
        .map(|entry| match entry {
            BookEntryDto::Lesson(lesson) => {
                let mut segments = prefix.to_vec();
                segments.push(lesson.slug.clone());
                let full = segments.join("/");
                let href = format!("/synapse/{full}");
                // Fine-grained: each item tracks the path itself.
                let is_current = Memo::new(move |_| path.get().join("/") == full);
                view! {
                    <li class:current=move || is_current.get()>
                        <a href=href>{lesson.title.clone()}</a>
                    </li>
                }
                .into_any()
            }
            BookEntryDto::Chapter(chapter) => {
                let mut segments = prefix.to_vec();
                segments.push(chapter.slug.clone());
                let contains_current = path.get_untracked().join("/").starts_with(&segments.join("/"));
                let children = sidebar_entries(&chapter.entries, &segments, path);
                view! {
                    <li class="sidebar-chapter">
                        <details open=contains_current>
                            <summary class="sidebar-chapter__title">{chapter.title.clone()}</summary>
                            <ul class="sidebar-lessons">{children}</ul>
                        </details>
                    </li>
                }
                .into_any()
            }
        })
        .collect()
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

fn loaded_lesson(payload: &LessonPayloadDto, segments: &[String]) -> impl IntoView + use<> {
    // Captured IN-TREE: hydrated blocks mount out-of-tree and cannot reach App's context.
    let auth = crate::identity::state::AuthStore::from_context();
    // The Coach's editor snapshot — filled by the hydrated workbench on mount + every edit.
    let code_ctx = RwSignal::new((String::new(), String::new()));
    // The body crosses the island bridge asynchronously; once the HTML lands, the interactive
    // placeholders hydrate (runnable blocks today; solutions/quizzes/diagrams with their
    // steps). The boxed unmount handles keep the mounts alive; clearing them (navigation /
    // unmount) tears the blocks down.
    let html = RwSignal::new(String::from("<p>rendering…</p>"));
    let body_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let mounts: StoredValue<Vec<Box<dyn std::any::Any>>, LocalStorage> = StoredValue::new_local(Vec::new());
    let raw = payload.raw.clone();
    let owned_segments = segments.to_vec();
    let problem_path_source = segments.join("/");
    spawn_local(async move {
        match islands::markdown::render(&raw).await {
            Ok(rendered) => {
                // The oracle's pattern (`el.innerHTML = html` then mount blocks): write the DOM
                // directly and hydrate in the same breath — no render-effect race. The signal
                // stays for the placeholder/error states only.
                let Some(body) = body_ref.get_untracked() else {
                    return;
                };
                body.set_inner_html(&rendered);
                mounts.set_value(crate::execution::view::hydrate_workbenches(
                    &body,
                    &owned_segments,
                    auth,
                    code_ctx,
                ));
            }
            Err(error) => html.set(format!("<p>markdown island failed: {error:?}</p>")),
        }
    });
    on_cleanup(move || mounts.set_value(Vec::new()));

    let nav_link = |target: &Option<String>, label: &'static str, class: &'static str| {
        target.clone().map(|path| {
            view! { <a class=class href=format!("/synapse/{path}")>{label}</a> }
        })
    };
    // Problem pages go full width (post-33 `a95e3fb`) — the workbench needs the column.
    let is_problem = payload.frontmatter.kind.as_deref() == Some("problem");
    let problem_path = is_problem.then_some(problem_path_source);
    view! {
        <div class="lesson" class:lesson--problem=is_problem>
            <header class="lesson-header">
                <p class="lesson-book muted">{payload.book.title.clone()}</p>
                <h1>{payload.frontmatter.title.clone()}</h1>
                {payload.frontmatter.summary.clone().map(|s| view! { <p class="lesson-summary">{s}</p> })}
            </header>
            <div class="lesson-body synapse-prose" node_ref=body_ref inner_html=move || html.get()></div>
            {is_problem.then(|| view! {
                <crate::tutoring::CoachPane problem=problem_path.clone() code_ctx=code_ctx />
            })}
            <nav class="lesson-nav">
                {nav_link(&payload.prev, "← Previous", "nav-prev")}
                {nav_link(&payload.next, "Next →", "nav-next")}
            </nav>
            <super::ReaderPrefsFab />
        </div>
    }
}
