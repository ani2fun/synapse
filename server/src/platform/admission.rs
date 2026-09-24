//! Admission to the sandbox: how many runs may be IN FLIGHT at once — per caller, and in total.
//!
//! The rate limiter counts runs per window; it cannot see how long each one holds the sandbox,
//! and go-judge runs ONE program at a time. Ten slow runs a minute from one address is well
//! inside a run budget and several minutes of sandbox, so without this a handful of callers keep
//! the queue full and everyone else waits out the edge timeout behind them. This gate bounds the
//! queue itself:
//!
//! - **per caller** — a caller already holding `per_caller` runs is refused (`CallerBusy`) until
//!   one finishes. More than one, because Cancel in the workbench only abandons the response; the
//!   run it started still finishes on the server, and pressing Run again must not be refused.
//! - **in total** — past `total` runs, running or waiting, a new one is refused at once
//!   (`SandboxBusy`) rather than queued. A refusal in a millisecond that says "try again" beats a
//!   request that sits in line for two minutes and then times out.
//!
//! A `Ticket` holds the place and gives it back on drop, so every path out of a run — success,
//! error, panic — releases it. The caller holds it for the whole sandbox call; see `run_code`
//! for why that call must not be tied to the client's connection.
//!
//! The keys are the rate limiter's (`rate_limiter::budget_key` for an address, the subject for a
//! signed-in caller), so an IPv6 host is one caller here too.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::platform::rate_limiter::lock_unpoisoned;

/// Why a run was not admitted. Typed because the edge answers the two differently: one is the
/// caller's own doing, the other is everyone's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// This caller already has `limit` runs in flight.
    #[error("caller already has {limit} run(s) in flight")]
    CallerBusy { limit: u32 },
    /// The sandbox's queue is full.
    #[error("sandbox queue is full")]
    SandboxBusy,
}

#[derive(Default)]
struct InFlight {
    total: u32,
    by_caller: HashMap<String, u32>,
}

pub struct Admission {
    per_caller: u32,
    total: u32,
    in_flight: Mutex<InFlight>,
}

/// One admitted run's place in the sandbox queue, released on drop.
pub struct Ticket {
    admission: Arc<Admission>,
    key: String,
}

impl Admission {
    /// Both limits floor at 1: a zero would refuse every run.
    pub fn new(per_caller: u32, total: u32) -> Self {
        tracing::debug!(per_caller, total, "admission configured");
        Self {
            per_caller: per_caller.max(1),
            total: total.max(1),
            in_flight: Mutex::new(InFlight::default()),
        }
    }

    /// Take a place for `key`, or say which limit is full. The caller's own limit is checked
    /// first: a caller who is the reason they are refused should be told so.
    pub fn admit(self: &Arc<Self>, key: &str) -> Result<Ticket, Refused> {
        let mut in_flight = lock_unpoisoned(&self.in_flight);
        let held = in_flight.by_caller.get(key).copied().unwrap_or(0);
        if held >= self.per_caller {
            tracing::warn!(key, held, "admission: caller busy");
            return Err(Refused::CallerBusy {
                limit: self.per_caller,
            });
        }
        if in_flight.total >= self.total {
            tracing::warn!(total = in_flight.total, "admission: sandbox busy");
            return Err(Refused::SandboxBusy);
        }
        in_flight.total += 1;
        in_flight.by_caller.insert(key.to_owned(), held + 1);
        Ok(Ticket {
            admission: Arc::clone(self),
            key: key.to_owned(),
        })
    }

    /// Runs in flight, for tests and logs.
    pub fn in_flight(&self) -> u32 {
        lock_unpoisoned(&self.in_flight).total
    }

    fn release(&self, key: &str) {
        let mut in_flight = lock_unpoisoned(&self.in_flight);
        in_flight.total = in_flight.total.saturating_sub(1);
        // A caller with nothing in flight leaves the map, so it holds only live callers.
        if let Some(held) = in_flight.by_caller.get_mut(key) {
            *held -= 1;
            if *held == 0 {
                in_flight.by_caller.remove(key);
            }
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        self.admission.release(&self.key);
    }
}

#[cfg(test)]
mod tests;
