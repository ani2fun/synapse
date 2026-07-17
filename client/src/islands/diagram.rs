//! The `@diagram` island (oracle: `MermaidView` via `@diagram/loader`). The extern binds the
//! tiny loader; the loader dynamic-imports mermaid, so the multi-hundred-KB chunk lands only
//! on lessons that actually contain a mermaid diagram.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "@diagram/loader")]
extern "C" {
    #[wasm_bindgen(js_name = renderMermaid)]
    fn render_mermaid_js(target: &web_sys::HtmlElement, src: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = renderD2)]
    fn render_d2_js(source: &str) -> js_sys::Promise;
}

/// Render mermaid source into `target` as an inline SVG. A malformed diagram rejects —
/// callers show the loud error card (ADR-S026), never a blank figure.
pub async fn render_mermaid(target: &web_sys::HtmlElement, src: &str) -> Result<(), JsValue> {
    wasm_bindgen_futures::JsFuture::from(render_mermaid_js(target, src)).await?;
    Ok(())
}

/// Compile + render one d2 fence's source to an SVG string (prose-first: d2 renders on the
/// client at mount, not at parse time). The multi-MB d2 WASM loads lazily on the first call.
/// A malformed diagram rejects — the caller shows the loud error card, never a blank figure.
pub async fn render_d2(source: &str) -> Result<String, JsValue> {
    let value = wasm_bindgen_futures::JsFuture::from(render_d2_js(source)).await?;
    Ok(value.as_string().unwrap_or_default())
}
