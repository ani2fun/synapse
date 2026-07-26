//! `GET /api/c4/sources` — which repositories the merged LikeC4 workspace must pull from.
//!
//! The `/c4` viewer is ONE workspace built from every `.c4` in the corpus, with exactly one
//! `specification {}`. That was a single repository's `docker build`; with books in their own
//! repositories it has to gather them. The build is a CI job with no token and no database, so it
//! asks the running app instead — which is also what makes a newly-registered repository's
//! diagrams appear without a second pull request.
//!
//! Unauthenticated on purpose. Every registered source is a public repository, the answer is
//! `owner/name` and a branch, and requiring a credential here would mean minting one for CI to
//! learn something anyone can read off GitHub.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::catalog::C4SourceDto;

use crate::catalog::application::ContentSources;
use crate::catalog::http::admin::ContentSourceRoutesState;

pub fn routes<S: ContentSources + 'static>(state: ContentSourceRoutesState<S>) -> Router {
    Router::new()
        .route("/api/c4/sources", get(list_c4_sources::<S>))
        .with_state(state)
}

/// Enabled sources only, in mount order. A disabled repository is one an admin has taken out of
/// the library, and its diagrams should leave the workspace with it.
#[utoipa::path(
    get,
    path = "/api/c4/sources",
    operation_id = "listC4Sources",
    responses(
        (status = 200, description = "Repositories to gather .c4 files from", body = [C4SourceDto])
    )
)]
pub(crate) async fn list_c4_sources<S: ContentSources>(
    State(state): State<ContentSourceRoutesState<S>>,
) -> Result<Json<Vec<C4SourceDto>>, (axum::http::StatusCode, Json<ApiError>)> {
    match state.sources.list().await {
        Ok(records) => Ok(Json(
            records
                .into_iter()
                .filter(|record| record.enabled)
                .map(|record| C4SourceDto {
                    repo: record.repo,
                    branch: record.branch,
                })
                .collect(),
        )),
        Err(error) => {
            tracing::error!(%error, "c4 sources: the registry is unavailable");
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: "Content source store unavailable".to_owned(),
                    detail: None,
                    hint: None,
                }),
            ))
        }
    }
}
