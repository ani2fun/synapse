//! The caller's IP for anonymous rate-limit keys. Good enough for budgets, never for auth.
//!
//! First, the EDGE's address for the request: the LAST `X-Forwarded-For` hop, then `X-Real-IP`,
//! then the socket peer, then a shared `"unknown"` bucket. The LAST hop, because it is the only
//! one a client cannot write: Traefik APPENDS the address it accepted the connection from, so
//! everything to the left of it arrived in the request, and a client sending its own
//! `X-Forwarded-For: 1.2.3.4` would otherwise pick its own budget key, a fresh one per request.
//!
//! Then, CLOUDFLARE. The site is proxied through it, so the address Traefik accepted is a
//! Cloudflare edge's, not the reader's — and Cloudflare connects from many edges, a different one
//! from request to request. Keyed by that, every anonymous budget and the per-caller admission cap
//! are shared by strangers and escaped by anyone: fourteen runs a minute went through a ten-a-minute
//! budget in production. Cloudflare names the real client in `CF-Connecting-IP`, and overwrites it
//! on every request, so through Cloudflare it cannot be forged. It IS forgeable by a client that
//! reaches the origin directly — which works, the origin answers on its own address — so the
//! header is believed ONLY when the edge address is inside Cloudflare's published ranges. A
//! direct request's edge address is the client's own, and its `CF-Connecting-IP` is ignored.
//!
//! `Peer` is an infallible extractor over the connect-info extension — present when `main` serves
//! with connect info, absent (and harmless) under the in-process test router.

use std::net::{IpAddr, SocketAddr};
use std::sync::LazyLock;

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
    let edge = edge_ip(headers, peer);
    let through_cloudflare = edge.parse::<IpAddr>().is_ok_and(is_cloudflare);
    if through_cloudflare
        && let Some(client) = header(headers, "cf-connecting-ip").filter(|v| v.parse::<IpAddr>().is_ok())
    {
        return client.to_owned();
    }
    edge
}

/// The address the edge accepted the connection from — see the module doc.
fn edge_ip(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    let forwarded = header(headers, "x-forwarded-for")
        .and_then(|v| v.rsplit(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty());
    if let Some(ip) = forwarded {
        return ip.to_owned();
    }
    if let Some(ip) = header(headers, "x-real-ip") {
        return ip.to_owned();
    }
    peer.map_or_else(|| "unknown".to_owned(), |addr| addr.ip().to_string())
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

/// Cloudflare's published edge ranges (<https://www.cloudflare.com/ips/>). They change rarely; a
/// range missing here costs the requests it carries their real key — they fall back to the edge
/// address, the behaviour this replaces — and never lets a client choose one.
const CLOUDFLARE_RANGES: &[&str] = &[
    "173.245.48.0/20",
    "103.21.244.0/22",
    "103.22.200.0/22",
    "103.31.4.0/22",
    "141.101.64.0/18",
    "108.162.192.0/18",
    "190.93.240.0/20",
    "188.114.96.0/20",
    "197.234.240.0/22",
    "198.41.128.0/17",
    "162.158.0.0/15",
    "104.16.0.0/13",
    "104.24.0.0/14",
    "172.64.0.0/13",
    "131.0.72.0/22",
    "2400:cb00::/32",
    "2606:4700::/32",
    "2803:f800::/32",
    "2405:b500::/32",
    "2405:8100::/32",
    "2a06:98c0::/29",
    "2c0f:f248::/32",
];

static CLOUDFLARE: LazyLock<Vec<(IpAddr, u8)>> = LazyLock::new(|| {
    CLOUDFLARE_RANGES
        .iter()
        .filter_map(|range| parse_cidr(range))
        .collect()
});

fn parse_cidr(range: &str) -> Option<(IpAddr, u8)> {
    let (addr, prefix) = range.split_once('/')?;
    Some((addr.parse().ok()?, prefix.parse().ok()?))
}

pub(crate) fn is_cloudflare(ip: IpAddr) -> bool {
    CLOUDFLARE.iter().any(|&(net, prefix)| in_range(ip, net, prefix))
}

fn in_range(ip: IpAddr, net: IpAddr, prefix: u8) -> bool {
    match (ip, net) {
        (IpAddr::V4(ip), IpAddr::V4(net)) => {
            let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
            u32::from(ip) & mask == u32::from(net) & mask
        }
        (IpAddr::V6(ip), IpAddr::V6(net)) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
            u128::from(ip) & mask == u128::from(net) & mask
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
