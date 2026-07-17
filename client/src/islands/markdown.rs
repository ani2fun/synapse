//! The `@markdown` island (oracle: `MarkdownView` via `@markdown/loader`). The extern binds the
//! tiny loader; the loader dynamic-imports the renderer, so the markdown pipeline lands in its
//! own Vite chunk off the critical path.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "@markdown/loader")]
extern "C" {
    #[wasm_bindgen(js_name = renderMarkdown)]
    fn render_markdown_js(src: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = highlightCode)]
    fn highlight_code_js(code: &str, lang: &str) -> js_sys::Promise;
}

/// Render markdown source to HTML via the TS island. Errors surface as the JS value that
/// rejected the promise — callers decide how to degrade.
pub async fn render(src: &str) -> Result<String, JsValue> {
    let value = wasm_bindgen_futures::JsFuture::from(render_markdown_js(src)).await?;
    Ok(value.as_string().unwrap_or_default())
}

/// Highlight one snippet with the pipeline's shiki theme — the lazy workbench's pre-mount
/// placeholder (unknown languages fall back to plaintext inside the island).
pub async fn highlight(code: &str, lang: &str) -> Result<String, JsValue> {
    let value = wasm_bindgen_futures::JsFuture::from(highlight_code_js(code, lang)).await?;
    Ok(value.as_string().unwrap_or_default())
}
