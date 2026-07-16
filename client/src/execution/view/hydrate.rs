//! Placeholder hydration (oracle: `RunnableBlocks.discover` + `MarkdownView.mountBlocks`):
//! after the lesson HTML lands via `inner_html`, find every `div.workbench`, decode its
//! `data-variants`, and mount a live `RunnableBlock` into it. The returned boxed unmount
//! handles keep the mounts alive — dropping them (lesson navigation / unmount) tears the
//! blocks down, which disposes their monaco editors.

use std::any::Any;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::execution::logic;
use crate::execution::view::RunnableBlock;

pub fn hydrate_workbenches(
    root: &web_sys::HtmlElement,
    lesson_path: &[String],
    auth: crate::identity::state::AuthStore,
    code_sink: RwSignal<(String, String)>,
) -> Vec<Box<dyn Any>> {
    let mut handles: Vec<Box<dyn Any>> = Vec::new();
    let Ok(nodes) = root.query_selector_all("div.workbench") else {
        return handles;
    };
    for index in 0..nodes.length() {
        let Some(node) = nodes.get(index) else { continue };
        let Ok(element) = node.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        let Some(encoded) = element.get_attribute("data-variants") else {
            continue;
        };
        let Ok(decoded) = js_sys::decode_uri_component(&encoded) else {
            continue;
        };
        let Some(variants) = logic::parse_variants(&String::from(decoded)) else {
            continue;
        };
        // First variant for now (the language switch joins with the multi-variant step).
        let Some(variant) = variants.into_iter().next() else {
            continue;
        };
        // The authored suite rides in data-spec (absent on plain lesson blocks).
        let spec = element
            .get_attribute("data-spec")
            .and_then(|encoded| js_sys::decode_uri_component(&encoded).ok())
            .and_then(|decoded| serde_json::from_str(&String::from(decoded)).ok());
        let path = lesson_path.to_vec();
        let handle = leptos::mount::mount_to(element, move || {
            view! { <RunnableBlock variant=variant spec=spec lesson_path=path auth=auth code_sink=code_sink /> }
        });
        handles.push(Box::new(handle));
    }
    handles
}
