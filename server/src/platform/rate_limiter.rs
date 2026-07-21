//! In-memory fixed-window rate limiting. Two buckets with two
//! key namespaces: anonymous meters per IP, authenticated per subject — a per-person key
//! survives NAT and gives signed-in readers the bigger budget. Windows are FLOOR-ALIGNED to the
//! epoch (everyone's window rolls at the same instant); expired entries are pruned
//! opportunistically once the map outgrows `PRUNE_ABOVE`, so an IP scan can't grow it
//! unbounded. Redis stays deferred — this trait is the port a distributed adapter would fill.

use std::collections::HashMap;
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

/// The refusal: how long until the window rolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("rate limited; retry after {retry_after_sec}s")]
pub struct Throttled {
    pub retry_after_sec: u32,
}

const PRUNE_ABOVE: usize = 4096;

pub struct RateLimiter {
    anonymous: RateLimitBucket,
    authenticated: RateLimitBucket,
    /// key → (window expiry epoch-sec, count). A plain mutex: the critical section is a map
    /// probe — no await inside, no contention story worth an actor.
    state: Mutex<HashMap<String, (u64, u32)>>,
}

impl RateLimiter {
    pub fn new(anonymous: RateLimitBucket, authenticated: RateLimitBucket) -> Self {
        tracing::debug!(?anonymous, ?authenticated, "rate limiter configured");
        Self {
            anonymous,
            authenticated,
            state: Mutex::new(HashMap::new()),
        }
    }

    pub fn consume_anonymous(&self, ip: &str) -> Result<Quota, Throttled> {
        self.consume_at(self.anonymous, &format!("anon:{ip}"), now_epoch())
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
            });
        }
        Ok(Quota {
            used: count,
            limit: bucket.limit,
            reset_epoch_sec: expiry,
        })
    }
}

/// A poisoned lock means a panic mid-insert on a plain map — the data cannot be torn; keep
/// serving rather than poisoning every request after one bug.
fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "rate_limiter_tests.rs"]
mod tests;
