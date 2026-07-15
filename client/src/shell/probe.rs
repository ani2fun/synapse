//! The step-02 probe page — one component per mechanic the spike must prove. It is throwaway
//! chrome (the reader replaces it in step 06) but NOT throwaway mechanics: everything here is
//! the exact pattern the features will use.

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::api::HealthStatus;

use crate::islands;

const SAMPLE_MARKDOWN: &str = "# Interop lives\n\nThis HTML crossed the **wasm ↔ TS island** \
boundary: Rust called `@markdown/loader`, the loader dynamic-imported the renderer, and the \
renderer's chunk stayed off the critical path.";

/// GET `/api/health` through the Vite proxy, decoded into the SHARED `HealthStatus` — the same
/// struct the server serialized, proving the shared kernel compiles and agrees on wasm32.
async fn fetch_health() -> Option<HealthStatus> {
    gloo_net::http::Request::get("/api/health")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()
}

#[component]
pub fn ProbePage() -> impl IntoView {
    // ── Proof 1 · fine-grained reactivity (Laminar Var → RwSignal) ──
    let count = RwSignal::new(0_i32);

    // ── Proof 2 · the TS-island round trip ──
    let rendered = RwSignal::new(String::from("<p>rendering…</p>"));
    spawn_local(async move {
        match islands::markdown::render(SAMPLE_MARKDOWN).await {
            Ok(html) => rendered.set(html),
            Err(err) => rendered.set(format!("<p>island failed: {err:?}</p>")),
        }
    });

    // ── Proof 3 · shared DTOs over the wire ──
    let health = RwSignal::new(String::from("asking the server…"));
    spawn_local(async move {
        match fetch_health().await {
            Some(h) => health.set(format!("server says: {}", h.status)),
            None => health.set(String::from("server unreachable (is dev-tools/dev running?)")),
        }
    });

    view! {
        <section class="probe">
            <h2>"Signals"</h2>
            <button on:click=move |_| count.update(|n| *n += 1)>
                "clicked " {count} " times"
            </button>

            <h2>"TS island"</h2>
            <div class="probe-markdown" inner_html=move || rendered.get()></div>

            <h2>"Shared DTO"</h2>
            <p data-probe="health">{health}</p>
        </section>
    }
}
