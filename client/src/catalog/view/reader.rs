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
                    let lesson = state::load_lesson(path.get());
                    view! { <LessonBody lesson=lesson /> }
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
                    let items: Vec<_> = logic::reading_order(&book)
                        .into_iter()
                        .map(|(full, lesson)| {
                            let href = format!("/synapse/{full}");
                            // Fine-grained: each item tracks the path itself.
                            let is_current = Memo::new(move |_| path.get().join("/") == full);
                            view! {
                                <li class:current=move || is_current.get()>
                                    <a href=href>{lesson.title}</a>
                                </li>
                            }
                        })
                        .collect();
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

#[component]
fn LessonBody(lesson: RwSignal<AsyncResult<LessonPayloadDto>>) -> impl IntoView {
    view! {
        {move || match lesson.get() {
            AsyncResult::Loading => view! { <p class="muted">"Loading…"</p> }.into_any(),
            AsyncResult::Failed(message) => {
                view! { <p class="error">"Lesson failed to load: " {message}</p> }.into_any()
            }
            AsyncResult::Loaded(payload) => loaded_lesson(&payload).into_any(),
        }}
    }
}

fn loaded_lesson(payload: &LessonPayloadDto) -> impl IntoView + use<> {
    // The body crosses the island bridge asynchronously; the walking-skeleton renderer holds
    // this seam until step 08 ports the oracle's full pipeline behind the same loader.
    let html = RwSignal::new(String::from("<p>rendering…</p>"));
    let raw = payload.raw.clone();
    spawn_local(async move {
        match islands::markdown::render(&raw).await {
            Ok(rendered) => html.set(rendered),
            Err(error) => html.set(format!("<p>markdown island failed: {error:?}</p>")),
        }
    });

    let nav_link = |target: &Option<String>, label: &'static str, class: &'static str| {
        target.clone().map(|path| {
            view! { <a class=class href=format!("/synapse/{path}")>{label}</a> }
        })
    };
    view! {
        <header class="lesson-header">
            <p class="lesson-book muted">{payload.book.title.clone()}</p>
            <h1>{payload.frontmatter.title.clone()}</h1>
            {payload.frontmatter.summary.clone().map(|s| view! { <p class="lesson-summary">{s}</p> })}
        </header>
        <div class="lesson-body synapse-prose" inner_html=move || html.get()></div>
        <nav class="lesson-nav">
            {nav_link(&payload.prev, "← Previous", "nav-prev")}
            {nav_link(&payload.next, "Next →", "nav-next")}
        </nav>
    }
}
