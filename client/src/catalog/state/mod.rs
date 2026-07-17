//! Reactive catalog state (oracle: `CatalogStore.scala`, the state layer). The store lives in
//! Leptos CONTEXT, created under `App`'s owner — a module-level cache would tie the signal to
//! whichever page touched it first and go inert when that page unmounts (found the hard way in
//! this step's browser verify). The index is fetched once and shared by the library page and
//! every lesson's sidebar; the cache drops on failure so a transient miss doesn't pin a broken
//! index for the whole session. Lessons are fetch-per-navigation (the server caches the build).

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::catalog::{LessonPayloadDto, SynapseIndexDto};

use crate::api::{self, AsyncResult};

/// The app-level catalog store. `Copy` — signal handles, not data.
#[derive(Clone, Copy)]
pub struct CatalogStore {
    index: RwSignal<AsyncResult<SynapseIndexDto>>,
    index_started: StoredValue<bool>,
}

impl CatalogStore {
    /// Created ONCE in `App` and provided as context.
    pub fn provide() {
        provide_context(Self {
            index: RwSignal::new(AsyncResult::Loading),
            index_started: StoredValue::new(false),
        });
    }

    pub fn from_context() -> Self {
        expect_context::<Self>()
    }

    /// The shared index signal — the first caller triggers the fetch; a failure re-arms it so
    /// the next navigation re-fetches.
    pub fn index(self) -> RwSignal<AsyncResult<SynapseIndexDto>> {
        if !self.index_started.get_value() {
            self.index_started.set_value(true);
            self.index.set(AsyncResult::Loading);
            crate::log::info("loading catalog index");
            spawn_local(async move {
                match api::index().await {
                    Ok(idx) => {
                        crate::log::debug(&format!(
                            "catalog index loaded: {} top-level entries",
                            idx.entries.len()
                        ));
                        self.index.set(AsyncResult::Loaded(idx));
                    }
                    Err(message) => {
                        crate::log::error(&format!("index: {message}"));
                        self.index_started.set_value(false);
                        self.index.set(AsyncResult::Failed(message));
                    }
                }
            });
        }
        self.index
    }
}

/// One lesson fetch, spawned per navigation.
pub fn load_lesson(path: Vec<String>) -> RwSignal<AsyncResult<LessonPayloadDto>> {
    let state = RwSignal::new(AsyncResult::Loading);
    crate::log::info(&format!("loading lesson: {}", path.join("/")));
    spawn_local(async move {
        match api::lesson(&path).await {
            Ok(payload) => {
                crate::log::debug(&format!("lesson loaded: {}", payload.frontmatter.title));
                state.set(AsyncResult::Loaded(payload));
            }
            Err(message) => {
                crate::log::error(&format!("lesson: {message}"));
                state.set(AsyncResult::Failed(message));
            }
        }
    });
    state
}

// ─────────────────────────────────────────────────────────────────────────────
// READING PREFERENCES (oracle: `ReadingPrefs` — the persisted half)
// ─────────────────────────────────────────────────────────────────────────────
// Loaded from localStorage and reflected onto `<html>` as `data-reader-*`
// attributes the stylesheet reads — set once BEFORE first paint (provide() runs
// in App's body), surviving navigation with no flash and no per-page rewiring.

use crate::catalog::logic::prefs::{self, Prefs};

const PREFS_KEY: &str = "reader-prefs";

#[derive(Clone, Copy)]
pub struct PrefsStore {
    pub prefs: RwSignal<Prefs>,
}

impl PrefsStore {
    /// Created ONCE in `App`: load → reflect onto `<html>` → provide as context.
    pub fn provide() {
        let loaded = prefs::parse(storage_get(PREFS_KEY).as_deref());
        apply_to_html(&loaded);
        provide_context(Self {
            prefs: RwSignal::new(loaded),
        });
    }

    pub fn from_context() -> Self {
        expect_context::<Self>()
    }

    /// Commit one change: signal + localStorage + the `<html>` attributes, in one breath.
    pub fn commit(self, next: Prefs) {
        apply_to_html(&next);
        storage_set(PREFS_KEY, &prefs::serialize(&next));
        self.prefs.set(next);
    }

    pub fn reset(self) {
        self.commit(prefs::DEFAULT_PREFS);
    }
}

fn apply_to_html(p: &Prefs) {
    let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    else {
        return;
    };
    let _ = root.set_attribute("data-reader-size", p.size);
    let _ = root.set_attribute("data-reader-leading", p.leading);
    let _ = root.set_attribute("data-reader-family", p.family);
    let _ = root.set_attribute("data-reader-width", p.width);
}

fn storage_get(key: &str) -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
}

fn storage_set(key: &str, value: &str) {
    if let Some(Ok(Some(storage))) = web_sys::window().map(|w| w.local_storage()) {
        let _ = storage.set_item(key, value);
    }
}
