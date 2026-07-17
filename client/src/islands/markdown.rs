//! The `@markdown` island (oracle: `MarkdownView` via `@markdown/loader`). The extern binds the
//! tiny loader; the loader dynamic-imports the renderer, so the markdown pipeline lands in its
//! own Vite chunk off the critical path.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "@markdown/loader")]
extern "C" {
    #[wasm_bindgen(js_name = renderMarkdown)]
    fn render_markdown_js(src: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = highlightCode)]
    fn highlight_code_js(code: &str, lang: &str) -> js_sys::Promise;
}

thread_local! {
    // Rendered HTML memoized by a hash of the RAW markdown, so re-navigation (back/forward,
    // re-clicking a visited lesson) skips the whole pipeline — unified + shiki + d2 grouping.
    // Since d2 now renders on the client (not baked into this HTML), cached strings stay small.
    // Keyed by content hash, so a dev edit (new raw) misses cleanly.
    static RENDER_CACHE: RefCell<HashMap<u64, String>> = RefCell::new(HashMap::new());
}

fn content_hash(src: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut hasher);
    hasher.finish()
}

/// Render markdown source to HTML via the TS island, memoized by content hash. Errors surface
/// as the JS value that rejected the promise — callers decide how to degrade.
pub async fn render(src: &str) -> Result<String, JsValue> {
    let key = content_hash(src);
    if let Some(cached) = RENDER_CACHE.with_borrow(|c| c.get(&key).cloned()) {
        return Ok(cached);
    }
    let value = wasm_bindgen_futures::JsFuture::from(render_markdown_js(src)).await?;
    let html = value.as_string().unwrap_or_default();
    RENDER_CACHE.with_borrow_mut(|c| c.insert(key, html.clone()));
    Ok(html)
}

/// Highlight one snippet with the pipeline's shiki theme — the lazy workbench's pre-mount
/// placeholder (unknown languages fall back to plaintext inside the island).
pub async fn highlight(code: &str, lang: &str) -> Result<String, JsValue> {
    let value = wasm_bindgen_futures::JsFuture::from(highlight_code_js(code, lang)).await?;
    Ok(value.as_string().unwrap_or_default())
}
