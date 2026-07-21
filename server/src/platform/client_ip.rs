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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_for_wins_and_takes_the_first_hop() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        headers.insert("x-real-ip", "10.0.0.2".parse().unwrap());
        assert_eq!(client_ip(&headers, None), "203.0.113.7");
    }

    #[test]
    fn real_ip_then_peer_then_unknown() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "198.51.100.4".parse().unwrap());
        assert_eq!(client_ip(&headers, None), "198.51.100.4");

        let peer = SocketAddr::from(([127, 0, 0, 1], 4321));
        assert_eq!(client_ip(&HeaderMap::new(), Some(peer)), "127.0.0.1");
        assert_eq!(client_ip(&HeaderMap::new(), None), "unknown");
    }
}
