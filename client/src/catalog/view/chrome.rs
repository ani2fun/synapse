//! The reader's fixed chrome (oracle: `ReadingProgress` + `ReaderStickyBar` +
//! `ReaderMiniMap` + `ReaderToc` + the scroll-top FAB): everything that floats over the
//! prose. One scroll handler in `LessonPage` feeds the shared signals; these components
//! only render them.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::catalog::logic;

/// One harvested prose heading (h2/h3 with an id, courtesy of rehype-slug).
#[derive(Clone, PartialEq, Eq)]
pub struct Heading {
    pub id: String,
    pub text: String,
    pub level: u8,
}

/// The reader page's shared chrome state — created once per `LessonPage`.
#[derive(Clone, Copy)]
pub struct ChromeState {
    pub headings: RwSignal<Vec<Heading>>,
    pub title: RwSignal<String>,
    pub progress: RwSignal<f64>,
    pub active_id: RwSignal<Option<String>>,
    pub show_top: RwSignal<bool>,
    pub past_title: RwSignal<bool>,
    pub toc_open: RwSignal<bool>,
    pub is_problem: RwSignal<bool>,
    /// The nav drawer, opened from two places: the mobile FAB, and the problem page's
    /// Contents button (problem pages hide the sidebar column at every width, so the
    /// drawer is their ONLY route to the book's reading order).
    pub nav_open: RwSignal<bool>,
    /// Latched (step 51): true once the reader has reached the end of THIS lesson, and it does
    /// not go back to false on scroll-up — otherwise "finished" would flicker off every time
    /// someone scrolled back to re-read a paragraph. Reset per lesson by the reader.
    pub reached_end: RwSignal<bool>,
}

impl Default for ChromeState {
    fn default() -> Self {
        Self::new()
    }
}

impl ChromeState {
    pub fn new() -> Self {
        Self {
            headings: RwSignal::new(Vec::new()),
            title: RwSignal::new(String::new()),
            progress: RwSignal::new(0.0),
            active_id: RwSignal::new(None),
            show_top: RwSignal::new(false),
            past_title: RwSignal::new(false),
            toc_open: RwSignal::new(false),
            is_problem: RwSignal::new(false),
            nav_open: RwSignal::new(false),
            reached_end: RwSignal::new(false),
        }
    }

    /// The one scroll recompute (oracle thresholds: top 600 · sticky 160 · active ≤120).
    pub fn recompute(self) {
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
        let scroll = window.scroll_y().unwrap_or(0.0);
        let track = document
            .document_element()
            .map_or(0.0, |el| f64::from(el.scroll_height()))
            - window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
        self.progress.set(if track > 0.0 {
            (scroll / track).clamp(0.0, 1.0)
        } else {
            0.0
        });
        // The decision lives in `logic::progress` so it can be tested — this view layer is not
        // exercisable in a browser (the preview reports `innerHeight: 0`). Note problem pages
        // have no window scroll at all, so they never reach here; they do not auto-complete.
        if crate::catalog::logic::progress::is_at_end(scroll, track) {
            self.reached_end.set(true);
        }
        self.show_top.set(scroll > 600.0);
        self.past_title.set(scroll > 160.0);
        let mut active: Option<String> = None;
        for h in self.headings.get_untracked() {
            if let Some(el) = document.get_element_by_id(&h.id)
                && el.get_bounding_client_rect().top() <= 120.0
            {
                active = Some(h.id.clone());
            }
        }
        let fallback = self.headings.get_untracked().first().map(|h| h.id.clone());
        self.active_id.set(active.or(fallback));
    }
}

/// Jump to a heading, offset for the fixed header (`scroll-margin-top: 80px` twin).
pub fn scroll_to_heading(id: &str) {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let Some(el) = document.get_element_by_id(id) else {
        return;
    };
    let top = el.get_bounding_client_rect().top() + window.scroll_y().unwrap_or(0.0) - 80.0;
    window.scroll_to_with_x_and_y(0.0, top);
}

/// Harvest h2[id]/h3[id] from the rendered prose (the leading h1 is the page title).
pub fn harvest_headings(body: &web_sys::HtmlElement) -> Vec<Heading> {
    let mut out = Vec::new();
    if let Ok(nodes) = body.query_selector_all("h2[id], h3[id]") {
        for i in 0..nodes.length() {
            let Some(node) = nodes.get(i) else { continue };
            let Ok(el) = node.dyn_into::<web_sys::Element>() else {
                continue;
            };
            let Some(id) = el.get_attribute("id") else {
                continue;
            };
            out.push(Heading {
                id,
                text: el.text_content().unwrap_or_default(),
                level: if el.tag_name().eq_ignore_ascii_case("h3") {
                    3
                } else {
                    2
                },
            });
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// TOP PROGRESS BAR + STICKY WAYFINDING
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn ReadingProgress() -> impl IntoView {
    let chrome = expect_context::<ChromeState>();
    view! {
        <div
            class="reader-progress"
            style=move || format!("width: {:.2}%", chrome.progress.get() * 100.0)
        ></div>
    }
}

/// "Lesson title / active section" — appears past 160px of scroll.
#[component]
pub fn StickyBar() -> impl IntoView {
    let chrome = expect_context::<ChromeState>();
    let section = Memo::new(move |_| {
        chrome.active_id.get().and_then(|id| {
            chrome
                .headings
                .get()
                .iter()
                .find(|h| h.id == id)
                .map(|h| h.text.clone())
        })
    });
    view! {
        <div class="reader-sticky" data-on=move || chrome.past_title.get().to_string()>
            <span class="reader-sticky__title">{move || chrome.title.get()}</span>
            {move || section.get().map(|sec| view! {
                <span class="reader-sticky__sec-wrap">
                    <span class="reader-sticky__sep">"/"</span>
                    <span class="reader-sticky__section">{sec}</span>
                </span>
            })}
        </div>
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE RIGHT-EDGE MINIMAP — one tick per heading, spread apart, fill = progress
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn MiniMap() -> impl IntoView {
    let chrome = expect_context::<ChromeState>();
    // (heading, document fraction) — measured on heading changes and window resize.
    let positions: RwSignal<Vec<(Heading, f64)>> = RwSignal::new(Vec::new());
    let measure = move || {
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
        let total = document
            .document_element()
            .map_or(1.0, |el| f64::from(el.scroll_height()).max(1.0));
        let scroll = window.scroll_y().unwrap_or(0.0);
        let hs = chrome.headings.get_untracked();
        let raw: Vec<f64> = hs
            .iter()
            .filter_map(|h| document.get_element_by_id(&h.id))
            .map(|el| (el.get_bounding_client_rect().top() + scroll) / total)
            .collect();
        if raw.len() != hs.len() {
            return;
        }
        let spread = logic::spread_fractions(&raw);
        positions.set(hs.into_iter().zip(spread).collect());
    };
    Effect::new(move |_| {
        chrome.headings.track();
        measure();
    });
    let resized = window_event_listener(leptos::ev::resize, move |_| measure());
    on_cleanup(move || resized.remove());

    view! {
        {move || {
            let ticks = positions.get();
            (ticks.len() > 1).then(|| {
                #[allow(clippy::cast_precision_loss)]
                let gap = f64::min(0.05, 1.0 / (ticks.len() as f64 + 1.0));
                let tick_views: Vec<_> = ticks
                    .into_iter()
                    .map(|(h, frac)| {
                        let id = h.id.clone();
                        let active_id = h.id.clone();
                        let label = h.text.clone();
                        let level_class = if h.level == 3 {
                            "reader-minimap__tick reader-minimap__tick--l3"
                        } else {
                            "reader-minimap__tick reader-minimap__tick--l2"
                        };
                        view! {
                            <button
                                class=level_class
                                class:reader-minimap__tick--active=move || {
                                    chrome.active_id.get().as_deref() == Some(active_id.as_str())
                                }
                                style=format!(
                                    "top: {:.2}%; height: min(18px, {:.2}%)",
                                    frac * 100.0,
                                    gap * 100.0
                                )
                                aria-label=format!("Jump to {}", h.text)
                                on:click=move |_| scroll_to_heading(&id)
                            >
                                <span class="reader-minimap__label">{label}</span>
                            </button>
                        }
                    })
                    .collect();
                view! {
                    <aside class="reader-minimap" aria-label="Section map">
                        <div class="reader-minimap__track">
                            <div
                                class="reader-minimap__fill"
                                style=move || format!("height: {:.2}%", chrome.progress.get() * 100.0)
                            ></div>
                            {tick_views}
                        </div>
                    </aside>
                }
            })
        }}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE PAGE-TOC FAB + POPOVER (hidden on problem pages)
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn TocFab() -> impl IntoView {
    let chrome = expect_context::<ChromeState>();
    let open = chrome.toc_open;
    view! {
        {move || (!chrome.headings.get().is_empty() && !chrome.is_problem.get()).then(|| view! {
            <button
                class="reader-toc-fab"
                aria-label="On this page"
                aria-expanded=move || open.get().to_string()
                on:click=move |_| open.update(|o| *o = !*o)
            >
                <svg class="reader-toc-fab__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                     stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M8 6h13 M8 12h13 M8 18h13 M3 6h.01 M3 12h.01 M3 18h.01"></path>
                </svg>
            </button>
        })}
        {move || open.get().then(|| view! {
            <div class="reader-toc-scrim" on:click=move |_| open.set(false)></div>
            <div class="reader-toc-pop">
                <div class="reader-toc-pop__eyebrow">"On this page"</div>
                <ul class="reader-toc-pop__list">
                    {chrome
                        .headings
                        .get()
                        .into_iter()
                        .map(|h| {
                            let id = h.id.clone();
                            let row_id = h.id.clone();
                            let href = format!("#{}", h.id);
                            let btn_class = if h.level >= 3 {
                                "reader-toc-pop__btn reader-toc-pop__btn--l3"
                            } else {
                                "reader-toc-pop__btn"
                            };
                            view! {
                                <li
                                    class="reader-toc-pop__row"
                                    class:reader-toc-pop__row--active=move || {
                                        chrome.active_id.get().as_deref() == Some(row_id.as_str())
                                    }
                                >
                                    <a
                                        href=href
                                        class=btn_class
                                        on:click=move |event| {
                                            event.prevent_default();
                                            scroll_to_heading(&id);
                                            open.set(false);
                                        }
                                    >
                                        <span class="reader-toc-pop__tick"></span>
                                        <span class="reader-toc-pop__label">{h.text.clone()}</span>
                                    </a>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()}
                </ul>
            </div>
        })}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SCROLL-TO-TOP — appears past 600px, rides the top of the FAB stack
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn ScrollTop() -> impl IntoView {
    let chrome = expect_context::<ChromeState>();
    view! {
        <button
            class="reader-scrolltop"
            class:reader-scrolltop--hidden=move || !chrome.show_top.get()
            aria-label="Scroll to top"
            on:click=|_| {
                if let Some(window) = web_sys::window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
            }
        >
            <svg class="reader-toc-fab__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="m5 12 7-7 7 7 M12 19V5"></path>
            </svg>
        </button>
    }
}
