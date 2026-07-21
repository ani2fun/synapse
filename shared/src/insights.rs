//! The readership wire contract. Admin-only, and deliberately coarse: counts and a
//! timestamp, never a reader. There is no DTO here for recording a view because there is no
//! endpoint for it — a view is recorded as a side effect of serving the lesson, so the client
//! never asks for it and cannot inflate it.

use serde::{Deserialize, Serialize};

/// One lesson's readership. `lastViewed` is an ISO-8601 UTC string, matching the `publishedAt`
/// convention in `blog` — the wire carries strings, the caller parses if it cares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LessonViewDto {
    pub lesson_path: String,
    pub views: i64,
    pub authed_views: i64,
    pub last_viewed: String,
}
