//! The narrator: a [`Narration`] as one continuous MP3 stream.
//!
//! PULL-BASED. Each unit is fetched or synthesised only when the response body asks for the next
//! chunk, so the engine runs exactly as fast as the reader's connection drains — and a reader who
//! closes the tab drops the body, which drops the narrator mid-unit and frees the engine for
//! someone else. A detached producer would keep synthesising a lesson nobody is listening to.
//!
//! A unit the engine fails on is SKIPPED: one bad sentence should not end a twenty-minute
//! lesson. Two failures in a row mean the engine is down, and the stream ends with an error so
//! the player can say so rather than going quietly silent.

use std::sync::Arc;

use axum::body::Bytes;
use futures_util::Stream;
use tokio::sync::Semaphore;

use crate::listen::{AudioCache, ListenError, Narration, SpeechEngine, cache_key};
use crate::platform::admission::Ticket;

/// Consecutive engine failures that end the stream.
const GIVE_UP_AFTER: u32 = 2;

/// What every unit needs, shared across a stream's whole life.
pub struct Narrator<E> {
    pub engine: Arc<E>,
    pub cache: Arc<AudioCache>,
    /// The single synthesis permit. Held only while the engine works, never across a cache hit.
    pub synth_gate: Arc<Semaphore>,
}

impl<E> Clone for Narrator<E> {
    fn clone(&self) -> Self {
        Self {
            engine: Arc::clone(&self.engine),
            cache: Arc::clone(&self.cache),
            synth_gate: Arc::clone(&self.synth_gate),
        }
    }
}

impl<E: SpeechEngine + 'static> Narrator<E> {
    /// How long each unit from `from` plays, as far as the cache can say: contiguous, stopping at
    /// the first unit not yet synthesised, at most `limit` of them. Never touches the engine.
    pub async fn durations(&self, narration: &Narration, from: usize, limit: usize) -> Vec<f64> {
        let mut durations = Vec::new();
        for text in narration.units.iter().skip(from).take(limit) {
            let Some(audio) = self.cache.get(&cache_key(narration.voice, text)).await else {
                break;
            };
            durations.push(crate::listen::mp3::duration_secs(&audio));
        }
        durations
    }

    /// One unit's audio: the cache, or the engine (which then fills the cache).
    pub async fn unit(&self, narration: &Narration, index: usize) -> Result<Bytes, ListenError> {
        let text = &narration.units[index];
        let key = cache_key(narration.voice, text);
        if let Some(audio) = self.cache.get(&key).await {
            tracing::debug!(index, "listen: cache hit");
            return Ok(audio);
        }
        tracing::debug!(index, "listen: cache miss, waiting for the engine");
        let _permit = Arc::clone(&self.synth_gate)
            .acquire_owned()
            .await
            .map_err(|e| ListenError::EngineFailed(e.to_string()))?;
        let audio = self.engine.synthesize(narration.voice, text).await?;
        self.cache.put(&key, audio.clone()).await;
        Ok(audio)
    }

    /// The units from `from` onwards as a body stream. `hold` is the reader's admission ticket:
    /// it lives inside the stream, so the place is given back exactly when the body is dropped.
    pub fn stream(
        self,
        narration: Arc<Narration>,
        from: usize,
        hold: Ticket,
    ) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static {
        let state = (self, narration, from, 0_u32, hold);
        futures_util::stream::unfold(
            state,
            |(narrator, narration, mut next, mut failures, hold)| async move {
                while next < narration.units.len() {
                    let index = next;
                    next += 1;
                    match narrator.unit(&narration, index).await {
                        Ok(audio) => return Some((Ok(audio), (narrator, narration, next, 0, hold))),
                        Err(error) => {
                            failures += 1;
                            tracing::warn!(%error, index, failures, "listen: unit skipped");
                            if failures >= GIVE_UP_AFTER {
                                let gave_up = std::io::Error::other(error.to_string());
                                // Past the last unit, so the next poll ends the stream.
                                let done = narration.units.len();
                                return Some((Err(gave_up), (narrator, narration, done, failures, hold)));
                            }
                        }
                    }
                }
                tracing::info!(units = narration.units.len(), "listen: stream finished");
                None
            },
        )
    }
}
