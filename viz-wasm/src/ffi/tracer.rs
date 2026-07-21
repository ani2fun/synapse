//! The `@tracer` island (oracle: `PythonTracer`/`JavaTracer` over `@tracer/loader`): wraps
//! user source in the language's trace harness. Each ~900-line harness string lives in its
//! own lazy chunk, loaded on the reader's FIRST Visualise of that language.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "@tracer/loader")]
extern "C" {
    #[wasm_bindgen(js_name = loadWrapPython)]
    fn load_wrap_python_js() -> js_sys::Promise;
    #[wasm_bindgen(js_name = loadWrapJava)]
    fn load_wrap_java_js() -> js_sys::Promise;
}

async fn wrap_with(loader: js_sys::Promise, source: &str) -> Result<String, JsValue> {
    let wrap_fn = wasm_bindgen_futures::JsFuture::from(loader).await?;
    let wrap_fn: js_sys::Function = wrap_fn.unchecked_into();
    let wrapped = wrap_fn.call1(&JsValue::NULL, &JsValue::from_str(source))?;
    wrapped
        .as_string()
        .ok_or_else(|| JsValue::from_str("tracer wrap returned a non-string"))
}

pub async fn wrap_python(source: &str) -> Result<String, JsValue> {
    wrap_with(load_wrap_python_js(), source).await
}

pub async fn wrap_java(source: &str) -> Result<String, JsValue> {
    wrap_with(load_wrap_java_js(), source).await
}
