//! The mount kit slice (A10): the two mechanics `blocks` needs from the client's hydration
//! seam (step 58), carried in-crate so widget discovery works under BOTH hosts. The store
//! caravan deliberately did NOT come along — viz owns no app-level context stores; the modal
//! store is provided by whichever host mounts the modal.

use std::any::Any;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// Every element under `root` matching `selector`, already narrowed to `HtmlElement`.
/// Non-element nodes (there are none in practice) are skipped silently.
#[must_use]
pub fn elements(root: &web_sys::HtmlElement, selector: &str) -> Vec<web_sys::HtmlElement> {
    let mut found = Vec::new();
    let Ok(nodes) = root.query_selector_all(selector) else {
        return found;
    };
    for index in 0..nodes.length() {
        let Some(node) = nodes.get(index) else { continue };
        if let Ok(element) = node.dyn_into::<web_sys::HtmlElement>() {
            found.push(element);
        }
    }
    found
}

/// Mount one island into `element` and box its unmount handle. The boxed handle is the
/// island's lifetime: hold it to keep the mount alive, drop it to tear the island down
/// (which disposes its reactive owner — and any Monaco editor with it).
#[must_use]
pub fn mount<F, N>(element: web_sys::HtmlElement, view_fn: F) -> Box<dyn Any>
where
    F: FnOnce() -> N + 'static,
    N: IntoView + 'static,
{
    Box::new(leptos::mount::mount_to(element, view_fn))
}
