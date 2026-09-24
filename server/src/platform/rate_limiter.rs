//! In-memory fixed-window rate limiting. Two buckets with two
//! key namespaces: anonymous meters per IP, authenticated per subject — a per-person key
//! survives NAT and gives signed-in readers the bigger budget.
//!
//! Anonymous traffic also draws on a THIRD bucket that every anonymous caller shares: a
//! site-wide ceiling. Per-IP budgets add up with no bound — a hundred addresses get a hundred
//! budgets — and the sandbox behind them runs one program at a time, so the ceiling is the
//! circuit breaker that keeps anonymous load from starving everyone. Signed-in callers never
//! touch it: their budget is theirs, and signing in is the way past a busy anonymous minute.
//!
//! An IPv6 caller is keyed by its /64, not its address. One host is routinely handed a whole
//! /64, so keying the full address would give one machine 2^64 budgets. Windows are FLOOR-ALIGNED to the
//! epoch (everyone's window rolls at the same instant); expired entries are pruned
//! opportunistically once the map outgrows `PRUNE_ABOVE`, so an IP scan can't grow it
//! unbounded. Redis stays deferred — this trait is the port a distributed adapter would fill.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// One bucket's shape: `limit` consumes per `window_seconds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitBucket {
    pub window_seconds: u64,
    pub limit: u32,
}

/// A successful consume's receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quota {
    pub used: u32,
    pub limit: u32,
    pub reset_epoch_sec: u64,
}

/// Whose budget ran out — which decides what the refusal tells the caller to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleScope {
    /// This caller's own budget: wait, or sign in for a bigger one.
    Caller,
    /// The ceiling every anonymous caller shares: this caller may not have run anything yet.
    AllAnonymous,
}

/// The refusal: how long until the window rolls, and whose window it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("rate limited; retry after {retry_after_sec}s")]
pub struct Throttled {
    pub retry_after_sec: u32,
    pub scope: ThrottleScope,
}

const PRUNE_ABOVE: usize = 4096;

/// The shared anonymous ledger's key. No IP formats as `*`, so it cannot collide with one.
const ALL_ANONYMOUS_KEY: &str = "anon:*";

pub struct RateLimiter {
    anonymous: RateLimitBucket,
    authenticated: RateLimitBucket,
    /// The site-wide anonymous ceiling — see the module doc.
    all_anonymous: RateLimitBucket,
    /// key → (window expiry epoch-sec, count). A plain mutex: the critical section is a map
    /// probe — no await inside, no contention story worth an actor.
    state: Mutex<HashMap<String, (u64, u32)>>,
}

impl RateLimiter {
    pub fn new(
        anonymous: RateLimitBucket,
        authenticated: RateLimitBucket,
        all_anonymous: RateLimitBucket,
    ) -> Self {
        tracing::debug!(
            ?anonymous,
            ?authenticated,
            ?all_anonymous,
            "rate limiter configured"
        );
        Self {
            anonymous,
            authenticated,
            all_anonymous,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// The caller's own budget first, then the shared ceiling. A caller already over their own
    /// budget is refused without spending the shared one, so one noisy address cannot drain it
    /// by being refused.
    pub fn consume_anonymous(&self, ip: &str) -> Result<Quota, Throttled> {
        self.consume_anonymous_at(ip, now_epoch())
    }

    fn consume_anonymous_at(&self, ip: &str, now: u64) -> Result<Quota, Throttled> {
        let own = self.consume_at(self.anonymous, &format!("anon:{}", budget_key(ip)), now)?;
        self.consume_at(self.all_anonymous, ALL_ANONYMOUS_KEY, now)
            .map_err(|throttled| Throttled {
                scope: ThrottleScope::AllAnonymous,
                ..throttled
            })?;
        Ok(own)
    }

    pub fn consume_authenticated(&self, sub: &str) -> Result<Quota, Throttled> {
        self.consume_at(self.authenticated, &format!("auth:{sub}"), now_epoch())
    }

    /// The clock-explicit core (tests drive `now` directly; the public verbs pass wall time).
    fn consume_at(&self, bucket: RateLimitBucket, key: &str, now: u64) -> Result<Quota, Throttled> {
        let expiry = (now / bucket.window_seconds + 1) * bucket.window_seconds;
        let count = {
            let mut state = lock_unpoisoned(&self.state);
            if state.len() > PRUNE_ABOVE {
                state.retain(|_, (exp, _)| *exp > now);
            }
            let count = match state.get(key) {
                Some((exp, c)) if *exp > now => c + 1,
                _ => 1,
            };
            state.insert(key.to_owned(), (expiry, count));
            count
        };
        if count > bucket.limit {
            tracing::warn!(key, count, limit = bucket.limit, "rate limit: throttled");
            #[allow(clippy::cast_possible_truncation)] // window_seconds bounds the difference
            return Err(Throttled {
                retry_after_sec: (expiry - now).max(1) as u32,
                scope: ThrottleScope::Caller,
            });
        }
        Ok(Quota {
            used: count,
            limit: bucket.limit,
            reset_epoch_sec: expiry,
        })
    }
}

/// The address a budget is kept under. IPv4 as written; IPv6 by its /64, the block one host is
/// commonly assigned whole; an IPv4-mapped IPv6 address as the IPv4 it carries. Anything that is
/// not an address (the shared `unknown`) is its own key.
pub(crate) fn budget_key(ip: &str) -> String {
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V6(v6)) => v6.to_ipv4_mapped().map_or_else(
            || {
                let [a, b, c, d, ..] = v6.segments();
                format!("{a:x}:{b:x}:{c:x}:{d:x}::/64")
            },
            |v4| v4.to_string(),
        ),
        Ok(IpAddr::V4(v4)) => v4.to_string(),
        Err(_) => ip.to_owned(),
    }
}

/// A poisoned lock means a panic mid-insert on a plain map — the data cannot be torn; keep
/// serving rather than poisoning every request after one bug.
pub(crate) fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
