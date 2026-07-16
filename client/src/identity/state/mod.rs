//! Auth state + the boot flow (oracle: `AuthStore` + `AuthBoot`). The signal starts `Loading`
//! (never a "Sign in" flash before check-sso answers), adopts the session by echoing
//! `GET /api/me`, and a 30 s loop refreshes the token when < 60 s remain — a failed refresh
//! degrades to `Anonymous`. The bearer flows into EVERY api call via the token provider seam
//! (identity → api; api stays feature-agnostic with an anonymous default).

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use synapse_shared::identity::MeDto;

use crate::api;
use crate::islands::auth::{self, AuthHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStatus {
    Loading,
    Anonymous,
    Authed(MeDto),
}

thread_local! {
    /// The live keycloak handle — session-scoped, owned here (JS object, `!Send`).
    static HANDLE: RefCell<Option<Rc<AuthHandle>>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy)]
pub struct AuthStore {
    pub status: RwSignal<AuthStatus>,
}

impl AuthStore {
    /// Created ONCE in `App`; installs the bearer seam and starts the boot flow.
    pub fn provide() {
        let store = Self {
            status: RwSignal::new(AuthStatus::Loading),
        };
        provide_context(store);
        api::set_token_provider(|| HANDLE.with_borrow(|h| h.as_ref().and_then(|h| h.token())));
        spawn_local(boot(store));
    }

    pub fn from_context() -> Self {
        expect_context::<Self>()
    }

    pub fn authed(self) -> bool {
        matches!(&*self.status.read(), AuthStatus::Authed(_))
    }

    pub fn sign_in(self) {
        HANDLE.with_borrow(|h| {
            if let Some(handle) = h {
                handle.login();
            }
        });
    }

    pub fn sign_out(self) {
        HANDLE.with_borrow(|h| {
            if let Some(handle) = h {
                let origin = web_sys::window()
                    .and_then(|w| w.location().origin().ok())
                    .unwrap_or_default();
                handle.logout(&origin);
            }
        });
        self.status.set(AuthStatus::Anonymous);
    }

    pub fn account_url(self) -> Option<String> {
        HANDLE.with_borrow(|h| h.as_ref().map(|h| h.account_url()))
    }
}

/// The oracle's `AuthBoot.start`: config → keycloak init (check-sso, PKCE S256) → adopt via
/// `/api/me` → the refresh loop. Every failure lands on `Anonymous`, never an error page.
async fn boot(store: AuthStore) {
    let Ok(config) = api::auth_config().await else {
        return store.status.set(AuthStatus::Anonymous);
    };
    let handle = match auth::boot(&config.url, &config.realm, &config.client_id).await {
        Ok(handle) => Rc::new(handle),
        Err(error) => {
            leptos::logging::log!("auth boot failed: {error:?}");
            return store.status.set(AuthStatus::Anonymous);
        }
    };
    HANDLE.with_borrow_mut(|h| *h = Some(Rc::clone(&handle)));

    if !handle.authenticated() {
        return store.status.set(AuthStatus::Anonymous);
    }
    adopt(store).await;
    refresh_loop(store, handle).await;
}

/// The session is adopted only when OUR server verifies the token (`/api/me`).
async fn adopt(store: AuthStore) {
    match api::me().await {
        Ok(me) => store.status.set(AuthStatus::Authed(me)),
        Err(_) => store.status.set(AuthStatus::Anonymous),
    }
}

/// Poll every 30 s, refreshing when < 60 s remain (the oracle uses polling `updateToken`, not
/// `onTokenExpired`). A failed refresh means the session is gone.
async fn refresh_loop(store: AuthStore, handle: Rc<AuthHandle>) {
    loop {
        gloo_timers::future::TimeoutFuture::new(30_000).await;
        if !matches!(store.status.get_untracked(), AuthStatus::Authed(_)) {
            return;
        }
        if handle.update_token(60).await.is_err() {
            store.status.set(AuthStatus::Anonymous);
            return;
        }
    }
}
