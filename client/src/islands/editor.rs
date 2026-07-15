//! The `@editor` island (oracle: `MonacoEditor.scala` over `@editor/loader`). The extern binds
//! the tiny loader; monaco core + its worker live in their own lazy chunk. The `MountedEditor`
//! wrapper owns the JS callbacks — dropping it disposes the editor AND the closures, so a
//! block unmount can't leave monaco listeners calling into freed wasm.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "@editor/loader")]
extern "C" {
    #[wasm_bindgen(js_name = mountEditor)]
    fn mount_editor_js(
        container: &web_sys::HtmlElement,
        value: &str,
        language: &str,
        read_only: bool,
        dark: bool,
        on_change: &js_sys::Function,
        on_run: &js_sys::Function,
        on_toggle_edit: &js_sys::Function,
    ) -> js_sys::Promise;

    pub type EditorHandle;
    #[wasm_bindgen(method, js_name = setValue)]
    fn set_value_js(this: &EditorHandle, value: &str);
    #[wasm_bindgen(method, js_name = getValue)]
    fn get_value_js(this: &EditorHandle) -> String;
    #[wasm_bindgen(method, js_name = setReadOnly)]
    fn set_read_only_js(this: &EditorHandle, read_only: bool);
    #[wasm_bindgen(method, js_name = dispose)]
    fn dispose_js(this: &EditorHandle);
}

pub struct MountedEditor {
    handle: EditorHandle,
    _on_change: Closure<dyn FnMut(String)>,
    _on_run: Closure<dyn FnMut()>,
    _on_toggle_edit: Closure<dyn FnMut()>,
}

impl MountedEditor {
    pub fn set_value(&self, value: &str) {
        self.handle.set_value_js(value);
    }

    pub fn get_value(&self) -> String {
        self.handle.get_value_js()
    }

    pub fn set_read_only(&self, read_only: bool) {
        self.handle.set_read_only_js(read_only);
    }
}

impl Drop for MountedEditor {
    fn drop(&mut self) {
        self.handle.dispose_js();
    }
}

/// Mount a Monaco editor into `container`. The oracle's default height rule is applied by the
/// caller (`clamp(lines*20+28, 64, 520)` px).
pub async fn mount(
    container: &web_sys::HtmlElement,
    value: &str,
    language: &str,
    read_only: bool,
    on_change: impl FnMut(String) + 'static,
    on_run: impl FnMut() + 'static,
    on_toggle_edit: impl FnMut() + 'static,
) -> Result<MountedEditor, JsValue> {
    let on_change = Closure::<dyn FnMut(String)>::new(on_change);
    let on_run = Closure::<dyn FnMut()>::new(on_run);
    let on_toggle_edit = Closure::<dyn FnMut()>::new(on_toggle_edit);
    let promise = mount_editor_js(
        container,
        value,
        language,
        read_only,
        false, // light theme; the dark-mode step re-themes
        on_change.as_ref().unchecked_ref(),
        on_run.as_ref().unchecked_ref(),
        on_toggle_edit.as_ref().unchecked_ref(),
    );
    let handle = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(MountedEditor {
        handle: handle.unchecked_into(),
        _on_change: on_change,
        _on_run: on_run,
        _on_toggle_edit: on_toggle_edit,
    })
}

/// The oracle's editor height rule.
pub fn default_height_px(source: &str) -> u32 {
    let lines = u32::try_from(source.lines().count()).unwrap_or(u32::MAX);
    lines.saturating_mul(20).saturating_add(28).clamp(64, 520)
}
