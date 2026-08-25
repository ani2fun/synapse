//! Driven-clock windows, independent keys, separate ledgers.

#![allow(clippy::unwrap_used)]

use super::*;

const ANON: RateLimitBucket = RateLimitBucket {
    window_seconds: 60,
    limit: 3,
};
const AUTH: RateLimitBucket = RateLimitBucket {
    window_seconds: 3600,
    limit: 5,
};

fn limiter() -> RateLimiter {
    RateLimiter::new(ANON, AUTH)
}

#[test]
fn consuming_to_the_limit_counts_then_throttles() {
    let limiter = limiter();
    let q1 = limiter.consume_at(ANON, "anon:1.2.3.4", 100).unwrap();
    assert_eq!((q1.used, q1.limit), (1, 3));
    limiter.consume_at(ANON, "anon:1.2.3.4", 101).unwrap();
    let q3 = limiter.consume_at(ANON, "anon:1.2.3.4", 102).unwrap();
    assert_eq!(q3.used, 3);
    let throttled = limiter.consume_at(ANON, "anon:1.2.3.4", 103).unwrap_err();
    assert!((1..=60).contains(&throttled.retry_after_sec));
}

#[test]
fn different_keys_meter_independently() {
    let limiter = limiter();
    for t in 0..3 {
        limiter.consume_at(ANON, "anon:a", 100 + t).unwrap();
    }
    let fresh = limiter.consume_at(ANON, "anon:b", 104).unwrap();
    assert_eq!(fresh.used, 1);
}

#[test]
fn a_full_key_is_fresh_again_after_the_window_rolls() {
    let limiter = limiter();
    for t in 0..3 {
        limiter.consume_at(ANON, "anon:a", 100 + t).unwrap();
    }
    assert!(limiter.consume_at(ANON, "anon:a", 104).is_err());
    // The floor-aligned window [60,120) ends at 120; one second past it the key is fresh.
    let after = limiter.consume_at(ANON, "anon:a", 121).unwrap();
    assert_eq!(after.used, 1);
}

#[test]
fn anonymous_and_authenticated_are_separate_ledgers() {
    let limiter = limiter();
    for _ in 0..3 {
        limiter.consume_anonymous("same-key").unwrap();
    }
    assert!(limiter.consume_anonymous("same-key").is_err());
    let authed = limiter.consume_authenticated("same-key").unwrap();
    assert_eq!((authed.used, authed.limit), (1, 5), "its own namespace + budget");
}
