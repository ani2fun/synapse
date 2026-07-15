//! The app shell — router + chrome (oracle: the `shell` feature). Step 02 carries one route and
//! a probe page that proves the three load-bearing mechanics end to end: Leptos signals, the
//! TS-island round trip, and the shared wire DTOs decoding a real server response.

mod probe;

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

/// The root component `lib.rs` mounts.
#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <header class="shell-header">
                <span class="shell-brand">"synapse-rs"</span>
                <span class="shell-tag">"the Rust rebuild — walking skeleton"</span>
            </header>
            <main class="shell-main">
                <Routes fallback=|| view! { <p>"Not found."</p> }>
                    <Route path=path!("/") view=probe::ProbePage />
                </Routes>
            </main>
        </Router>
    }
}
