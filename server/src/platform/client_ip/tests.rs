//! The precedence deciding which address rate limiting counts against: the first hop of
//! `X-Forwarded-For`, then `X-Real-IP`, then the peer, then `unknown`.

#![allow(clippy::unwrap_used)]

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
