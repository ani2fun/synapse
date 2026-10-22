//! Listen ITs: the `/api/listen` routes through the REAL router state — a stub realm minting real
//! tokens, a wiremock speech engine answering with genuine MP3 frames, a temporary audio cache.
//! The anonymous half is ungated (it answers before the allowlist is asked, so a lazy pool is
//! never dialed); everything past the gate rides the gated Postgres pool (`POSTGRES_IT=1`), and
//! each test uses its own usernames so the suite is safe under default parallelism.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use synapse_server::config::AppConfig;
use synapse_server::identity::http::IdentityRoutesState;
use synapse_server::listen::http::{ListenRoutesState, routes, routes_state};
use synapse_server::listen::{AudioCache, KokoroEngine, PostgresListenAllowlist, Tickets};
use synapse_server::platform::admission::Admission;
use tokio::sync::Semaphore;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// One second of MPEG-2 Layer III at 24 kHz — the engine's own format: 125 frames × 24 ms.
fn one_second_mp3() -> Vec<u8> {
    let mut frame = vec![0_u8; 384];
    frame[..4].copy_from_slice(&[0xFF, 0xF3, 0xC4, 0xC4]);
    frame.repeat(125)
}

/// A speech engine answering every unit with `status` (and, on 200, one second of audio).
async fn engine(status: u16) -> MockServer {
    let server = MockServer::start().await;
    let response = if status == 200 {
        ResponseTemplate::new(200)
            .insert_header("content-type", "audio/mpeg")
            .set_body_bytes(one_second_mp3())
    } else {
        ResponseTemplate::new(status).set_body_string("engine exploded")
    };
    Mock::given(method("POST"))
        .and(path("/v1/audio/speech"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://nobody:nowhere@127.0.0.1:9/none")
        .unwrap()
}

/// The routes over `pool`, an engine at `engine_url`, a fresh cache, and `tester` as the admin.
fn app(
    issuer: &str,
    pool: sqlx::PgPool,
    engine_url: &str,
    streams: Admission,
) -> (Router, tempfile::TempDir) {
    let cache = tempfile::tempdir().unwrap();
    let state = ListenRoutesState {
        allowlist: Arc::new(PostgresListenAllowlist::new(pool)),
        identity: common::identity_for(issuer),
        admin_users: Arc::new(HashSet::from([common::username("tester")])),
        engine: Arc::new(KokoroEngine::new(engine_url)),
        cache: Arc::new(AudioCache::new(cache.path(), 16)),
        tickets: Arc::new(Tickets::new(Duration::from_mins(10))),
        streams: Arc::new(streams),
        synth_gate: Arc::new(Semaphore::new(1)),
    };
    (routes(state), cache)
}

fn request(verb: &str, uri: &str, token: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(verb).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Vec<u8>) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (
        status,
        axum::body::to_bytes(res.into_body(), 1 << 24)
            .await
            .unwrap()
            .to_vec(),
    )
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

/// Grant `who` through the admin route, as `tester`.
async fn grant(app: &Router, issuer: &str, who: &str) {
    let admin = common::mint(issuer, "tester");
    let (status, body) = send(
        app,
        request(
            "POST",
            "/api/admin/listeners",
            Some(&admin),
            Some(json!({ "username": who, "note": "it" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
}

/// Issue a ticket for `token`; returns the ticket's `.mp3` path.
async fn ticket(app: &Router, token: &str, units: &[&str]) -> String {
    let (status, body) = send(
        app,
        request(
            "POST",
            "/api/listen",
            Some(token),
            Some(json!({ "voice": "af_heart", "units": units })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    json_of(&body)["url"].as_str().unwrap().to_owned()
}

// ── UNGATED: answered before the allowlist is asked ──────────────────────────────────────────

#[tokio::test]
async fn an_anonymous_reader_is_told_no_and_refused() {
    let issuer = common::stub_realm().await;
    let (app, _cache) = app(&issuer, lazy_pool(), "http://127.0.0.1:9", Admission::new(2, 4));

    let (status, body) = send(&app, request("GET", "/api/listen/config", None, None)).await;
    assert_eq!(status, StatusCode::OK);
    let config = json_of(&body);
    assert_eq!(config["canListen"], false);
    assert_eq!(config["voices"][0]["id"], "af_heart");
    assert_eq!(config["maxUnitChars"], 400);

    let start = request(
        "POST",
        "/api/listen",
        None,
        Some(json!({ "voice": "af_heart", "units": ["Hi."] })),
    );
    assert_eq!(send(&app, start).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        send(&app, request("GET", "/api/listen/nope.mp3", None, None))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&app, request("GET", "/api/listen/nope.wav", None, None))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&app, request("GET", "/api/listen/nope/marks?from=0", None, None))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn the_admin_routes_are_for_admins_only() {
    let issuer = common::stub_realm().await;
    let (app, _cache) = app(&issuer, lazy_pool(), "http://127.0.0.1:9", Admission::new(2, 4));
    let stranger = common::mint(&issuer, "stranger");
    assert_eq!(
        send(&app, request("GET", "/api/admin/listeners", None, None))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &app,
            request("GET", "/api/admin/listeners", Some(&stranger), None)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let admin = common::mint(&issuer, "tester");
    let blank = request(
        "POST",
        "/api/admin/listeners",
        Some(&admin),
        Some(json!({ "username": "  " })),
    );
    assert_eq!(send(&app, blank).await.0, StatusCode::BAD_REQUEST);
    let nobody = request("DELETE", "/api/admin/listeners/%20", Some(&admin), None);
    assert_eq!(send(&app, nobody).await.0, StatusCode::NOT_FOUND);
}

#[test]
fn the_routes_mount_only_where_an_engine_is_configured() {
    let identity = IdentityRoutesState {
        identity: common::identity_for("http://127.0.0.1:9/realms/synapse"),
        issuer: "http://127.0.0.1:9/realms/synapse".to_owned(),
        audience: "synapse-web".to_owned(),
        admin_users: Arc::new(HashSet::new()),
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();
    let off = AppConfig::default();
    assert!(routes_state(&off, &identity, &lazy_pool()).is_none());
    let cache = tempfile::tempdir().unwrap();
    let on = AppConfig {
        tts_url: Some("http://127.0.0.1:8880".to_owned()),
        tts_cache: cache.path().display().to_string(),
        ..AppConfig::default()
    };
    assert!(routes_state(&on, &identity, &lazy_pool()).is_some());
}

// ── GATED: past the allowlist ─────────────────────────────────────────────────────────────────

/// The whole path a listed reader takes: config says yes, a ticket is issued, the stream is the
/// units' audio end to end, and the marks report each unit's length — then a revoke shuts it.
#[tokio::test]
async fn a_listed_reader_streams_a_lesson_and_reads_its_marks() {
    let Some(pool) = common::gated_pool().await else {
        return;
    };
    let issuer = common::stub_realm().await;
    let speech = engine(200).await;
    let (app, _cache) = app(&issuer, pool, &speech.uri(), Admission::new(2, 4));
    grant(&app, &issuer, "it-listen-reader").await;
    let reader = common::mint(&issuer, "it-listen-reader");

    let (_, body) = send(&app, request("GET", "/api/listen/config", Some(&reader), None)).await;
    assert_eq!(json_of(&body)["canListen"], true);

    let mp3 = ticket(&app, &reader, &["One.", "  Two  sentences. ", "\n"]).await;
    let res = app
        .clone()
        .oneshot(request("GET", &mp3, None, None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "audio/mpeg");
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    let audio = axum::body::to_bytes(res.into_body(), 1 << 24).await.unwrap();
    assert_eq!(
        audio.len(),
        one_second_mp3().len() * 2,
        "two units, the blank one dropped"
    );

    let marks_path = mp3.replace(".mp3", "/marks");
    let (status, body) = send(&app, request("GET", &format!("{marks_path}?from=0"), None, None)).await;
    assert_eq!(status, StatusCode::OK);
    let marks = json_of(&body);
    assert_eq!(marks["from"], 0);
    let durations: Vec<f64> = serde_json::from_value(marks["durations"].clone()).unwrap();
    assert_eq!(durations.len(), 2);
    assert!(durations.iter().all(|d| (d - 3.0).abs() < 1e-9));

    let admin = common::mint(&issuer, "tester");
    let (status, body) = send(&app, request("GET", "/api/admin/listeners", Some(&admin), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json_of(&body)
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["username"] == "it-listen-reader")
    );
    let revoke = request(
        "DELETE",
        "/api/admin/listeners/it-listen-reader",
        Some(&admin),
        None,
    );
    assert_eq!(send(&app, revoke).await.0, StatusCode::NO_CONTENT);
    let again = request(
        "DELETE",
        "/api/admin/listeners/it-listen-reader",
        Some(&admin),
        None,
    );
    assert_eq!(send(&app, again).await.0, StatusCode::NOT_FOUND);
    let refused = request(
        "POST",
        "/api/listen",
        Some(&reader),
        Some(json!({ "voice": "af_heart", "units": ["Hi."] })),
    );
    assert_eq!(send(&app, refused).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_listen_the_server_cannot_say_is_a_400() {
    let Some(pool) = common::gated_pool().await else {
        return;
    };
    let issuer = common::stub_realm().await;
    let (app, _cache) = app(&issuer, pool, "http://127.0.0.1:9", Admission::new(2, 4));
    grant(&app, &issuer, "it-listen-validate").await;
    let reader = common::mint(&issuer, "it-listen-validate");
    for body in [
        json!({ "voice": "af_nicole", "units": ["Hi."] }),
        json!({ "voice": "af_heart", "units": [" "] }),
        json!({ "voice": "af_heart", "units": ["x".repeat(401)] }),
    ] {
        let (status, reply) = send(&app, request("POST", "/api/listen", Some(&reader), Some(body))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json_of(&reply)["error"], "Cannot read that aloud");
    }
}

#[tokio::test]
async fn an_engine_that_fails_on_the_first_unit_is_a_502_with_a_reason() {
    let Some(pool) = common::gated_pool().await else {
        return;
    };
    let issuer = common::stub_realm().await;
    let speech = engine(500).await;
    let (app, _cache) = app(&issuer, pool, &speech.uri(), Admission::new(2, 4));
    grant(&app, &issuer, "it-listen-engine").await;
    let reader = common::mint(&issuer, "it-listen-engine");
    let mp3 = ticket(&app, &reader, &["One."]).await;
    let (status, body) = send(&app, request("GET", &mp3, None, None)).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        json_of(&body)["detail"]
            .as_str()
            .unwrap()
            .contains("engine exploded")
    );

    // And an engine that is not there at all.
    let (app, _cache) = app_unreachable(&issuer).await;
    let mp3 = ticket(&app, &reader, &["One."]).await;
    assert_eq!(
        send(&app, request("GET", &mp3, None, None)).await.0,
        StatusCode::BAD_GATEWAY
    );
}

async fn app_unreachable(issuer: &str) -> (Router, tempfile::TempDir) {
    let pool = common::gated_pool().await.unwrap();
    app(issuer, pool, "http://127.0.0.1:9", Admission::new(2, 4))
}

/// A reader already holding their streams is refused a third at once, not queued.
#[tokio::test]
async fn a_reader_at_the_stream_limit_is_refused() {
    let Some(pool) = common::gated_pool().await else {
        return;
    };
    let issuer = common::stub_realm().await;
    let speech = engine(200).await;
    let (app, _cache) = app(&issuer, pool, &speech.uri(), Admission::new(1, 4));
    grant(&app, &issuer, "it-listen-busy").await;
    let reader = common::mint(&issuer, "it-listen-busy");
    let mp3 = ticket(&app, &reader, &["One.", "Two."]).await;
    // The first stream's body is held, not drained, so its place stays taken.
    let held = app
        .clone()
        .oneshot(request("GET", &mp3, None, None))
        .await
        .unwrap();
    assert_eq!(held.status(), StatusCode::OK);
    let (status, body) = send(&app, request("GET", &mp3, None, None)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json_of(&body)["error"], "Already listening");
    drop(held);
}
