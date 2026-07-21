//! The `/c4` reverse proxy: the LikeC4 container isn't
//! internet-facing — the server forwards `GET /c4/*` to it, STRIPPING the `/c4` prefix (prod
//! gotcha: the image serves UNDER `/c4`, so `LIKEC4_URL` ends in `/c4` and the two cancel).
//! GET-only, buffered, only `content-type` copied back; an unreachable upstream is a 502,
//! never an exception.

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

#[derive(Clone)]
struct ProxyState {
    client: reqwest::Client,
    upstream_base: String,
}

pub fn routes(upstream_base: &str) -> Router {
    let client = reqwest::Client::builder()
        .http1_only()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        // Builder failure = TLS backend missing at boot — a config bug, not a request error.
        .unwrap_or_default();
    let state = ProxyState {
        client,
        upstream_base: upstream_base.trim_end_matches('/').to_owned(),
    };
    Router::new()
        .route("/c4", get(proxy_root))
        // `/c4/` needs its OWN route: axum's `{*rest}` wildcard does not match an empty
        // remainder, so the trailing-slash form 404'd while both `/c4` and `/c4/view/…` served.
        // It is the natural index URL, and the one a browser lands on after a redirect.
        .route("/c4/", get(proxy_root))
        .route("/c4/{*rest}", get(proxy))
        .with_state(state)
}

async fn proxy_root(State(state): State<ProxyState>, RawQuery(query): RawQuery) -> Response {
    forward(&state, "", query.as_deref()).await
}

async fn proxy(
    State(state): State<ProxyState>,
    Path(rest): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    forward(&state, &rest, query.as_deref()).await
}

async fn forward(state: &ProxyState, rest: &str, query: Option<&str>) -> Response {
    let sep = query.map(|q| format!("?{q}")).unwrap_or_default();
    let url = format!("{}/{}{sep}", state.upstream_base, rest.trim_start_matches('/'));
    match state.client.get(&url).send().await {
        Ok(upstream) => {
            let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let content_type = upstream
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| HeaderValue::from_bytes(v.as_bytes()).ok());
            match upstream.bytes().await {
                Ok(bytes) => {
                    let mut response = Response::builder().status(status);
                    if let Some(ct) = content_type {
                        response = response.header(header::CONTENT_TYPE, ct);
                    }
                    response
                        .body(Body::from(bytes))
                        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
                }
                Err(error) => bad_gateway(&url, &error),
            }
        }
        Err(error) => bad_gateway(&url, &error),
    }
}

fn bad_gateway(url: &str, error: &dyn std::fmt::Display) -> Response {
    tracing::warn!(url, %error, "likec4 proxy: upstream failed");
    StatusCode::BAD_GATEWAY.into_response()
}
