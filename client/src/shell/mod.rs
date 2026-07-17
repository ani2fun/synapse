//! The app shell — chrome + the router feeding the pure app-map (oracle: `AppRouter.scala` +
//! `Header.scala`). The header is the oracle's polished chrome: brand chip + wordmark, the
//! centred ⌘K search affordance, then Blog · account chip · the theme toggle (sun/moon).

pub mod theme;

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::blog::view::{BlogListPage, BlogPostPage};
use crate::catalog::view::{LessonPage, LibraryPage};
use crate::search::view::{SearchButton, SearchPalette};
use theme::{Mode, ThemeStore};

/// The root component `lib.rs` mounts.
#[component]
pub fn App() -> impl IntoView {
    // App-level stores live under the root owner — they outlive every page (state layer rule).
    // PrefsStore also reflects the stored reading prefs onto <html> BEFORE the first paint.
    crate::catalog::state::CatalogStore::provide();
    crate::catalog::state::PrefsStore::provide();
    crate::identity::state::AuthStore::provide();
    crate::blog::state::BlogStore::provide();
    crate::search::state::SearchStore::provide();
    ThemeStore::provide();
    crate::viz::modal::VizModalStore::provide();
    view! {
        <Router>
            <RouteTrace />
            <header class="header">
                <nav class="header__nav">
                    <a class="header__brand" href="/">
                        <BrandChip />
                        <span class="header__wordmark">"synapse"</span>
                    </a>
                    <div class="header__mid">
                        <SearchButton />
                    </div>
                    <div class="header__actions">
                        <a class="header__link" href="/blog">"Blog"</a>
                        <crate::identity::view::AccountChip />
                        <ThemeToggle />
                    </div>
                </nav>
            </header>
            <main class="shell-main">
                <Routes fallback=|| view! { <p class="muted">"Not found."</p> }>
                    <Route path=path!("/") view=LibraryPage />
                    <Route path=path!("/synapse/*path") view=LessonPage />
                    <Route path=path!("/blog") view=BlogListPage />
                    <Route path=path!("/blog/:slug") view=BlogPostPage />
                    <Route path=path!("/account") view=crate::identity::view::AccountPage />
                    <Route path=path!("/admin") view=crate::identity::view::AdminPage />
                </Routes>
            </main>
            <SearchPalette />
            <crate::viz::modal::VisualiseModal />
        </Router>
    }
}

/// Every navigation logs its destination (oracle: `AppRouter`'s `route → …` INFO) — the
/// first line of the dev-flow trace for each page.
#[component]
fn RouteTrace() -> impl IntoView {
    let location = leptos_router::hooks::use_location();
    Effect::new(move |_| {
        crate::log::info(&format!("route → {}", location.pathname.get()));
    });
}

/// The brand mark (oracle: `Icons.brandChip`) — a 3-node graph on a primary chip; recolors
/// with the theme through the tokens.
#[component]
fn BrandChip() -> impl IntoView {
    view! {
        <svg class="header__chip" width="30" height="30" viewBox="0 0 30 30" aria-hidden="true">
            <rect width="30" height="30" rx="7" fill="hsl(var(--primary))"></rect>
            <g stroke="hsl(var(--primary-foreground))" stroke-width="1.6">
                <line x1="10" y1="19" x2="15" y2="10"></line>
                <line x1="15" y1="10" x2="20" y2="19"></line>
                <line x1="10" y1="19" x2="20" y2="19"></line>
            </g>
            <g fill="hsl(var(--primary-foreground))">
                <circle cx="15" cy="10" r="2.4"></circle>
                <circle cx="10" cy="19" r="2.4"></circle>
                <circle cx="20" cy="19" r="2.4"></circle>
            </g>
        </svg>
    }
}

/// Sun in the dark (→ switch to light), moon in the light (oracle: lucide sun/moon).
#[component]
fn ThemeToggle() -> impl IntoView {
    let theme = ThemeStore::from_context();
    view! {
        <button
            class="header__icon-btn"
            aria-label=move || match theme.mode.get() {
                Mode::Dark => "Switch to light mode",
                Mode::Light => "Switch to dark mode",
            }
            on:click=move |_| theme.toggle()
        >
            {move || match theme.mode.get() {
                Mode::Dark => sun().into_any(),
                Mode::Light => moon().into_any(),
            }}
        </button>
    }
}

fn sun() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="4"></circle>
            <path d="M12 2v2"></path><path d="M12 20v2"></path>
            <path d="m4.93 4.93 1.41 1.41"></path><path d="m17.66 17.66 1.41 1.41"></path>
            <path d="M2 12h2"></path><path d="M20 12h2"></path>
            <path d="m6.34 17.66-1.41 1.41"></path><path d="m19.07 4.93-1.41 1.41"></path>
        </svg>
    }
}

fn moon() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"></path>
        </svg>
    }
}
