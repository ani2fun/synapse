//! The caller's IP for anonymous rate-limit keys: the LAST
//! `X-Forwarded-For` hop, then `X-Real-IP`, then the socket peer, then a shared `"unknown"`
//! bucket. Good enough for budgets, never for auth.
//!
//! The LAST hop, because it is the only one a client cannot write. The edge (one Traefik, in
//! front of this server) APPENDS the address it accepted the connection from, so everything to
//! the left of it arrived in the request — and a client that sends its own `X-Forwarded-For:
//! 1.2.3.4` would otherwise pick its own budget key, a fresh one per request. Traefik also strips
//! an untrusted client's forwarding headers by default, so today the two readings agree; this one
//! keeps the budget honest if that default ever changes (`forwardedHeaders.insecure`). A second
//! trusted proxy in front of Traefik would make its address the last hop, and this would need to
//! skip that many from the right. `Peer` is an
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
        .and_then(|v| v.rsplit(',').next())
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
mod tests;
