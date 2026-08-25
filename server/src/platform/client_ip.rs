//! The caller's IP for anonymous rate-limit keys: the first
//! `X-Forwarded-For` hop (the edge appends; good enough for budgets, not for auth), then
//! `X-Real-IP`, then the socket peer, then a shared `"unknown"` bucket. `Peer` is an
//! infallible extractor over the connect-info extension — present when `main` serves with
//! connect info, absent (and harmless) under the in-process test router.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::request::Parts;

/// The TCP peer, when the serving stack recorded one.
pub struct Peer(pub Option<SocketAddr>);

impl<S: Send + Sync> FromRequestParts<S> for Peer {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0),
        ))
    }
}

pub fn client_ip(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty());
    if let Some(ip) = forwarded {
        return ip.to_owned();
    }
    let real = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty());
    if let Some(ip) = real {
        return ip.to_owned();
    }
    peer.map_or_else(|| "unknown".to_owned(), |addr| addr.ip().to_string())
}

#[cfg(test)]
#[path = "client_ip_tests.rs"]
mod tests;
