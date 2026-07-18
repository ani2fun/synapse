//! Widget placeholder discovery (oracle: `WidgetBlocks`): the markdown pipeline plants
//! `div.viz-widget[data-widget][data-payload]`; this finds them and decodes. Structure
//! resolution and payload decode are INDEPENDENT, so the host can tell "unknown structure"
//! from "unreadable payload" — honest cards, never a blank box.

use std::any::Any;

use leptos::prelude::*;
use synapse_shared::viz::graph::VizCases;
use synapse_shared::viz::vocabulary::VizStructure;
use wasm_bindgen::JsCast;

pub struct WidgetSpec {
    pub name: String,
    pub structure: Option<VizStructure>,
    pub cases: Option<VizCases>,
}

#[must_use]
pub fn discover(root: &web_sys::HtmlElement) -> Vec<(web_sys::HtmlElement, WidgetSpec)> {
    let mut out = Vec::new();
    let Ok(nodes) = root.query_selector_all("div.viz-widget") else {
        return out;
    };
    for index in 0..nodes.length() {
        let Some(node) = nodes.get(index) else { continue };
        let Ok(element) = node.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        let name = element.get_attribute("data-widget").unwrap_or_default();
        let payload = element.get_attribute("data-payload").unwrap_or_default();
        out.push((element, decode(&name, &payload)));
    }
    out
}

/// Discover AND mount every planted widget, returning the mount handles.
///
/// Three surfaces render authored markdown — the reader lesson, the problem description, and
/// the problem editorial — and each must mount what the pipeline plants. Keeping the loop here
/// rather than inline at each call site is deliberate: the editorial shipped without it and
/// every `viz` fence in an editorial rendered as an empty box under its caption.
#[must_use]
pub fn mount_widgets(root: &web_sys::HtmlElement) -> Vec<Box<dyn Any>> {
    discover(root)
        .into_iter()
        .map(|(element, spec)| {
            let handle = leptos::mount::mount_to(element, move || {
                view! {
                    <crate::viz::host::WidgetHost
                        name=spec.name
                        structure=spec.structure
                        cases=spec.cases
                    />
                }
            });
            Box::new(handle) as Box<dyn Any>
        })
        .collect()
}

#[must_use]
pub fn decode(name: &str, encoded_payload: &str) -> WidgetSpec {
    let structure = VizStructure::from_name(name);
    let cases = js_sys::decode_uri_component(encoded_payload)
        .ok()
        .map(String::from)
        .and_then(|json| serde_json::from_str::<VizCases>(&json).ok());
    WidgetSpec {
        name: name.trim().to_owned(),
        structure,
        cases,
    }
}
