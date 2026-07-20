//! The admin gate, extracted in step 49 from `submission/http/admin.rs` where it landed in
//! step 21. Two contexts need it now — the allowlist panel and the readership read — and the
//! invariant is worth stating in one place rather than twice:
//!
//! **ADMIN is CONFIG (`ADMIN_USERS`), never a token claim, and the server re-checks it on
//! EVERY call.** `MeDto.admin` exists so the client can hide a menu item; it is not what
//! authorises anything.

use std::collections::HashSet;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use synapse_shared::api::ApiError;

use crate::identity::http::{LiveIdentityService, optional_user};

pub type Reject = (StatusCode, Json<ApiError>);

/// Anonymous → 401; a verified non-admin → 403 "Admin only". Returns the caller's canonical
/// (lowercase) username.
///
/// `what` names the call in the audit line so the two callers stay distinguishable in the log —
/// before the extraction the message was hardcoded to "allowlist call", which would have been
/// quietly wrong the moment a second admin route existed.
/// Generic over the hasher because clippy's `implicit_hasher` fires on a free function taking
/// `&HashSet<String>` — a lint the previous shape hid, since the set was reached through
/// `&self.admin_users` rather than passed.
pub async fn require_admin<S: std::hash::BuildHasher + Sync>(
    identity: &LiveIdentityService,
    admin_users: &HashSet<String, S>,
    headers: &HeaderMap,
    what: &str,
) -> Result<String, Reject> {
    // The shared skeleton resolves the caller; only the POLICY (admin required) lives here.
    let Some(user) = optional_user(identity, headers).await? else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                error: "Missing bearer token".to_owned(),
                detail: Some("Admin calls require a signed-in admin".to_owned()),
                hint: None,
            }),
        ));
    };
    if admin_users.contains(&user.username) {
        tracing::info!(admin = user.username, what, "admin call");
        Ok(user.username)
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: "Admin only".to_owned(),
                detail: Some(format!("'{}' is not an admin on this deployment", user.username)),
                hint: None,
            }),
        ))
    }
}
