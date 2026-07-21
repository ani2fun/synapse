//! `GET /api/admin/lesson-views` — the read half of readership. Gated per call by
//! the shared `require_admin`, and generic over the store port so the route tests drive a fake
//! through the REAL router (the `admin_allowlist_it.rs` pattern: implement the port for
//! `&'static Fake`).

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use chrono::SecondsFormat;
use serde::Deserialize;
use synapse_shared::api::ApiError;
use synapse_shared::insights::LessonViewDto;

use crate::identity::http::LiveIdentityService;
use crate::insights::{LessonViewCount, LessonViewStore};
use crate::platform::admin_gate::{Reject, require_admin};

/// Bounded so a caller cannot ask for the whole table; the default is a screenful.
const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 500;

pub struct InsightsRoutesState<V> {
    pub views: Arc<V>,
    pub identity: Arc<LiveIdentityService>,
    pub admin_users: Arc<HashSet<String>>,
}

/// Hand-written: `#[derive(Clone)]` would demand `V: Clone`, which the port does not promise.
impl<V> Clone for InsightsRoutesState<V> {
    fn clone(&self) -> Self {
        Self {
            views: Arc::clone(&self.views),
            identity: Arc::clone(&self.identity),
            admin_users: Arc::clone(&self.admin_users),
        }
    }
}

pub fn routes<V: LessonViewStore + 'static>(state: InsightsRoutesState<V>) -> Router {
    Router::new()
        .route("/api/admin/lesson-views", get(list_lesson_views::<V>))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct LimitQuery {
    limit: Option<i64>,
}

fn to_dto(count: &LessonViewCount) -> LessonViewDto {
    LessonViewDto {
        lesson_path: count.lesson_path.clone(),
        views: count.views,
        authed_views: count.authed_views,
        last_viewed: count.last_viewed.to_rfc3339_opts(SecondsFormat::Secs, true),
    }
}

/// Most-read lessons first. What is NOT here is as important as what is: no reader, no session,
/// no IP — the store cannot answer "who read this" because it never recorded it.
#[utoipa::path(
    get,
    path = "/api/admin/lesson-views",
    operation_id = "listLessonViews",
    params(("limit" = Option<i64>, Query, description = "Rows to return (default 50, max 500)")),
    responses(
        (status = 200, description = "Most-read lessons first", body = [LessonViewDto]),
        (status = 401, description = "Not signed in", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError),
        (status = 500, description = "Store failed", body = ApiError)
    )
)]
pub async fn list_lesson_views<V: LessonViewStore>(
    State(state): State<InsightsRoutesState<V>>,
    headers: HeaderMap,
    Query(query): Query<LimitQuery>,
) -> Result<Json<Vec<LessonViewDto>>, Reject> {
    require_admin(&state.identity, &state.admin_users, &headers, "lesson-views").await?;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    match state.views.top(limit).await {
        Ok(counts) => Ok(Json(counts.iter().map(to_dto).collect())),
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: "Readership unavailable".to_owned(),
                detail: Some(error.to_string()),
                hint: None,
            }),
        )),
    }
}
