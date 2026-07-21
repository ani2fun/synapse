//! The blog HTTP surface (`BlogRoutes` + `BlogDtos`): the flat listing and one post by
//! slug. DTO↔domain mapping lives ONLY here; `publishedAt` crosses the wire as an ISO string,
//! empty when the post is undated.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::blog::{BlogPostDto, BlogSummaryDto};

use crate::blog::application::{BlogError, BlogPostView, BlogService};
use crate::blog::domain::BlogSummary;
use crate::blog::infrastructure::FileSystemBlogRepository;

pub type LiveBlogService = BlogService<FileSystemBlogRepository>;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

pub fn routes(service: Arc<LiveBlogService>) -> Router {
    Router::new()
        .route("/api/blog", get(list_posts))
        .route("/api/blog/{slug}", get(get_post))
        .with_state(service)
}

fn date(d: Option<chrono::NaiveDate>) -> String {
    d.map(|d| d.to_string()).unwrap_or_default()
}

fn to_summary(s: &BlogSummary) -> BlogSummaryDto {
    BlogSummaryDto {
        slug: s.slug.clone(),
        title: s.title.clone(),
        summary: s.summary.clone(),
        published_at: date(s.published_at),
        tags: s.tags.clone(),
        read_minutes: s.read_minutes,
        eyebrow: s.eyebrow.clone(),
    }
}

fn to_post(view: &BlogPostView) -> BlogPostDto {
    let p = &view.post;
    BlogPostDto {
        slug: p.slug.clone(),
        title: p.title.clone(),
        summary: p.summary.clone(),
        published_at: date(p.published_at),
        tags: p.tags.clone(),
        read_minutes: p.read_minutes,
        eyebrow: p.eyebrow.clone(),
        body: p.body.clone(),
        prev: view.prev.clone(),
        next: view.next.clone(),
    }
}

fn to_error(error: &BlogError) -> (StatusCode, Json<ApiError>) {
    let (status, message, detail) = match error {
        BlogError::NotFound(slug) => (StatusCode::NOT_FOUND, "No such post", slug.clone()),
        BlogError::Io(detail) => (StatusCode::INTERNAL_SERVER_ERROR, "Blog IO error", detail.clone()),
    };
    (
        status,
        Json(ApiError {
            error: message.to_owned(),
            detail: Some(detail),
            hint: None,
        }),
    )
}

/// The listing, newest first (undated last).
#[utoipa::path(
    get,
    path = "/api/blog",
    operation_id = "listBlogPosts",
    responses((status = 200, description = "Every published post, newest first", body = [BlogSummaryDto]))
)]
pub(crate) async fn list_posts(
    State(service): State<Arc<LiveBlogService>>,
) -> ApiResult<Vec<BlogSummaryDto>> {
    tracing::debug!("GET /api/blog");
    match service.list().await {
        Ok(listing) => Ok(Json(listing.iter().map(to_summary).collect())),
        Err(error) => Err(to_error(&error)),
    }
}

/// One post with body + publish-order neighbours.
#[utoipa::path(
    get,
    path = "/api/blog/{slug}",
    operation_id = "getBlogPost",
    params(("slug" = String, Path, description = "The post's slug")),
    responses(
        (status = 200, description = "The post", body = BlogPostDto),
        (status = 404, description = "Unknown slug", body = ApiError)
    )
)]
pub(crate) async fn get_post(
    State(service): State<Arc<LiveBlogService>>,
    Path(slug): Path<String>,
) -> ApiResult<BlogPostDto> {
    tracing::debug!(slug, "GET /api/blog/{{slug}}");
    match service.post(&slug).await {
        Ok(view) => Ok(Json(to_post(&view))),
        Err(error) => Err(to_error(&error)),
    }
}
