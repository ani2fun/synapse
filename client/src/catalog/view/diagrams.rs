//! Authored-diagram hydration (oracle: `DiagramBlocks` + `MermaidView`/`D2View` +
//! `DiagramZoom`): `.mermaid-block` AND `.d2-block`/`.d2-slideshow` placeholders carry their
//! RAW SOURCE and render through the lazy `@diagram` island on the CLIENT — d2 no longer
//! renders at parse time (prose-first refactor 2026-07-17), so the multi-MB d2 WASM loads
//! only when a diagram nears the viewport and prose paints immediately. Every rendered figure
//! gets the Enlarge affordance → the near-fullscreen zoom overlay (wheel zoom · drag pan ·
//! − ⟲ + controls). House rule: the diagram chrome — Enlarge on the card AND Close in the
//! overlay — sits top-LEFT (LikeC4 owns top-right).

use std::any::Any;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use crate::execution::view::lazy::{self, NearWatch};
use crate::islands::diagram;

// ─────────────────────────────────────────────────────────────────────────────
// DISCOVERY — every placeholder carries its raw source; the card renders it lazily.
// ─────────────────────────────────────────────────────────────────────────────

pub fn hydrate_diagrams(root: &web_sys::HtmlElement) -> Vec<Box<dyn Any>> {
    let mut handles: Vec<Box<dyn Any>> = Vec::new();
    for (selector, attr) in [
        ("div.mermaid-block", "data-source"),
        ("div.d2-block", "data-source"),
        ("div.d2-slideshow", "data-slides"),
    ] {
        let Ok(nodes) = root.query_selector_all(selector) else {
            continue;
        };
        for index in 0..nodes.length() {
            let Some(node) = nodes.get(index) else { continue };
            let Ok(element) = node.dyn_into::<web_sys::HtmlElement>() else {
                continue;
            };
            let Some(payload) = element
                .get_attribute(attr)
                .and_then(|encoded| js_sys::decode_uri_component(&encoded).ok())
                .map(String::from)
            else {
                continue;
            };
            let handle = match selector {
                "div.mermaid-block" => leptos::mount::mount_to(element, move || {
                    view! { <MermaidCard source=payload /> }.into_any()
                }),
                "div.d2-block" => {
                    leptos::mount::mount_to(element, move || view! { <D2Card source=payload /> }.into_any())
                }
                _ => {
                    let Ok(slides) = serde_json::from_str::<Vec<String>>(&payload) else {
                        continue;
                    };
                    if slides.is_empty() {
                        continue;
                    }
                    leptos::mount::mount_to(element, move || {
                        view! { <D2Slideshow slides=slides /> }.into_any()
                    })
                }
            };
            handles.push(Box::new(handle));
        }
    }
    handles
}

// ─────────────────────────────────────────────────────────────────────────────
// CARDS
// Every diagram sits on a FIXED-LIGHT card (the authored palettes assume light),
// with the Enlarge pill revealed on hover once the figure has rendered.
// ─────────────────────────────────────────────────────────────────────────────

/// A ` ```mermaid ` fence: source → SVG via the lazy island; a malformed diagram becomes the
/// loud error card with the raw source to fix — never a blank figure.
#[component]
fn MermaidCard(source: String) -> impl IntoView {
    let figure_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let svg_html: RwSignal<Option<String>> = RwSignal::new(None);
    let failed: RwSignal<Option<String>> = RwSignal::new(None);
    let render_source = source.clone();
    Effect::new(move |ran: Option<bool>| {
        if ran == Some(true) {
            return true;
        }
        let Some(node) = figure_ref.get() else {
            return false;
        };
        let src = render_source.clone();
        spawn_local(async move {
            match diagram::render_mermaid(&node, &src).await {
                Ok(()) => svg_html.set(Some(node.inner_html())),
                Err(error) => failed.set(Some(format!("{error:?}"))),
            }
        });
        true
    });
    view! {
        {move || {
            failed.get().map(|message| {
                let raw = source.clone();
                view! {
                    <div class="diagram-error">
                        {format!("Mermaid diagram failed — {message}.")}
                        <details>
                            <summary>"diagram source"</summary>
                            <pre>{raw}</pre>
                        </details>
                    </div>
                }
            })
        }}
        <div class="diagram not-prose" class:hidden=move || failed.get().is_some()>
            <ZoomAffordance svg_html=svg_html />
            <div class="diagram__figure" node_ref=figure_ref></div>
        </div>
    }
}

/// A single ` ```d2 ` fence: raw source → SVG via the lazy `@diagram` island, rendered on the
/// CLIENT only when the card nears the viewport (so the multi-MB d2 WASM never blocks prose).
/// A malformed diagram becomes the loud error card with the raw source — never a blank figure.
#[component]
fn D2Card(source: String) -> impl IntoView {
    let root_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let figure_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let svg_html: RwSignal<Option<String>> = RwSignal::new(None);
    let failed: RwSignal<Option<String>> = RwSignal::new(None);
    let near = RwSignal::new(false);
    let started = RwSignal::new(false);
    let watch: StoredValue<Option<NearWatch>, LocalStorage> = StoredValue::new_local(None);

    // Arm the near-viewport observer once the root exists.
    Effect::new(move |_| {
        let Some(node) = root_ref.get() else { return };
        if watch.read_value().is_none() {
            watch.set_value(lazy::watch_near(&node, near));
        }
    });
    // Render once, the moment the card first nears the viewport (latched by `started`).
    let render_source = source.clone();
    Effect::new(move |_| {
        if started.get() || !near.get() {
            return;
        }
        let Some(node) = figure_ref.get() else { return };
        started.set(true);
        let src = render_source.clone();
        spawn_local(async move {
            match diagram::render_d2(&src).await {
                Ok(svg) => {
                    node.set_inner_html(&svg);
                    svg_html.set(Some(svg));
                }
                Err(error) => failed.set(Some(format!("{error:?}"))),
            }
        });
    });
    on_cleanup(move || watch.set_value(None));

    view! {
        {move || {
            failed.get().map(|message| {
                let raw = source.clone();
                view! {
                    <div class="diagram-error">
                        {format!("D2 diagram failed — {message}.")}
                        <details>
                            <summary>"diagram source"</summary>
                            <pre>{raw}</pre>
                        </details>
                    </div>
                }
            })
        }}
        <div class="diagram not-prose" class:hidden=move || failed.get().is_some() node_ref=root_ref>
            <ZoomAffordance svg_html=svg_html />
            <div class="diagram__figure" node_ref=figure_ref></div>
        </div>
    }
}

/// A run of adjacent d2 fences: one figure + the step transport (‹ i / n ›). Each slide's SVG
/// renders from source via the lazy island the FIRST time its step is shown (and the card is
/// near the viewport), then is memoized per index so stepping back is instant.
#[component]
fn D2Slideshow(slides: Vec<String>) -> impl IntoView {
    let count = slides.len();
    let idx = RwSignal::new(0_usize);
    let sources = StoredValue::new(slides);
    // Rendered SVGs by slide index — render each once, reuse thereafter.
    let rendered: StoredValue<Vec<Option<String>>, LocalStorage> = StoredValue::new_local(vec![None; count]);
    let figure_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let root_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let svg_html: RwSignal<Option<String>> = RwSignal::new(None);
    let near = RwSignal::new(false);
    let bump = RwSignal::new(0_u32); // ticks when a lazily-rendered slide lands
    let watch: StoredValue<Option<NearWatch>, LocalStorage> = StoredValue::new_local(None);

    Effect::new(move |_| {
        let Some(node) = root_ref.get() else { return };
        if watch.read_value().is_none() {
            watch.set_value(lazy::watch_near(&node, near));
        }
    });
    // Show the active slide: paint its cached SVG if we have it, else render it (once near).
    Effect::new(move |_| {
        bump.track();
        if !near.get() {
            return;
        }
        let i = idx.get().min(count - 1);
        let Some(node) = figure_ref.get() else { return };
        if let Some(svg) = rendered.read_value()[i].clone() {
            node.set_inner_html(&svg);
            svg_html.set(Some(svg));
            return;
        }
        let src = sources.read_value()[i].clone();
        spawn_local(async move {
            if let Ok(svg) = diagram::render_d2(&src).await {
                rendered.update_value(|r| r[i] = Some(svg));
                bump.update(|b| *b += 1); // re-run this effect to paint the freshly-cached slide
            }
        });
    });
    on_cleanup(move || watch.set_value(None));
    view! {
        <div class="diagram diagram--slides not-prose" node_ref=root_ref>
            <ZoomAffordance svg_html=svg_html />
            <div class="diagram__figure" node_ref=figure_ref></div>
            <div class="transport">
                <button
                    class="transport__btn"
                    title="Previous"
                    on:click=move |_| idx.update(|i| *i = i.saturating_sub(1))
                >
                    "‹"
                </button>
                <span class="transport__label">
                    {move || format!("{} / {count}", idx.get() + 1)}
                </span>
                <button
                    class="transport__btn"
                    title="Next"
                    on:click=move |_| idx.update(|i| *i = (*i + 1).min(count - 1))
                >
                    "›"
                </button>
            </div>
        </div>
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE ZOOM OVERLAY
// Near-fullscreen light card over a scrim; wheel zoom, drag pan, − ⟲ + controls.
// Enlarge (card) and Close (overlay) both live top-LEFT — the house corner.
// ─────────────────────────────────────────────────────────────────────────────

/// Lucide `maximize` — the Enlarge pill's glyph (oracle: Icons.maximize).
fn icon_maximize() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M15 3h6v6" /><path d="M9 21H3v-6" />
            <path d="m21 3-7 7" /><path d="m3 21 7-7" />
        </svg>
    }
}

/// Lucide `x` — the overlay Close pill's glyph (oracle: Icons.close).
fn icon_close() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M18 6 6 18" /><path d="m6 6 12 12" />
        </svg>
    }
}

#[component]
fn ZoomAffordance(svg_html: RwSignal<Option<String>>) -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        {move || svg_html.get().map(|_| view! {
            <button
                class="diagram__zoom modal-btn"
                aria-label="Enlarge diagram"
                on:click=move |_| open.set(true)
            >
                {icon_maximize()}
                <span>"Enlarge"</span>
            </button>
        })}
        {move || (open.get() && svg_html.get_untracked().is_some()).then(|| {
            let svg = svg_html.get_untracked().unwrap_or_default();
            view! { <ZoomOverlay svg=svg open=open /> }
        })}
    }
}

#[component]
fn ZoomOverlay(svg: String, open: RwSignal<bool>) -> impl IntoView {
    let scale = RwSignal::new(1.0_f64);
    let pan = RwSignal::new((0.0_f64, 0.0_f64));
    let grip: StoredValue<Option<(f64, f64)>> = StoredValue::new(None);
    let figure_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    Effect::new(move |ran: Option<bool>| {
        if ran == Some(true) {
            return true;
        }
        let Some(node) = figure_ref.get() else {
            return false;
        };
        node.set_inner_html(&svg);
        true
    });

    let esc = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" && open.get_untracked() {
            open.set(false);
        }
    });
    on_cleanup(move || esc.remove());
    let moved = window_event_listener(leptos::ev::pointermove, move |event| {
        let Some((last_x, last_y)) = grip.get_value() else {
            return;
        };
        let (x, y) = (f64::from(event.client_x()), f64::from(event.client_y()));
        pan.update(|(tx, ty)| {
            *tx += x - last_x;
            *ty += y - last_y;
        });
        grip.set_value(Some((x, y)));
    });
    let released = window_event_listener(leptos::ev::pointerup, move |_| grip.set_value(None));
    on_cleanup(move || {
        moved.remove();
        released.remove();
    });

    let zoom_by = move |factor: f64| {
        scale.update(|s| *s = (*s * factor).clamp(0.25, 4.0));
    };
    let transform = move || {
        let (tx, ty) = pan.get();
        format!(
            "transform: translate({tx:.1}px, {ty:.1}px) scale({:.3})",
            scale.get()
        )
    };
    view! {
        <div class="diagram-zoom-scrim" on:click=move |_| open.set(false)>
            <div class="diagram-zoom diagram-zoom--paper" on:click=|event| event.stop_propagation()>
                <button class="diagram-zoom__close modal-btn" aria-label="Close" on:click=move |_| open.set(false)>
                    {icon_close()}
                    <span>"Close"</span>
                </button>
                <div class="diagram-zoom__zoomable">
                    <div
                        class="diagram-zoom__viewport"
                        on:pointerdown=move |event| {
                            event.prevent_default();
                            grip.set_value(Some((
                                f64::from(event.client_x()),
                                f64::from(event.client_y()),
                            )));
                        }
                        on:wheel=move |event| {
                            event.prevent_default();
                            zoom_by(if event.delta_y() < 0.0 { 1.12 } else { 1.0 / 1.12 });
                        }
                    >
                        <div class="diagram-zoom__figure" style=transform node_ref=figure_ref></div>
                    </div>
                </div>
                <div class="diagram-zoom__controls">
                    <button class="diagram-zoom__ctl" aria-label="Zoom out" on:click=move |_| zoom_by(1.0 / 1.25)>"−"</button>
                    <span class="diagram-zoom__level">
                        {move || format!("{:.0}%", scale.get() * 100.0)}
                    </span>
                    <button class="diagram-zoom__ctl" aria-label="Zoom in" on:click=move |_| zoom_by(1.25)>"+"</button>
                    <button
                        class="diagram-zoom__ctl"
                        aria-label="Reset zoom"
                        on:click=move |_| {
                            scale.set(1.0);
                            pan.set((0.0, 0.0));
                        }
                    >
                        "⟲"
                    </button>
                </div>
            </div>
        </div>
    }
}
