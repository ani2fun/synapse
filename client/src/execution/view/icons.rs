//! The workbench toolbar icons (oracle: `Icons.scala`, lucide strokes) — split from
//! `runnable.rs` to keep it under the client file cap.

use leptos::prelude::*;

pub(crate) fn icon_play(class: &'static str) -> impl IntoView {
    view! {
        <svg class=class viewBox="0 0 24 24" aria-hidden="true">
            <path d="M6 3v18l14-9z" fill="currentColor"></path>
        </svg>
    }
}

pub(crate) fn icon_chevron_down() -> impl IntoView {
    view! {
        <svg class="wb__lang-chev" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="m6 9 6 6 6-6"></path>
        </svg>
    }
}

pub(crate) fn icon_lock() -> impl IntoView {
    view! {
        <svg class="wb__ghost-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <rect x="3" y="11" width="18" height="11" rx="2"></rect>
            <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
        </svg>
    }
}

pub(crate) fn icon_eye() -> impl IntoView {
    view! {
        <svg class="wb__ghost-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7z"></path>
            <circle cx="12" cy="12" r="3"></circle>
        </svg>
    }
}

pub(crate) fn icon_rocket() -> impl IntoView {
    view! {
        <svg class="wb__submit-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z"></path>
            <path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z"></path>
            <path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0"></path>
            <path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5"></path>
        </svg>
    }
}

pub(crate) fn icon_reset() -> impl IntoView {
    view! {
        <svg class="wb__ghost-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"></path>
            <path d="M3 3v5h5"></path>
        </svg>
    }
}
