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
            spawn_local(async move {
                match api::index().await {
                    Ok(idx) => self.index.set(AsyncResult::Loaded(idx)),
                    Err(message) => {
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
    spawn_local(async move {
        match api::lesson(&path).await {
            Ok(payload) => state.set(AsyncResult::Loaded(payload)),
            Err(message) => state.set(AsyncResult::Failed(message)),
        }
    });
    state
}
