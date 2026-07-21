//! The `@editor` island (oracle: `MonacoEditor.scala` over `@editor/loader`). The extern binds
//! the tiny loader; monaco core + its worker live in their own lazy chunk. The `MountedEditor`
//! wrapper owns the JS callbacks — dropping it disposes the editor AND the closures, so a
//! block unmount can't leave monaco listeners calling into freed wasm.

use wasm_bindgen::prelude::*;

/// Broadcast on `window` when something revealed an editor that may have mounted collapsed
/// (the editorial's section pills). Every mounted viewer re-measures; `layout()` is idempotent,
/// so a listener that was already sized pays nothing.
pub const RELAYOUT_EVENT: &str = "synapse:relayout";

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
        on_submit: Option<&js_sys::Function>,
    ) -> js_sys::Promise;

    pub type EditorHandle;
    #[wasm_bindgen(method, js_name = setValue)]
    fn set_value_js(this: &EditorHandle, value: &str);
    #[wasm_bindgen(method, js_name = getValue)]
    fn get_value_js(this: &EditorHandle) -> String;
    #[wasm_bindgen(method, js_name = setReadOnly)]
    fn set_read_only_js(this: &EditorHandle, read_only: bool);
    #[wasm_bindgen(method, js_name = setTheme)]
    fn set_theme_js(this: &EditorHandle, dark: bool);
    #[wasm_bindgen(method, js_name = setLineHighlights)]
    fn set_line_highlights_js(this: &EditorHandle, current: u32, next: Option<u32>);
    #[wasm_bindgen(method, js_name = setLanguage)]
    fn set_language_js(this: &EditorHandle, fence_lang: &str);
    #[wasm_bindgen(method, js_name = relayout)]
    fn relayout_js(this: &EditorHandle);
    #[wasm_bindgen(method, js_name = dispose)]
    fn dispose_js(this: &EditorHandle);
}

pub struct MountedEditor {
    handle: EditorHandle,
    _on_change: Closure<dyn FnMut(String)>,
    _on_run: Closure<dyn FnMut()>,
    _on_toggle_edit: Closure<dyn FnMut()>,
    _on_submit: Option<Closure<dyn FnMut()>>,
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

    /// `monaco.editor.setTheme` is GLOBAL (re-themes every editor) — cheap + idempotent.
    pub fn set_theme(&self, dark: bool) {
        self.handle.set_theme_js(dark);
    }

    /// The Visualise modal's source pane: highlight the current (+ next) 1-indexed lines.
    pub fn set_line_highlights(&self, current: u32, next: Option<u32>) {
        self.handle.set_line_highlights_js(current, next);
    }

    /// Re-tokenize the buffer as another fence language (the workbench language tabs).
    pub fn set_language(&self, fence_lang: &str) {
        self.handle.set_language_js(fence_lang);
    }

    /// Force a re-measure after the editor's container changes size for a reason monaco's own
    /// observer can miss — chiefly a reveal out of `display: none`, where it mounted 0×0 and
    /// rendered no lines. Idempotent and cheap.
    pub fn relayout(&self) {
        self.handle.relayout_js();
    }
}

impl Drop for MountedEditor {
    fn drop(&mut self) {
        self.handle.dispose_js();
    }
}

/// The workbench verbs an editor wires (oracle: `Keymap`) — submit only where the surface has
/// the verb.
pub struct EditorCallbacks {
    pub on_change: Box<dyn FnMut(String)>,
    pub on_run: Box<dyn FnMut()>,
    pub on_toggle_edit: Box<dyn FnMut()>,
    pub on_submit: Option<Box<dyn FnMut()>>,
}

/// Mount a Monaco editor into `container`. The oracle's default height rule is applied by the
/// caller (`clamp(lines*20+28, 64, 520)` px).
pub async fn mount(
    container: &web_sys::HtmlElement,
    value: &str,
    language: &str,
    read_only: bool,
    dark: bool,
    callbacks: EditorCallbacks,
) -> Result<MountedEditor, JsValue> {
    let on_change = Closure::<dyn FnMut(String)>::new(callbacks.on_change);
    let on_run = Closure::<dyn FnMut()>::new(callbacks.on_run);
    let on_toggle_edit = Closure::<dyn FnMut()>::new(callbacks.on_toggle_edit);
    let on_submit = callbacks.on_submit.map(Closure::<dyn FnMut()>::new);
    let promise = mount_editor_js(
        container,
        value,
        language,
        read_only,
        dark,
        on_change.as_ref().unchecked_ref(),
        on_run.as_ref().unchecked_ref(),
        on_toggle_edit.as_ref().unchecked_ref(),
        on_submit.as_ref().map(|c| c.as_ref().unchecked_ref()),
    );
    let handle = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(MountedEditor {
        handle: handle.unchecked_into(),
        _on_change: on_change,
        _on_run: on_run,
        _on_toggle_edit: on_toggle_edit,
        _on_submit: on_submit,
    })
}

/// The oracle's editor height rule.
pub fn default_height_px(source: &str) -> u32 {
    let lines = u32::try_from(source.lines().count()).unwrap_or(u32::MAX);
    lines.saturating_mul(20).saturating_add(28).clamp(64, 520)
}
