//! Integration: the JWKS verifier + identity routes against a LOCAL JWKS stub (oracle:
//! `JwksTokenVerifierSpec` + `IdentityRoutesSpec`). Tokens are minted in-test with a committed
//! TEST-ONLY RSA key (`tests/fixtures/test-only-rsa.pem` — generated for this suite, never a
//! secret); the stub serves its public JWK exactly where a realm would.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Value, json};
use tower::ServiceExt;

const TEST_PEM: &str = include_str!("fixtures/test-only-rsa.pem");
const KID: &str = "synapse-test";
const AUDIENCE: &str = "synapse-web";
const N_B64: &str = "zhViOX4PnOD51OW9MWknnaOwPKP1lodDI-BX4tk4Ulq6yj816CV89b9F-TXuUkHEXToXrheogf8gAIuYpx1PJD-e2spf9mIbKqmMFTSHZv36GIWsZ-afRr9vhSFhRkf8Jix9Yoo8au9JnbhkkkexXWg_j-w-ct5jTXwBBq-Sy72ijxKZ3Hrv0IkKIdYbwbVY57FLd7GM_cJOioCsqZuuw3HscaP33CUIpuXWam-q5tejXFlR7ldo9qrpuuPfcJUwh9Jgz4UA79asREpyyKkOv7IczvXODWYtSQYRK6bLgpuiIvwiDQ8M2K02OH-dYtIJ2euWYH6h2VNqabcZ36zDFw";

/// A realm lookalike serving the JWKS at the OIDC certs path. Returns the issuer URL.
async fn stub_realm() -> String {
    let jwks = json!({
        "keys": [{ "kty": "RSA", "alg": "RS256", "use": "sig", "kid": KID, "n": N_B64, "e": "AQAB" }]
    })
    .to_string();
    let app = Router::new().route(
        "/realms/synapse/protocol/openid-connect/certs",
        get(move || {
            let jwks = jwks.clone();
            async move { ([(header::CONTENT_TYPE, "application/json")], jwks) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}/realms/synapse", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    issuer
}

fn now() -> i64 {
    i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()).unwrap()
}

/// Mint a token with the test key; `overrides` merge over sane defaults.
fn mint(issuer: &str, overrides: Value) -> String {
    let mut claims = json!({
        "sub": "9f1c1d2e-1111-2222-3333-444455556666",
        "iss": issuer,
        "exp": now() + 300,
        "aud": "account",
        "azp": AUDIENCE,
        "preferred_username": "TeStEr",
        "email": "tester@synapse.local",
    });
    if let (Some(base), Value::Object(extra)) = (claims.as_object_mut(), overrides) {
        for (k, v) in extra {
            if v.is_null() {
                base.remove(&k);
            } else {
                base.insert(k, v);
            }
        }
    }
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(KID.to_owned());
    let key = EncodingKey::from_rsa_pem(TEST_PEM.as_bytes()).unwrap();
    jsonwebtoken::encode(&header, &claims, &key).unwrap()
}

async fn me(issuer: &str, bearer: Option<&str>) -> (StatusCode, Value) {
    let tmp = tempfile::tempdir().unwrap();
    let app = common::app_with_issuer(tmp.path(), "http://127.0.0.1:9", None, issuer);
    let mut builder = Request::builder().uri("/api/me");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let res = app.oneshot(builder.body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

// ── the verifier through /api/me ─────────────────────────────────────────────

#[tokio::test]
async fn a_public_spa_token_verifies_via_the_azp_branch_and_lowercases_the_username() {
    let issuer = stub_realm().await;
    // aud is "account" (the Keycloak default) — only azp names our client.
    let (status, body) = me(&issuer, Some(&mint(&issuer, json!({})))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["username"], "tester", "canonical LOWERCASE (step-36 fix)");
    assert_eq!(body["email"], "tester@synapse.local");
    assert_eq!(body["admin"], false);
}

#[tokio::test]
async fn the_aud_branch_and_the_sub_fallback_also_verify() {
    let issuer = stub_realm().await;
    let token = mint(
        &issuer,
        json!({ "aud": [AUDIENCE], "azp": null, "preferred_username": null, "email": null }),
    );
    let (status, body) = me(&issuer, Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["username"], "9f1c1d2e-1111-2222-3333-444455556666",
        "falls back to sub"
    );
    assert_eq!(body.get("email").cloned().unwrap_or(Value::Null), Value::Null);
}

#[tokio::test]
async fn expired_wrong_issuer_and_foreign_audience_tokens_are_401() {
    let issuer = stub_realm().await;
    for (label, overrides) in [
        ("expired", json!({ "exp": now() - 120 })),
        (
            "wrong issuer",
            json!({ "iss": "http://evil.example/realms/synapse" }),
        ),
        (
            "neither aud nor azp",
            json!({ "aud": "account", "azp": "evil-app" }),
        ),
    ] {
        let (status, body) = me(&issuer, Some(&mint(&issuer, overrides))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{label}: {body}");
        assert_eq!(body["error"], "Invalid bearer token", "{label}");
    }
}

#[tokio::test]
async fn garbage_and_missing_tokens_are_401() {
    let issuer = stub_realm().await;
    let (status, body) = me(&issuer, Some("not-a-jwt")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Invalid bearer token");

    let (status, body) = me(&issuer, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing bearer token");
}

#[tokio::test]
async fn an_unreachable_realm_is_503_never_401() {
    // Port 9 refuses — IdP-down is OUR problem, not the caller's.
    let issuer = "http://127.0.0.1:9/realms/synapse";
    let (status, body) = me(issuer, Some(&mint(issuer, json!({})))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["error"], "Token verifier unavailable");
}

// ── auth config ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn auth_config_splits_the_issuer_into_keycloak_coordinates() {
    let tmp = tempfile::tempdir().unwrap();
    let app = common::app_with_issuer(
        tmp.path(),
        "http://127.0.0.1:9",
        None,
        "http://localhost:8181/realms/synapse",
    );
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(
        body,
        json!({ "url": "http://localhost:8181", "realm": "synapse", "clientId": "synapse-web" })
    );
}

#[tokio::test]
async fn a_non_keycloak_issuer_is_a_loud_500() {
    let tmp = tempfile::tempdir().unwrap();
    let app = common::app_with_issuer(
        tmp.path(),
        "http://127.0.0.1:9",
        None,
        "http://plain-oidc.example",
    );
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ── the submission seams ─────────────────────────────────────────────────────

#[tokio::test]
async fn anonymous_list_is_empty_and_deletes_need_a_token() {
    let issuer = stub_realm().await;
    let tmp = tempfile::tempdir().unwrap();
    let app = common::app_with_issuer(tmp.path(), "http://127.0.0.1:9", None, &issuer);

    // Private list: anonymous → [] — the (dead) store is never touched, or this would 500.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/submissions?path=dsa/two-sum")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 4096).await.unwrap();
    assert_eq!(&body[..], b"[]");

    // Owner-only verbs 401 for anonymous callers.
    for uri in [
        "/api/submissions/00000000-0000-0000-0000-000000000000",
        "/api/submissions",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[tokio::test]
async fn a_bad_bearer_is_401_never_silently_anonymous() {
    let issuer = stub_realm().await;
    let tmp = tempfile::tempdir().unwrap();
    let app = common::app_with_issuer(tmp.path(), "http://127.0.0.1:9", None, &issuer);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/submissions?path=dsa/two-sum")
                .header(header::AUTHORIZATION, "Bearer not-a-jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "a present-but-bad token must 401"
    );
}
