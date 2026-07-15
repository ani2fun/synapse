//! The app shell — chrome + the router feeding the pure app-map (oracle: `AppRouter.scala`).
//! The step-02 probe page is gone; its mechanics (signals, the island bridge, shared DTOs)
//! live on inside the features.

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::catalog::view::{LessonPage, LibraryPage};

/// The root component `lib.rs` mounts.
#[component]
pub fn App() -> impl IntoView {
    // App-level stores live under the root owner — they outlive every page (state layer rule).
    crate::catalog::state::CatalogStore::provide();
    view! {
        <Router>
            <header class="shell-header">
                <a class="shell-brand" href="/">"synapse-rs"</a>
                <span class="shell-tag">"the Rust rebuild"</span>
            </header>
            <main class="shell-main">
                <Routes fallback=|| view! { <p class="muted">"Not found."</p> }>
                    <Route path=path!("/") view=LibraryPage />
                    <Route path=path!("/synapse/*path") view=LessonPage />
                </Routes>
            </main>
        </Router>
    }
}
