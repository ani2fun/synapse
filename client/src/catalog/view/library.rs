//! The library page (oracle: the browse/landing slice of steps 07/12) — the catalog tree as
//! category sections and book cards; a card links to the book's first lesson.

use leptos::prelude::*;
use synapse_shared::catalog::{CatalogEntryDto, SynapseIndexDto};

use crate::api::AsyncResult;
use crate::catalog::logic;
use crate::catalog::state;

#[component]
pub fn LibraryPage() -> impl IntoView {
    let index = state::CatalogStore::from_context().index();
    view! {
        <section class="library">
            {move || match index.get() {
                AsyncResult::Loading => view! { <p class="muted">"Loading the library…"</p> }.into_any(),
                AsyncResult::Failed(message) => {
                    view! { <p class="error">"The library failed to load: " {message}</p> }.into_any()
                }
                AsyncResult::Loaded(idx) => library_tree(&idx).into_any(),
            }}
        </section>
    }
}

fn library_tree(index: &SynapseIndexDto) -> impl IntoView + use<> {
    entries_view(&index.entries)
}

fn entries_view(entries: &[CatalogEntryDto]) -> impl IntoView + use<> {
    let rendered: Vec<_> = entries
        .iter()
        .map(|entry| match entry {
            CatalogEntryDto::Category(category) => {
                let inner = entries_view(&category.entries);
                view! {
                    <section class="library-category">
                        <h2>{category.title.clone()}</h2>
                        {category.description.clone().map(|d| view! { <p class="muted">{d}</p> })}
                        {inner}
                    </section>
                }
                .into_any()
            }
            CatalogEntryDto::Book(book) => {
                let href = logic::first_lesson_path(book)
                    .map_or_else(|| "/".to_owned(), |path| format!("/synapse/{path}"));
                view! {
                    <a class="book-card" href=href>
                        <h3>{book.title.clone()}</h3>
                        <p class="muted">{book.description.clone()}</p>
                        {book
                            .estimated_reading_minutes
                            .map(|minutes| view! { <span class="badge">{minutes} " min"</span> })}
                    </a>
                }
                .into_any()
            }
        })
        .collect();
    view! { <div class="library-level">{rendered}</div> }
}
