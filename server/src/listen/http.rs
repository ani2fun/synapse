//! The listen HTTP surface, mounted only where a speech engine is configured:
//!
//! - `GET /api/listen/config` — answered for everyone; `canListen` folds "signed in" and "on the
//!   listen allowlist" into one bit, so the page has nothing to re-derive.
//! - `POST /api/listen` — bearer REQUIRED and allowlisted; validates the units and issues a ticket.
//! - `GET /api/listen/{ticket}.mp3` — no header needed (an `<audio>` element sends none); streams
//!   the narration as one MP3. The FIRST unit is produced before the response starts, so an engine
//!   that is down answers 502 with a reason instead of a 200 that plays nothing.
//! - `/api/admin/listeners` — list · grant · revoke, through the shared admin gate.

use std::collections::HashSet;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use synapse_shared::api::ApiError;
use synapse_shared::listen::{
    LISTEN_MAX_REQUEST_CHARS, LISTEN_MAX_UNIT_CHARS, LISTEN_VOICES, ListenConfigDto, ListenMarksDto,
    ListenRequestDto, ListenTicketDto, ListenVoiceDto,
};
use synapse_shared::submission::{AllowlistEntryDto, GrantRequestDto};
use tokio::sync::Semaphore;

use crate::config::AppConfig;
use crate::identity::domain::Username;
use crate::identity::http::{IdentityRoutesState, LiveIdentityService, optional_user};
use crate::listen::narrate::Narrator;
use crate::listen::{
    AudioCache, KokoroEngine, ListenError, Narration, PostgresListenAllowlist, Tickets, spawn_sweeper,
};
use crate::platform::admin_gate::require_admin;
use crate::platform::admission::{Admission, Refused};

pub type Reject = (StatusCode, Json<ApiError>);

/// A page of marks: enough for minutes of audio, few enough that one poll is a handful of reads.
const MARKS_PAGE: usize = 64;

/// How long a ticket lives after its last use.
const TICKET_TTL: std::time::Duration = std::time::Duration::from_mins(10);

#[derive(Clone)]
pub struct ListenRoutesState {
    pub allowlist: Arc<PostgresListenAllowlist>,
    pub identity: Arc<LiveIdentityService>,
    pub admin_users: Arc<HashSet<Username>>,
    pub engine: Arc<KokoroEngine>,
    pub cache: Arc<AudioCache>,
    pub tickets: Arc<Tickets>,
    /// Streams in flight — per reader and in total. Synthesis is serialised separately
    /// (`synth_gate`); this bounds how many connections can wait on it.
    pub streams: Arc<Admission>,
    pub synth_gate: Arc<Semaphore>,
}

impl ListenRoutesState {
    fn narrator(&self) -> Narrator<KokoroEngine> {
        Narrator {
            engine: Arc::clone(&self.engine),
            cache: Arc::clone(&self.cache),
            synth_gate: Arc::clone(&self.synth_gate),
        }
    }
}

/// The routes-state, or `None` when no speech engine is configured — which leaves every
/// `/api/listen` route and its admin allowlist unmounted, so the lesson page offers nothing.
///
/// Two separate bounds, because they answer different questions. `streams` caps CONNECTIONS:
/// two per reader (a browser often opens a media URL twice — a probe, then the real fetch) and
/// four in total. `synth_gate` caps the ENGINE at one synthesis: measured on wk-1, one Kokoro
/// instance runs at ~2× realtime on two cores, which keeps ahead of exactly one listener.
pub fn routes_state(
    cfg: &AppConfig,
    identity: &IdentityRoutesState,
    pool: &sqlx::PgPool,
) -> Option<ListenRoutesState> {
    let Some(url) = cfg.tts_url() else {
        tracing::info!("listen: no SYNAPSE_TTS_URL — /api/listen is not mounted");
        return None;
    };
    let cache = Arc::new(AudioCache::new(&cfg.tts_cache, cfg.tts_cache_mib));
    spawn_sweeper(Arc::clone(&cache));
    tracing::info!(
        url,
        cache = cfg.tts_cache,
        cap_mib = cfg.tts_cache_mib,
        "listen: speech engine configured"
    );
    Some(ListenRoutesState {
        allowlist: Arc::new(PostgresListenAllowlist::new(pool.clone())),
        identity: Arc::clone(&identity.identity),
        admin_users: Arc::clone(&identity.admin_users),
        engine: Arc::new(KokoroEngine::new(url)),
        cache,
        tickets: Arc::new(Tickets::new(TICKET_TTL)),
        streams: Arc::new(Admission::new(2, 4)),
        synth_gate: Arc::new(Semaphore::new(1)),
    })
}

pub fn routes(state: ListenRoutesState) -> Router {
    Router::new()
        .route("/api/listen/config", get(get_config))
        .route("/api/listen", post(start_listen))
        .route("/api/listen/{file}", get(play))
        .route("/api/listen/{ticket}/marks", get(marks))
        .route("/api/admin/listeners", get(list_listeners).post(grant_listener))
        .route("/api/admin/listeners/{username}", delete(revoke_listener))
        .with_state(state)
}

fn reject(status: StatusCode, error: &str, detail: Option<String>, hint: Option<&str>) -> Reject {
    (
        status,
        Json(ApiError {
            error: error.to_owned(),
            detail,
            hint: hint.map(str::to_owned),
        }),
    )
}

/// The context error flattens HERE and only here (RS001: erase at the EDGE).
fn to_error(error: &ListenError) -> Reject {
    let detail = Some(error.to_string());
    match error {
        ListenError::NotAllowed(_) => reject(
            StatusCode::FORBIDDEN,
            "Listening is not enabled for you",
            detail,
            Some("Ask an admin to add you to the listen list"),
        ),
        ListenError::UnknownVoice(_)
        | ListenError::Empty
        | ListenError::UnitTooLong { .. }
        | ListenError::TooLong { .. } => {
            reject(StatusCode::BAD_REQUEST, "Cannot read that aloud", detail, None)
        }
        ListenError::EngineFailed(_) => reject(
            StatusCode::BAD_GATEWAY,
            "The speech engine is unavailable",
            detail,
            Some("Try again in a minute"),
        ),
        ListenError::StoreFailed(_) => reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Listening unavailable",
            detail,
            None,
        ),
    }
}

/// The voices on offer, in the order the picker shows them.
fn voices() -> Vec<ListenVoiceDto> {
    LISTEN_VOICES
        .iter()
        .map(|(id, label)| ListenVoiceDto {
            id: (*id).to_owned(),
            label: (*label).to_owned(),
        })
        .collect()
}

/// Whether listening is on for THIS caller. Answers for everyone — an anonymous reader gets
/// `canListen: false`, not a 401, because the lesson page asks before it knows who is reading.
#[utoipa::path(
    get,
    path = "/api/listen/config",
    operation_id = "getListenConfig",
    responses(
        (status = 200, description = "Listening coordinates for this caller", body = ListenConfigDto),
        (status = 500, description = "The allowlist store failed", body = ApiError)
    )
)]
pub(crate) async fn get_config(
    State(state): State<ListenRoutesState>,
    headers: HeaderMap,
) -> Result<Json<ListenConfigDto>, Reject> {
    let can_listen = match optional_user(&state.identity, &headers).await? {
        Some(user) => state
            .allowlist
            .is_allowed(&user.username)
            .await
            .map_err(|e| to_error(&e))?,
        None => false,
    };
    Ok(Json(ListenConfigDto {
        can_listen,
        voices: voices(),
        max_unit_chars: LISTEN_MAX_UNIT_CHARS,
        max_request_chars: LISTEN_MAX_REQUEST_CHARS,
    }))
}

/// Validate the units and issue a ticket the `<audio>` element can play.
#[utoipa::path(
    post,
    path = "/api/listen",
    operation_id = "startListen",
    request_body = ListenRequestDto,
    responses(
        (status = 200, description = "The address to play", body = ListenTicketDto),
        (status = 400, description = "Unknown voice, nothing to read, or too long", body = ApiError),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not on the listen allowlist", body = ApiError),
        (status = 500, description = "The allowlist store failed", body = ApiError)
    )
)]
pub(crate) async fn start_listen(
    State(state): State<ListenRoutesState>,
    headers: HeaderMap,
    Json(request): Json<ListenRequestDto>,
) -> Result<Json<ListenTicketDto>, Reject> {
    let Some(user) = optional_user(&state.identity, &headers).await? else {
        return Err(reject(
            StatusCode::UNAUTHORIZED,
            "Listening requires a bearer token",
            Some("Sign in first".to_owned()),
            None,
        ));
    };
    if !state
        .allowlist
        .is_allowed(&user.username)
        .await
        .map_err(|e| to_error(&e))?
    {
        return Err(to_error(&ListenError::NotAllowed(user.username.into_string())));
    }
    let narration = Narration::prepare(&request.voice, &request.units).map_err(|e| to_error(&e))?;
    let units = narration.units.len();
    let voice = narration.voice.as_str();
    let ticket = state.tickets.issue(user.username.clone(), narration);
    tracing::info!(user = %user.username, units, voice, "POST /api/listen: ticket issued");
    Ok(Json(ListenTicketDto {
        url: format!("/api/listen/{ticket}.mp3"),
        units,
        expires_in: state.tickets.ttl().as_secs(),
    }))
}

/// The narration as one MP3 stream. Not in the OpenAPI document: its body is audio, not JSON,
/// and the address is only ever handed out by `startListen`.
pub(crate) async fn play(State(state): State<ListenRoutesState>, Path(file): Path<String>) -> Response {
    let Some(id) = file.strip_suffix(".mp3") else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some((owner, narration)) = state.tickets.redeem(id) else {
        return reject(
            StatusCode::NOT_FOUND,
            "This listen has expired",
            None,
            Some("Press Listen again"),
        )
        .into_response();
    };
    let hold = match state.streams.admit(owner.as_str()) {
        Ok(hold) => hold,
        Err(Refused::CallerBusy { limit }) => {
            return reject(
                StatusCode::TOO_MANY_REQUESTS,
                "Already listening",
                Some(format!("At most {limit} stream(s) at once")),
                Some("Stop the other one first"),
            )
            .into_response();
        }
        Err(Refused::SandboxBusy) => {
            return reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "The speech engine is busy",
                None,
                Some("Try again in a minute"),
            )
            .into_response();
        }
    };
    let narrator = state.narrator();
    let first = match narrator.unit(&narration, 0).await {
        Ok(audio) => audio,
        Err(error) => {
            tracing::warn!(%error, user = %owner, "listen: first unit failed");
            return to_error(&error).into_response();
        }
    };
    tracing::info!(user = %owner, units = narration.units.len(), "GET /api/listen: streaming");
    let head = futures_util::stream::once(async move { Ok::<_, std::io::Error>(first) });
    let rest = narrator.stream(narration, 1, hold);
    let body = Body::from_stream(futures_util::StreamExt::chain(head, rest));
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("audio/mpeg")),
            // A ticket is one reader's and short-lived: nothing between here and the browser
            // should keep it.
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        body,
    )
        .into_response()
}

#[derive(serde::Deserialize)]
pub(crate) struct MarksQuery {
    #[serde(default)]
    from: usize,
}

/// How long each synthesised unit plays, so the page can highlight what is being said. A cache
/// read per unit and never the engine: a unit not yet said ends the list, and the page asks again.
#[utoipa::path(
    get,
    path = "/api/listen/{ticket}/marks",
    operation_id = "listenMarks",
    params(
        ("ticket" = String, Path, description = "The ticket from startListen"),
        ("from" = Option<usize>, Query, description = "The first unit to report (default 0)")
    ),
    responses(
        (status = 200, description = "Durations of the units synthesised so far", body = ListenMarksDto),
        (status = 404, description = "The ticket lapsed", body = ApiError)
    )
)]
pub(crate) async fn marks(
    State(state): State<ListenRoutesState>,
    Path(ticket): Path<String>,
    Query(query): Query<MarksQuery>,
) -> Result<Json<ListenMarksDto>, Reject> {
    let Some((_, narration)) = state.tickets.redeem(&ticket) else {
        return Err(reject(
            StatusCode::NOT_FOUND,
            "This listen has expired",
            None,
            None,
        ));
    };
    let durations = state
        .narrator()
        .durations(&narration, query.from, MARKS_PAGE)
        .await;
    Ok(Json(ListenMarksDto {
        from: query.from,
        durations,
    }))
}

// ── ADMIN ─────────────────────────────────────────────────────────────────────────────────────

async fn gate(state: &ListenRoutesState, headers: &HeaderMap) -> Result<Username, Reject> {
    require_admin(&state.identity, &state.admin_users, headers, "listeners").await
}

/// The grants, newest first.
#[utoipa::path(
    get,
    path = "/api/admin/listeners",
    operation_id = "listListeners",
    responses(
        (status = 200, description = "Every grant, newest first", body = [AllowlistEntryDto]),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn list_listeners(
    State(state): State<ListenRoutesState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AllowlistEntryDto>>, Reject> {
    gate(&state, &headers).await?;
    state.allowlist.list().await.map(Json).map_err(|e| to_error(&e))
}

/// Grant (upsert) — the stored row comes back; usernames are canonicalised here.
#[utoipa::path(
    post,
    path = "/api/admin/listeners",
    operation_id = "grantListener",
    request_body = GrantRequestDto,
    responses(
        (status = 200, description = "The stored grant", body = AllowlistEntryDto),
        (status = 400, description = "Blank username", body = ApiError),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError)
    )
)]
pub(crate) async fn grant_listener(
    State(state): State<ListenRoutesState>,
    headers: HeaderMap,
    Json(request): Json<GrantRequestDto>,
) -> Result<Json<AllowlistEntryDto>, Reject> {
    gate(&state, &headers).await?;
    // Built through the verifier's own constructor, so the row is stored under exactly the name
    // the granted person will arrive with.
    let Some(username) = Username::parse(&request.username) else {
        return Err(reject(
            StatusCode::BAD_REQUEST,
            "Username required",
            Some("A grant needs a non-blank username".to_owned()),
            None,
        ));
    };
    let note = request.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    state
        .allowlist
        .grant(&username, note)
        .await
        .map(Json)
        .map_err(|e| to_error(&e))
}

/// Revoke — 204 on removal, 404 when the grant never existed. A stream already playing finishes;
/// the next `POST /api/listen` is refused.
#[utoipa::path(
    delete,
    path = "/api/admin/listeners/{username}",
    operation_id = "revokeListener",
    params(("username" = String, Path, description = "The granted username")),
    responses(
        (status = 204, description = "Revoked"),
        (status = 401, description = "Anonymous", body = ApiError),
        (status = 403, description = "Not an admin", body = ApiError),
        (status = 404, description = "No such grant", body = ApiError)
    )
)]
pub(crate) async fn revoke_listener(
    State(state): State<ListenRoutesState>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Result<StatusCode, Reject> {
    gate(&state, &headers).await?;
    let no_such_grant = |name: String| reject(StatusCode::NOT_FOUND, "No such grant", Some(name), None);
    let Some(username) = Username::parse(&raw) else {
        return Err(no_such_grant(raw));
    };
    match state.allowlist.revoke(&username).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(no_such_grant(username.into_string())),
        Err(error) => Err(to_error(&error)),
    }
}
