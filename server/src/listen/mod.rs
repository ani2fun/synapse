//! Listen — a lesson read aloud by the speech engine, for readers on the listen allowlist.
//!
//! A thin flat context like `notes`: nothing here has behaviour worth a domain layer. The shape:
//!
//! - the reader's page cuts the lesson's prose into UNITS (a few sentences each) and spends its
//!   bearer on `POST /api/listen`, which validates them and issues a short-lived ticket;
//! - the `<audio>` element plays `GET /api/listen/{ticket}.mp3`, ONE continuous MP3 that the
//!   narrator assembles unit by unit — from the cache when the unit was said before, from the
//!   engine when it was not.
//!
//! One stream rather than one file per unit is what keeps a locked phone playing: a single media
//! element is what the OS gives lock-screen controls to, and MP3 frames concatenate, so units can
//! be appended without re-encoding anything.
//!
//! The engine is the scarce part. One instance synthesises at ~2× realtime on two cores — enough
//! to keep ahead of ONE listener — so every synthesis goes through a single permit
//! (`synth_gate`), and a cache hit never touches it. A second listener on fresh text waits rather
//! than slowing both below realtime.

mod cache;
mod engine;
pub mod http;
mod mp3;
mod narrate;
mod postgres;
mod tickets;

pub use cache::{AudioCache, spawn_sweeper};
pub use engine::KokoroEngine;
pub use postgres::PostgresListenAllowlist;
pub use tickets::Tickets;

use std::fmt::Write as _;

use axum::body::Bytes;
use sha2::{Digest, Sha256};
use synapse_shared::listen::{LISTEN_MAX_REQUEST_CHARS, LISTEN_MAX_UNIT_CHARS, LISTEN_VOICES};

/// Folded into every cache key. Bump it when the engine's model or its text normalisation
/// changes, so audio said the old way is never served as if the new engine had said it.
const MODEL_TAG: &str = "kokoro-v1.0";

/// The context's error. HTTP mapping (at `http`): `NotAllowed` → 403; `UnknownVoice`, `Empty`,
/// `UnitTooLong`, `TooLong` → 400; `EngineFailed` → 502; `StoreFailed` → 500.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ListenError {
    #[error("'{0}' is not on the listen allowlist")]
    NotAllowed(String),
    #[error("'{0}' is not one of the voices on offer")]
    UnknownVoice(String),
    #[error("there is nothing to read")]
    Empty,
    #[error("a unit holds at most {max} characters; one has {len}")]
    UnitTooLong { len: usize, max: usize },
    #[error("a listen carries at most {max} characters; this one has {len}")]
    TooLong { len: usize, max: usize },
    #[error("speech engine failed: {0}")]
    EngineFailed(String),
    #[error("listen store failed: {0}")]
    StoreFailed(String),
}

/// A voice on offer — only ever one of [`LISTEN_VOICES`], so an engine request can never carry a
/// voice id the reader typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voice(&'static str);

impl Voice {
    pub fn parse(raw: &str) -> Result<Self, ListenError> {
        LISTEN_VOICES
            .iter()
            .find(|(id, _)| *id == raw)
            .map(|(id, _)| Self(id))
            .ok_or_else(|| ListenError::UnknownVoice(raw.to_owned()))
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// A validated listen: a voice and the units to say, in order, each already whitespace-folded so
/// the same sentence always hashes to the same cache key however the page laid it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Narration {
    pub voice: Voice,
    pub units: Vec<String>,
}

impl Narration {
    /// Characters are counted as `char`s. The client counts UTF-16 code units, which is never
    /// fewer, so a unit the page accepted is never one this refuses.
    pub fn prepare(voice: &str, units: &[String]) -> Result<Self, ListenError> {
        let voice = Voice::parse(voice)?;
        let units: Vec<String> = units
            .iter()
            .map(|unit| unit.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|unit| !unit.is_empty())
            .collect();
        if units.is_empty() {
            return Err(ListenError::Empty);
        }
        let mut total = 0;
        for unit in &units {
            let len = unit.chars().count();
            if len > LISTEN_MAX_UNIT_CHARS {
                return Err(ListenError::UnitTooLong {
                    len,
                    max: LISTEN_MAX_UNIT_CHARS,
                });
            }
            total += len;
        }
        if total > LISTEN_MAX_REQUEST_CHARS {
            return Err(ListenError::TooLong {
                len: total,
                max: LISTEN_MAX_REQUEST_CHARS,
            });
        }
        Ok(Self { voice, units })
    }
}

/// Where a unit's audio lives in the cache: what was said, by which voice, by which model. Hex
/// SHA-256, so it is also safe to use as a file name as it stands.
pub fn cache_key(voice: Voice, text: &str) -> String {
    let mut hasher = Sha256::new();
    for part in [MODEL_TAG, voice.as_str(), text] {
        hasher.update(part.as_bytes());
        // A separator no part can contain, so ("ab", "c") and ("a", "bc") never collide.
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// The speech engine, as the narrator needs it (native AFIT + a concrete adapter, per RS001 — the
/// only other implementation is the test fake).
pub trait SpeechEngine: Send + Sync {
    /// One unit as MP3.
    fn synthesize(&self, voice: Voice, text: &str)
    -> impl Future<Output = Result<Bytes, ListenError>> + Send;
}

#[cfg(test)]
mod tests;
