//! Friendly browser-console logger — a colored `SYNAPSE` badge + a per-level emoji, mirroring
//! the server's tracing style. Levels map to the MATCHING `console` method so `DevTools`
//! filtering still works. `debug` is suppressed off-localhost so production stays quiet. The
//! point: a dev session is **followable from the logs** — boot → route → lesson load →
//! markdown render → block mount → run → result — INFO as the follow-along level, DEBUG
//! filling in internals.

use wasm_bindgen::JsValue;

const RESET: &str = "color:inherit";

fn badge(color: &str) -> String {
    format!("background:{color};color:#fff;border-radius:3px;padding:1px 5px;font-weight:bold")
}

fn dev() -> bool {
    web_sys::window()
        .and_then(|w| w.location().hostname().ok())
        .is_some_and(|host| host == "localhost")
}

fn emit(method: fn(&JsValue, &JsValue, &JsValue), color: &str, emoji: &str, msg: &str) {
    method(
        &JsValue::from_str(&format!("%cSYNAPSE%c {emoji} {msg}")),
        &JsValue::from_str(&badge(color)),
        &JsValue::from_str(RESET),
    );
}

/// Lifecycle & notable events — the default follow-along level.
pub fn info(msg: &str) {
    emit(web_sys::console::info_3, "#2563eb", "ℹ️", msg);
}

/// Degraded but recovered (fallback, retry, stayed anonymous).
pub fn warn(msg: &str) {
    emit(web_sys::console::warn_3, "#d97706", "⚠️", msg);
}

/// A real failure needing attention.
pub fn error(msg: &str) {
    emit(web_sys::console::error_3, "#dc2626", "❌", msg);
}

/// Detailed internal steps — localhost-only, so production stays quiet.
pub fn debug(msg: &str) {
    if dev() {
        emit(web_sys::console::debug_3, "#6b7280", "🔍", msg);
    }
}
