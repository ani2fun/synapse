//! Viewport-lazy workbenches (qna Q1, option B): a `run` block renders shiki + toolbar
//! until it scrolls NEAR the viewport — only then does its Monaco instance mount — and a
//! page-level cap evicts the oldest FAR editor when too many are live at once. Block state
//! (edits, run results, verdicts) lives in `BlockStore`, so an evicted editor loses nothing;
//! re-approaching simply re-mounts over the same store.

use std::cell::{Cell, RefCell};

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

/// How many Monaco instances may be live at once before FAR ones are evicted. Visible
/// blocks are never evicted — a page showing more than this simultaneously keeps them all.
pub const MAX_LIVE_EDITORS: usize = 3;

/// The near-viewport margin: mount this far BEFORE the block scrolls in, so the editor is
/// ready by the time the reader reaches it.
const NEAR_MARGIN: &str = "600px 0px 600px 0px";

// ─────────────────────────────────────────────────────────────────────────────
// NEAR-VIEWPORT OBSERVATION
// ─────────────────────────────────────────────────────────────────────────────

/// Keeps the `IntersectionObserver` (and its callback closure) alive; dropping disconnects.
pub struct NearWatch {
    observer: web_sys::IntersectionObserver,
    _callback: Closure<dyn FnMut(js_sys::Array)>,
}

impl Drop for NearWatch {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

/// Watch one block: `near` tracks whether it is within the mount margin of the viewport.
pub fn watch_near(node: &web_sys::Element, near: RwSignal<bool>) -> Option<NearWatch> {
    let callback = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
        for entry in entries.iter() {
            let Ok(entry) = entry.dyn_into::<web_sys::IntersectionObserverEntry>() else {
                continue;
            };
            crate::log::debug(&format!("lazy workbench: near = {}", entry.is_intersecting()));
            near.set(entry.is_intersecting());
        }
    });
    let options = web_sys::IntersectionObserverInit::new();
    options.set_root_margin(NEAR_MARGIN);
    let observer =
        match web_sys::IntersectionObserver::new_with_options(callback.as_ref().unchecked_ref(), &options) {
            Ok(observer) => observer,
            Err(error) => {
                crate::log::warn(&format!(
                    "lazy workbench: IntersectionObserver unavailable ({error:?}) — mounting eagerly"
                ));
                near.set(true); // degrade to the pre-lazy behavior, never a dead editor
                return None;
            }
        };
    observer.observe(node);
    crate::log::debug("lazy workbench: watching");
    Some(NearWatch {
        observer,
        _callback: callback,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// THE LIVE-EDITOR REGISTRY (page-level LRU cap)
// ─────────────────────────────────────────────────────────────────────────────

struct LiveEntry {
    id: u64,
    near: RwSignal<bool>,
    evict: Callback<()>,
}

thread_local! {
    static LIVE: RefCell<Vec<LiveEntry>> = const { RefCell::new(Vec::new()) };
    static NEXT_ID: Cell<u64> = const { Cell::new(0) };
}

/// A mounted editor announces itself; the registry evicts the oldest FAR editor while the
/// cap is exceeded. Returns the id to `deregister` on unmount/eviction.
pub fn register(near: RwSignal<bool>, evict: Callback<()>) -> u64 {
    let id = NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    LIVE.with_borrow_mut(|live| live.push(LiveEntry { id, near, evict }));
    enforce_cap();
    id
}

pub fn deregister(id: u64) {
    LIVE.with_borrow_mut(|live| live.retain(|entry| entry.id != id));
}

fn enforce_cap() {
    loop {
        let victim = LIVE.with_borrow(|live| {
            if live.len() <= MAX_LIVE_EDITORS {
                return None;
            }
            live.iter()
                .find(|entry| !entry.near.get_untracked())
                .map(|entry| (entry.id, entry.evict))
        });
        let Some((id, evict)) = victim else { return };
        crate::log::debug("lazy workbench: evicting a far editor (cap reached)");
        deregister(id);
        evict.run(());
    }
}
