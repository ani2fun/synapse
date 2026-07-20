//! The Astro front door (migration step A01) — forwards page requests to the SSR sidecar.
//!
//! Modeled on `likec4_proxy` (buffered, GET-shaped, 502 on an unreachable upstream) but mounted
//! as the router's **fallback**, not a wildcard route. Registered routes — `/api`, `/media`,
//! `/c4`, robots/sitemap — always win over the fallback in axum, so the Cortex-inherited
//! "greedy wildcard shadows /api" scar cannot recur here by construction, and the sidecar's own
//! 404 page becomes the site 404.
//!
//! Header contract, stated once:
//! - Upstream request: original path+query, `accept` and `if-none-match` forwarded.
//!   `accept-encoding` is STRIPPED — the response passes back through axum's
//!   `CompressionLayer`, and compressing on both sides would either double-compress or make
//!   axum ship bytes it cannot inspect. `authorization` and `cookie` are NEVER forwarded:
//!   SSR renders anonymous by design (auth is a browser-side island), so the sidecar has no
//!   business seeing credentials.
//! - Response copyback: status + `content-type`, `cache-control`, `etag`, `vary`, `location`.
//!   Everything else the sidecar says is dropped — the axum stack owns security headers,
//!   compression and tracing.
//! - `/_astro/*` hashed assets get `immutable` stamped if the sidecar omitted a cache header,
//!   matching the old client's hashed-asset policy.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};

#[derive(Clone)]
pub struct AstroProxy {
    client: reqwest::Client,
    upstream_base: String,
}

const ASSET_CACHE: &str = "public, max-age=31536000, immutable";

impl AstroProxy {
    pub fn new(upstream_base: &str) -> Self {
        let client = reqwest::Client::builder()
            .http1_only()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            // Builder failure = TLS backend missing at boot — a config bug, not a request error.
            .unwrap_or_default();
        Self {
            client,
            upstream_base: upstream_base.trim_end_matches('/').to_owned(),
        }
    }
}

pub async fn handle(State(proxy): State<AstroProxy>, request: Request) -> Response {
    // Pages are GET-shaped; anything else at the fallback is a client error, not a proxy job.
    // (HEAD forwards as GET — hyper elides the body on the way out.)
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path_and_query = request.uri().path_and_query().map_or("/", |pq| pq.as_str());
    let url = format!("{}{path_and_query}", proxy.upstream_base);

    let mut upstream = proxy.client.get(&url);
    for name in [header::ACCEPT, header::IF_NONE_MATCH] {
        if let Some(value) = request.headers().get(&name) {
            upstream = upstream.header(name.clone(), value.clone());
        }
    }

    match upstream.send().await {
        Ok(response) => {
            let status = StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let mut builder = Response::builder().status(status);
            for name in [
                header::CONTENT_TYPE,
                header::CACHE_CONTROL,
                header::ETAG,
                header::VARY,
                header::LOCATION,
            ] {
                if let Some(value) = response.headers().get(&name)
                    && let Ok(copied) = HeaderValue::from_bytes(value.as_bytes())
                {
                    builder = builder.header(name.clone(), copied);
                }
            }
            let missing_cache = response.headers().get(header::CACHE_CONTROL).is_none();
            if missing_cache && path_and_query.starts_with("/_astro/") {
                builder = builder.header(header::CACHE_CONTROL, HeaderValue::from_static(ASSET_CACHE));
            }
            match response.bytes().await {
                Ok(bytes) => builder
                    .body(Body::from(bytes))
                    .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response()),
                Err(error) => bad_gateway(&url, &error),
            }
        }
        Err(error) => bad_gateway(&url, &error),
    }
}

fn bad_gateway(url: &str, error: &dyn std::fmt::Display) -> Response {
    tracing::warn!(url, %error, "astro proxy: upstream failed");
    StatusCode::BAD_GATEWAY.into_response()
}
