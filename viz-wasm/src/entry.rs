//! The wasm-bindgen surface: what the Astro app's lazy `viz.ts` loader calls. Mount the inline
//! widgets, open the Visualise modal, install the bearer provider — and, for the `/viz` page,
//! mount the docked panel, trace into it, and read the structure vocabulary back out.
//!
//! Self-hosting: there is no App shell to hold the modal store, so the entry mints ONE store
//! under a detached root owner (signals that outlive views must never be owned by a caller's
//! reactive scope) and mounts the modal into its own document-level host on first need,
//! providing that store as context so `modal.rs` runs against it directly.
//!
//! Handles from `mount_widgets` are deliberately leaked into a thread-local: the Astro app is
//! an MPA — every navigation is a full page load, so "page lifetime" and "wasm instance
//! lifetime" are the same thing and there is no unmount path to serve.

use std::any::Any;
use std::cell::RefCell;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::engine::d2;
use crate::engine::vocabulary::VizStructure;
use crate::modal::{VisualiseModal, VizModalStore};
use crate::panel::{VizPanel, VizPanelStore};
use crate::{blocks, session};

thread_local! {
    static WIDGET_HANDLES: RefCell<Vec<Box<dyn Any>>> = const { RefCell::new(Vec::new()) };
    static MODAL: RefCell<Option<VizModalStore>> = const { RefCell::new(None) };
    static PANEL: RefCell<Option<VizPanelStore>> = const { RefCell::new(None) };
    // The detached owner for everything that outlives a view (the modal store's signal).
    static ENTRY_OWNER: Owner = Owner::new_root(None);
}

/// The one modal store, minted on first use under the detached owner; the modal component
/// mounts alongside it, into a host div appended to `<body>`.
fn modal_store() -> Option<VizModalStore> {
    let existing = MODAL.with_borrow(|m| *m);
    if let Some(store) = existing {
        return Some(store);
    }
    let document = web_sys::window()?.document()?;
    let body = document.body()?;
    let host = document.create_element("div").ok()?;
    host.set_class_name("viz-modal-root");
    body.append_child(&host).ok()?;
    let store = ENTRY_OWNER.with(|owner| owner.with(VizModalStore::new));
    let handle = crate::mount::mount(host.unchecked_into(), move || {
        provide_context(store);
        view! { <VisualiseModal /> }
    });
    WIDGET_HANDLES.with_borrow_mut(|handles| handles.push(handle));
    MODAL.with_borrow_mut(|m| *m = Some(store));
    crate::log::debug("viz modal self-hosted (document-level root)");
    Some(store)
}

/// Discover and mount every planted `div.viz-widget` under `<body>`. Returns the count.
#[wasm_bindgen]
#[must_use]
pub fn viz_mount_widgets() -> usize {
    console_error_panic_hook::set_once();
    let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    else {
        return 0;
    };
    let handles = blocks::mount_widgets(&body.unchecked_into());
    let count = handles.len();
    WIDGET_HANDLES.with_borrow_mut(|all| all.extend(handles));
    crate::log::info(&format!("viz: mounted {count} widget(s)"));
    count
}

/// Open the Visualise modal for one traced run — the `window.__synapseViz` contract's landing
/// point. `viz_hint` is the variant's RAW `viz=` hint; `VizStructure::parse` splits it into
/// the structure + optional root exactly as the workbench's Visualise button expects. An
/// unknown hint is refused honestly (logged, `false`) rather than opening a modal that could
/// only show a failure card for an authoring mistake.
#[wasm_bindgen]
pub fn viz_open_modal(language: &str, source: &str, viz_hint: &str, stdin: &str) -> bool {
    console_error_panic_hook::set_once();
    let Some((structure, root)) = VizStructure::parse(viz_hint) else {
        crate::log::warn(&format!("viz: unusable viz hint “{viz_hint}” — not opening"));
        return false;
    };
    let Some(store) = modal_store() else {
        return false;
    };
    let token = structure.token();
    let key = session::Key {
        language: language.to_owned(),
        source: source.to_owned(),
        structure,
        root,
        stdin: stdin.to_owned(),
    };
    crate::log::info(&format!("viz: open modal ({language}, {token})"));
    store.open(session::obtain(key));
    true
}

/// Install the bearer provider: a JS function returning the current token (or null). The
/// trace's `/api/run` calls read it per-request, so a token refresh needs no re-install.
#[wasm_bindgen]
pub fn viz_install_token(provider: js_sys::Function) {
    crate::api::set_token_provider(move || {
        provider
            .call0(&JsValue::NULL)
            .ok()
            .and_then(|value| value.as_string())
    });
    crate::log::debug("viz: bearer provider installed");
}

// ─────────────────────────────────────────────────────────────────────────────
// THE DOCKED PANEL — the `/viz` page
// ─────────────────────────────────────────────────────────────────────────────

/// Mount the panel into a host element the page owns. The store is minted under the SAME
/// detached owner the modal uses, because the page drives it from JS callbacks whose reactive
/// scopes end the moment they return.
///
/// Idempotent: a second call on an already-mounted panel is a no-op rather than a second mount
/// competing for one element.
#[wasm_bindgen]
pub fn viz_mount_panel(host: web_sys::HtmlElement) -> bool {
    console_error_panic_hook::set_once();
    if PANEL.with_borrow(Option::is_some) {
        crate::log::debug("viz: panel already mounted");
        return true;
    }
    let store = ENTRY_OWNER.with(|owner| owner.with(VizPanelStore::new));
    let handle = crate::mount::mount(host, move || {
        provide_context(store);
        view! { <VizPanel /> }
    });
    WIDGET_HANDLES.with_borrow_mut(|handles| handles.push(handle));
    PANEL.with_borrow_mut(|p| *p = Some(store));
    crate::log::info("viz: panel mounted");
    true
}

/// Trace `source` into the mounted panel. `viz_hint` is the same `<structure>[:<root>]` token an
/// authored fence carries — the page's structure picker composes it — so an unknown one is
/// refused here exactly as it is for the modal, rather than drawing a confidently wrong picture.
///
/// Cached, not forced: the session key covers language, source, structure, root and stdin, so an
/// edit is already a different key and pressing Trace twice on unchanged input costs nothing.
#[wasm_bindgen]
pub fn viz_panel_trace(language: &str, source: &str, viz_hint: &str, stdin: &str) -> bool {
    console_error_panic_hook::set_once();
    let Some(store) = PANEL.with_borrow(|p| *p) else {
        crate::log::warn("viz: trace requested before the panel mounted");
        return false;
    };
    let Some((structure, root)) = VizStructure::parse(viz_hint) else {
        crate::log::warn(&format!("viz: unusable viz hint “{viz_hint}” — not tracing"));
        return false;
    };
    let token = structure.token();
    let key = session::Key {
        language: language.to_owned(),
        source: source.to_owned(),
        structure,
        root,
        stdin: stdin.to_owned(),
    };
    crate::log::info(&format!("viz: panel trace ({language}, {token})"));
    store.show(session::obtain(key));
    true
}

/// The authored structure vocabulary, as a JSON array of tokens — what the page's picker offers.
/// Served from the crate rather than spelled again in TypeScript: two copies of a closed set is
/// how one of them silently grows a token the other cannot render.
#[wasm_bindgen]
#[must_use]
pub fn viz_structures() -> String {
    let tokens: Vec<&str> = VizStructure::ALL.iter().map(|s| s.token()).collect();
    serde_json::to_string(&tokens).unwrap_or_else(|_| "[]".to_owned())
}

/// The step on screen — or the whole walkthrough — as **d2 source**.
///
/// `mode` is `"step"` or `"walkthrough"`. Source, not a fence: the markdown wrapper is the page's
/// business, and the `/d2` editor wants the bare document anyway.
///
/// `None` when there is nothing to export — no panel, no trace yet, or a trace that failed. The
/// caller renders its control from that answer rather than offering a button that copies "".
#[wasm_bindgen]
#[must_use]
pub fn viz_panel_export_d2(mode: &str) -> Option<String> {
    console_error_panic_hook::set_once();
    let store = PANEL.with_borrow(|p| *p)?;
    let session = store.current.get_untracked()?;
    let cases = store.ready_cases()?;
    let graph = cases
        .cases
        .get(store.case_idx.get_untracked().min(cases.cases.len() - 1))?;
    let structure = session.key.structure;
    let source = match mode {
        "walkthrough" => d2::walkthrough_source(graph, structure),
        _ => d2::step_source(graph, structure, store.step.get_untracked().index),
    };
    crate::log::info(&format!("viz: exported {} as d2 ({mode})", structure.token()));
    Some(source)
}
