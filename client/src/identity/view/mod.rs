//! The header account chip (oracle: `AccountChip`, step-17 scope): `Loading` renders a QUIET
//! placeholder (no "Sign in" flash before check-sso answers); `Anonymous` offers sign-in;
//! `Authed` shows @username with manage-account + sign-out. The admin entry joins with the
//! admin step.

use leptos::prelude::*;

use crate::identity::state::{AuthStatus, AuthStore};

#[component]
pub fn AccountChip() -> impl IntoView {
    let store = AuthStore::from_context();
    let open = RwSignal::new(false);
    view! {
        <span class="account-chip">
            {move || match store.status.get() {
                AuthStatus::Loading => view! { <span class="account-chip__quiet">"…"</span> }.into_any(),
                AuthStatus::Anonymous => view! {
                    <button class="account-chip__signin" on:click=move |_| store.sign_in()>
                        "Sign in"
                    </button>
                }
                .into_any(),
                AuthStatus::Authed(me) => {
                    let username = format!("@{}", me.username);
                    view! {
                        <span class="account-chip__menu-wrap">
                            <button
                                class="account-chip__user"
                                on:click=move |_| open.update(|o| *o = !*o)
                            >
                                {username}
                            </button>
                            {move || {
                                open.get()
                                    .then(|| {
                                        let account = store.account_url();
                                        view! {
                                            <span class="account-chip__menu">
                                                {account.map(|url| view! {
                                                    <a class="account-chip__item" href=url target="_blank">
                                                        "Manage account"
                                                    </a>
                                                })}
                                                <button
                                                    class="account-chip__item"
                                                    on:click=move |_| store.sign_out()
                                                >
                                                    "Sign out"
                                                </button>
                                            </span>
                                        }
                                    })
                            }}
                        </span>
                    }
                    .into_any()
                }
            }}
        </span>
    }
}
