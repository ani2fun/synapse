//! Widget placeholder discovery (oracle: `WidgetBlocks`): the markdown pipeline plants
//! `div.viz-widget[data-widget][data-payload]`; this finds them and decodes. Structure
//! resolution and payload decode are INDEPENDENT, so the host can tell "unknown structure"
//! from "unreadable payload" — honest cards, never a blank box.

use std::any::Any;

use crate::engine::graph::VizCases;
use crate::engine::vocabulary::VizStructure;
use crate::mount;
use leptos::prelude::*;

pub struct WidgetSpec {
    pub name: String,
    pub structure: Option<VizStructure>,
    pub cases: Option<VizCases>,
}

#[must_use]
pub fn discover(root: &web_sys::HtmlElement) -> Vec<(web_sys::HtmlElement, WidgetSpec)> {
    mount::elements(root, "div.viz-widget")
        .into_iter()
        .map(|element| {
            let name = element.get_attribute("data-widget").unwrap_or_default();
            let payload = element.get_attribute("data-payload").unwrap_or_default();
            let spec = decode(&name, &payload);
            (element, spec)
        })
        .collect()
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
        // Idempotent by marker (A10): the Astro host mounts body-wide at load AND again when
        // the editorial pane renders markdown late — the second sweep must skip live widgets,
        // not stack a second mount inside them.
        .filter(|(element, _)| element.get_attribute("data-viz-mounted").is_none())
        .map(|(element, spec)| {
            let _ = element.set_attribute("data-viz-mounted", "");
            mount::mount(element, move || {
                view! {
                    <crate::host::WidgetHost
                        name=spec.name
                        structure=spec.structure
                        cases=spec.cases
                    />
                }
            })
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
