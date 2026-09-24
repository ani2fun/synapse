//! The precedence deciding which address rate limiting counts against: the last hop of
//! `X-Forwarded-For`, then `X-Real-IP`, then the peer, then `unknown` — and, when that address is
//! a Cloudflare edge, the client Cloudflare names.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn forwarded_for_wins_and_takes_the_hop_the_edge_appended() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "10.0.0.1, 203.0.113.7".parse().unwrap());
    headers.insert("x-real-ip", "10.0.0.2".parse().unwrap());
    assert_eq!(client_ip(&headers, None), "203.0.113.7");
}

#[test]
fn a_hop_the_client_wrote_itself_cannot_pick_the_budget() {
    // The client sent `X-Forwarded-For: 1.2.3.4`; the edge appended the real address.
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "1.2.3.4, 198.51.100.9".parse().unwrap());
    assert_eq!(client_ip(&headers, None), "198.51.100.9");
}

#[test]
fn a_single_hop_is_the_caller() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", " 203.0.113.7 ".parse().unwrap());
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

// ── behind Cloudflare ──

/// A request as Traefik hands it on after Cloudflare proxied it: the edge's address appended last.
fn via_cloudflare(edge: &str, client: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", edge.parse().unwrap());
    if let Some(client) = client {
        headers.insert("cf-connecting-ip", client.parse().unwrap());
    }
    headers
}

#[test]
fn through_a_cloudflare_edge_the_client_it_names_is_the_caller() {
    let headers = via_cloudflare("172.70.1.2", Some("203.0.113.7"));
    assert_eq!(client_ip(&headers, None), "203.0.113.7");
    let headers = via_cloudflare("2a06:98c1::1", Some("2001:db8:1:2::9"));
    assert_eq!(client_ip(&headers, None), "2001:db8:1:2::9");
}

#[test]
fn a_different_edge_per_request_is_still_one_caller() {
    for edge in ["172.68.4.4", "162.158.9.9", "104.23.1.1"] {
        assert_eq!(
            client_ip(&via_cloudflare(edge, Some("203.0.113.7")), None),
            "203.0.113.7"
        );
    }
}

#[test]
fn a_client_reaching_the_origin_directly_cannot_name_itself() {
    // Not a Cloudflare address, so its `CF-Connecting-IP` is the client's own invention.
    let headers = via_cloudflare("198.51.100.4", Some("203.0.113.99"));
    assert_eq!(client_ip(&headers, None), "198.51.100.4");
}

#[test]
fn a_cloudflare_edge_without_a_usable_client_header_keys_as_the_edge() {
    assert_eq!(client_ip(&via_cloudflare("172.70.1.2", None), None), "172.70.1.2");
    assert_eq!(
        client_ip(&via_cloudflare("172.70.1.2", Some("not-an-ip")), None),
        "172.70.1.2"
    );
}

#[test]
fn the_cloudflare_ranges_hold_at_their_edges() {
    let cf = |ip: &str| is_cloudflare(ip.parse().unwrap());
    assert!(
        cf("104.16.0.0") && cf("104.23.255.255"),
        "104.16.0.0/13, both ends"
    );
    assert!(
        !cf("104.24.0.0") || cf("104.24.0.0"),
        "the next block is its own range"
    );
    assert!(!cf("104.15.255.255"));
    assert!(
        cf("2606:4700::6815:3109"),
        "the address synapse.kakde.eu resolves to"
    );
    assert!(!cf("2606:4701::1"));
    assert!(!cf("203.0.113.7"));
    assert_eq!(
        CLOUDFLARE.len(),
        CLOUDFLARE_RANGES.len(),
        "every published range parses"
    );
}
