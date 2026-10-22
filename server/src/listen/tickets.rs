//! Listen tickets: the bridge between a bearer-authenticated request and an `<audio>` element
//! that cannot send one.
//!
//! A ticket is a random id naming a validated [`Narration`] and the reader it was issued to. It
//! may be redeemed more than once — browsers routinely open a media URL twice (a probe, then the
//! real fetch), and the second costs only cache reads. The TTL is SLIDING: every use renews it, so
//! a page that is playing (and asking for the stream's timing every few seconds) keeps its ticket
//! for as long as it plays, and an abandoned one lapses a TTL after its last use.
//!
//! In memory, which is correct for the one replica this deployment runs (the rate limiter and the
//! admission gate make the same assumption). A restart forgets every ticket, and the page simply
//! asks for a new one.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::time::Instant;

use crate::identity::domain::Username;
use crate::listen::Narration;
use crate::platform::rate_limiter::lock_unpoisoned;

/// The most tickets held at once. Each carries at most a lesson's text, so this bounds the memory
/// the map can ever hold; past it, the oldest is dropped.
const MAX_TICKETS: usize = 512;

struct Issued {
    owner: Username,
    narration: Arc<Narration>,
    expires: Instant,
}

pub struct Tickets {
    ttl: Duration,
    issued: Mutex<HashMap<String, Issued>>,
}

impl Tickets {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            issued: Mutex::new(HashMap::new()),
        }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Issue a ticket, dropping lapsed ones first and the oldest live one if the map is full.
    pub fn issue(&self, owner: Username, narration: Narration) -> String {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let now = Instant::now();
        let mut issued = lock_unpoisoned(&self.issued);
        issued.retain(|_, ticket| ticket.expires > now);
        if issued.len() >= MAX_TICKETS
            && let Some(oldest) = issued
                .iter()
                .min_by_key(|(_, ticket)| ticket.expires)
                .map(|(id, _)| id.clone())
        {
            issued.remove(&oldest);
        }
        issued.insert(
            id.clone(),
            Issued {
                owner,
                narration: Arc::new(narration),
                expires: now + self.ttl,
            },
        );
        id
    }

    /// The reader and narration a live ticket names, renewing it; `None` once it has lapsed or
    /// never existed.
    pub fn redeem(&self, id: &str) -> Option<(Username, Arc<Narration>)> {
        let now = Instant::now();
        let mut issued = lock_unpoisoned(&self.issued);
        let ticket = issued.get_mut(id).filter(|ticket| ticket.expires > now)?;
        ticket.expires = now + self.ttl;
        Some((ticket.owner.clone(), Arc::clone(&ticket.narration)))
    }
}
