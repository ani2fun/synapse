//! The wasm-bindgen surface (A10): what the Astro app's lazy `viz.ts` loader calls. Three
//! verbs — mount the inline widgets, open the Visualise modal, install the bearer provider.
//!
//! Self-hosting: under the old client the modal store lives in App context and the shell
//! mounts `<VisualiseModal/>` once. There is no App here, so the entry mints ONE store under
//! a detached root owner (the session-cache lesson of step 39: signals that outlive views
//! must never be owned by a caller's reactive scope) and mounts the modal into its own
//! document-level host on first need, providing that store as context so `modal.rs` runs
//! unchanged under both hosts.
//!
//! Handles from `mount_widgets` are deliberately leaked into a thread-local: the Astro app is
//! an MPA — every navigation is a full page load, so "page lifetime" and "wasm instance
//! lifetime" are the same thing and there is no unmount path to serve.

use std::any::Any;
use std::cell::RefCell;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::engine::vocabulary::VizStructure;
use crate::modal::{VisualiseModal, VizModalStore};
use crate::{blocks, session};

thread_local! {
    static WIDGET_HANDLES: RefCell<Vec<Box<dyn Any>>> = const { RefCell::new(Vec::new()) };
    static MODAL: RefCell<Option<VizModalStore>> = const { RefCell::new(None) };
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
/// the structure + optional root exactly as the old client's Visualise button did. An unknown
/// hint is refused honestly (logged, `false`) rather than opening a modal that could only show
/// a failure card for an authoring mistake.
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
