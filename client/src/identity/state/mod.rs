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
        // The viz crate's trace runs carry the SAME bearer (its `/api/run` fetch lives
        // in-crate since A10, with its own seam).
        viz_wasm::api::set_token_provider(|| HANDLE.with_borrow(|h| h.as_ref().and_then(|h| h.token())));
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
        crate::log::warn("auth config unavailable — staying anonymous");
        return store.status.set(AuthStatus::Anonymous);
    };
    crate::log::debug(&format!(
        "auth config: realm '{}' at {} (client {})",
        config.realm, config.url, config.client_id
    ));
    let handle = match auth::boot(&config.url, &config.realm, &config.client_id).await {
        Ok(handle) => Rc::new(handle),
        Err(error) => {
            crate::log::warn(&format!("auth boot failed — staying anonymous ({error:?})"));
            return store.status.set(AuthStatus::Anonymous);
        }
    };
    HANDLE.with_borrow_mut(|h| *h = Some(Rc::clone(&handle)));

    if !handle.authenticated() {
        crate::log::debug("no Keycloak session — anonymous");
        return store.status.set(AuthStatus::Anonymous);
    }
    adopt(store).await;
    refresh_loop(store, handle).await;
}

/// The session is adopted only when OUR server verifies the token (`/api/me`).
async fn adopt(store: AuthStore) {
    match api::me().await {
        Ok(me) => {
            crate::log::info(&format!("signed in as '{}'", me.username));
            store.status.set(AuthStatus::Authed(me));
        }
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
            crate::log::warn("session refresh failed — signed out");
            store.status.set(AuthStatus::Anonymous);
            return;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ACCOUNT ACTIONS (oracle: `AccountStore` + `ActionStatus`, step 21)
// The destructive verbs, each driving one inline status banner. Deleting the
// account orchestrates erase → delete → sign-out ON THE CLIENT, so the server's
// identity context never depends on submissions (qna Q32).
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionStatus {
    Idle,
    Busy(String),
    Ok(String),
    Error(String),
}

/// The browser-side state the account owns (reading preferences; theme is a preference of the
/// DEVICE, not "my data" — the oracle excluded it too).
/// Reading progress joined this in step 51: a font size is a preference of the DEVICE,
/// but what someone has read is theirs, and "erase all my data" has to mean it.
const LOCAL_KEYS: [&str; 3] = ["reader-prefs", "reader-progress", "reader-last"];

fn clear_local() {
    if let Some(Ok(Some(storage))) = web_sys::window().map(|w| w.local_storage()) {
        for key in LOCAL_KEYS {
            let _ = storage.remove_item(key);
        }
    }
}

pub fn erase_submissions(status: RwSignal<ActionStatus>) {
    status.set(ActionStatus::Busy("Erasing your submissions…".into()));
    spawn_local(async move {
        match api::erase_submissions().await {
            Ok(result) => status.set(ActionStatus::Ok(format!(
                "Deleted {} submission(s).",
                result.deleted
            ))),
            Err(message) => status.set(ActionStatus::Error(message)),
        }
    });
}

/// Erase server data AND this browser's reading state, then reload.
pub fn erase_all_data(status: RwSignal<ActionStatus>) {
    status.set(ActionStatus::Busy("Erasing your data…".into()));
    spawn_local(async move {
        match api::erase_submissions().await {
            Ok(_) => {
                clear_local();
                if let Some(window) = web_sys::window() {
                    let _ = window.location().reload();
                }
            }
            Err(message) => status.set(ActionStatus::Error(message)),
        }
    });
}

/// Erase → delete → sign out; any failed leg stops the chain with its message.
pub fn delete_account(auth: AuthStore, status: RwSignal<ActionStatus>) {
    status.set(ActionStatus::Busy("Deleting your account…".into()));
    spawn_local(async move {
        if let Err(message) = api::erase_submissions().await {
            return status.set(ActionStatus::Error(message));
        }
        match api::delete_account().await {
            Ok(_) => {
                clear_local();
                auth.sign_out();
            }
            Err(message) => status.set(ActionStatus::Error(message)),
        }
    });
}
