//! The theme probe (A10): the modal's source-pane Monaco needs to know the CURRENT theme at
//! mount. Both hosts stamp dark mode the same way — the `dark` CLASS on `<html>` (step 25's
//! contract, which the Astro app kept) — so reading the class IS the seam; no store required.

/// Whether the document is currently in dark mode.
#[must_use]
pub fn html_is_dark() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .is_some_and(|root| root.class_list().contains("dark"))
}
