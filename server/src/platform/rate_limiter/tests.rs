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

const ALL_ANON: RateLimitBucket = RateLimitBucket {
    window_seconds: 60,
    limit: 5,
};

fn limiter() -> RateLimiter {
    RateLimiter::new(ANON, AUTH, ALL_ANON)
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

// ── the shared anonymous ceiling ──

#[test]
fn anonymous_callers_share_one_ceiling_past_their_own_budgets() {
    let limiter = limiter();
    // Five addresses, one run each: every one is under its own budget of 3.
    for n in 0..5 {
        limiter
            .consume_anonymous_at(&format!("203.0.113.{n}"), 100)
            .unwrap();
    }
    let refused = limiter.consume_anonymous_at("203.0.113.9", 101).unwrap_err();
    assert_eq!(
        refused.scope,
        ThrottleScope::AllAnonymous,
        "a fresh address met the ceiling"
    );
    assert!((1..=60).contains(&refused.retry_after_sec));
}

#[test]
fn a_caller_over_their_own_budget_does_not_spend_the_shared_one() {
    let limiter = limiter();
    for t in 0..3 {
        limiter.consume_anonymous_at("198.51.100.1", 100 + t).unwrap();
    }
    for t in 0..10 {
        let refused = limiter.consume_anonymous_at("198.51.100.1", 103 + t).unwrap_err();
        assert_eq!(refused.scope, ThrottleScope::Caller);
    }
    // 3 of the ceiling's 5 are spent — the ten refusals took none of it.
    limiter.consume_anonymous_at("198.51.100.2", 114).unwrap();
    limiter.consume_anonymous_at("198.51.100.3", 115).unwrap();
    assert!(limiter.consume_anonymous_at("198.51.100.4", 116).is_err());
}

#[test]
fn signed_in_callers_never_meet_the_anonymous_ceiling() {
    let limiter = limiter();
    for n in 0..5 {
        limiter
            .consume_anonymous_at(&format!("203.0.113.{n}"), 100)
            .unwrap();
    }
    assert!(limiter.consume_authenticated("someone").is_ok());
}

// ── the key a budget is kept under ──

#[test]
fn an_ipv6_host_is_one_budget_across_its_whole_slash_64() {
    assert_eq!(budget_key("2001:db8:1:2:aaaa::1"), "2001:db8:1:2::/64");
    assert_eq!(
        budget_key("2001:db8:1:2:ffff:ffff:ffff:ffff"),
        "2001:db8:1:2::/64"
    );
    assert_ne!(budget_key("2001:db8:1:3::1"), budget_key("2001:db8:1:2::1"));
}

#[test]
fn ipv4_and_its_mapped_ipv6_form_are_one_budget() {
    assert_eq!(budget_key("203.0.113.7"), "203.0.113.7");
    assert_eq!(budget_key("::ffff:203.0.113.7"), "203.0.113.7");
}

#[test]
fn something_that_is_not_an_address_keys_as_written() {
    assert_eq!(budget_key("unknown"), "unknown");
}

#[test]
fn rotating_through_a_slash_64_does_not_mint_budgets() {
    let limiter = limiter();
    for n in 0..3 {
        limiter
            .consume_anonymous_at(&format!("2001:db8:1:2::{n}"), 100)
            .unwrap();
    }
    let refused = limiter.consume_anonymous_at("2001:db8:1:2::99", 101).unwrap_err();
    assert_eq!(refused.scope, ThrottleScope::Caller);
}
