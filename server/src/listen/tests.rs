//! What the listen context promises, without a database or an engine: what counts as a valid
//! listen, how audio is keyed, that the cache and the tickets keep their bounds, and the
//! narrator's two policies — a cache hit never touches the engine, and one bad unit is skipped
//! while two in a row end the stream.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use axum::body::Bytes;
use futures_util::StreamExt;
use synapse_shared::listen::{LISTEN_MAX_REQUEST_CHARS, LISTEN_MAX_UNIT_CHARS};
use tokio::sync::Semaphore;

use super::narrate::Narrator;
use super::*;
use crate::identity::domain::Username;
use crate::platform::admission::Admission;

fn units(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|t| (*t).to_owned()).collect()
}

// ── VALIDATION ────────────────────────────────────────────────────────────────────────────────

#[test]
fn only_a_voice_on_offer_is_accepted() {
    assert_eq!(Voice::parse("af_heart").unwrap().as_str(), "af_heart");
    assert_eq!(
        Voice::parse("af_nicole"),
        Err(ListenError::UnknownVoice("af_nicole".to_owned()))
    );
}

#[test]
fn units_are_whitespace_folded_and_blank_ones_dropped() {
    let narration =
        Narration::prepare("af_heart", &units(&["  A  join\n pairs rows. ", "\n\t", "Done."])).unwrap();
    assert_eq!(narration.units, units(&["A join pairs rows.", "Done."]));
}

#[test]
fn a_listen_with_nothing_to_say_is_refused() {
    assert_eq!(
        Narration::prepare("af_heart", &units(&[" ", ""])),
        Err(ListenError::Empty)
    );
    assert_eq!(Narration::prepare("af_heart", &[]), Err(ListenError::Empty));
}

#[test]
fn a_unit_over_the_limit_is_refused_and_counted_in_chars() {
    // Multi-byte characters: the limit is characters, not bytes.
    let at_limit = "é".repeat(LISTEN_MAX_UNIT_CHARS);
    assert!(Narration::prepare("af_heart", std::slice::from_ref(&at_limit)).is_ok());
    let over = format!("{at_limit}é");
    assert_eq!(
        Narration::prepare("af_heart", &[over]),
        Err(ListenError::UnitTooLong {
            len: LISTEN_MAX_UNIT_CHARS + 1,
            max: LISTEN_MAX_UNIT_CHARS
        })
    );
}

#[test]
fn a_listen_over_the_total_limit_is_refused() {
    let unit = "a".repeat(LISTEN_MAX_UNIT_CHARS);
    let count = LISTEN_MAX_REQUEST_CHARS / LISTEN_MAX_UNIT_CHARS + 1;
    let error = Narration::prepare("af_heart", &vec![unit; count]).unwrap_err();
    assert!(matches!(
        error,
        ListenError::TooLong {
            max: LISTEN_MAX_REQUEST_CHARS,
            ..
        }
    ));
}

// ── CACHE KEYS ────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_cache_key_is_stable_hex_and_depends_on_voice_and_text() {
    let heart = Voice::parse("af_heart").unwrap();
    let emma = Voice::parse("bf_emma").unwrap();
    let key = cache_key(heart, "A join pairs rows.");
    assert_eq!(key.len(), 64);
    assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        key,
        cache_key(heart, "A join pairs rows."),
        "same input, same key"
    );
    assert_ne!(
        key,
        cache_key(emma, "A join pairs rows."),
        "voice is part of the key"
    );
    assert_ne!(
        key,
        cache_key(heart, "A join pairs rows"),
        "text is part of the key"
    );
}

// ── CACHE ─────────────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_cache_returns_what_it_kept_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let cache = AudioCache::new(dir.path(), 1);
    assert_eq!(cache.get("ab12").await, None);
    cache.put("ab12", Bytes::from_static(b"mp3")).await;
    assert_eq!(cache.get("ab12").await, Some(Bytes::from_static(b"mp3")));
    assert!(
        dir.path().join("ab").join("ab12.mp3").is_file(),
        "sharded by the key's prefix"
    );
}

#[tokio::test]
async fn a_sweep_evicts_the_least_recently_used_until_under_the_ceiling() {
    let dir = tempfile::tempdir().unwrap();
    // A 1 MiB ceiling, and three 512 KiB files: one has to go.
    let cache = AudioCache::new(dir.path(), 1);
    let half_mib = Bytes::from(vec![0_u8; 512 * 1024]);
    for key in ["aa01", "bb02", "cc03"] {
        cache.put(key, half_mib.clone()).await;
    }
    let base = SystemTime::now() - Duration::from_hours(1);
    for (key, age) in [("aa01", 30), ("bb02", 10), ("cc03", 20)] {
        let path = dir.path().join(&key[..2]).join(format!("{key}.mp3"));
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(base + Duration::from_secs(age)).unwrap();
    }
    let swept = cache.sweep().await;
    assert_eq!(swept.removed, 1);
    assert_eq!(cache.get("bb02").await, None, "the oldest went");
    assert!(cache.get("aa01").await.is_some() && cache.get("cc03").await.is_some());
}

// ── TICKETS ───────────────────────────────────────────────────────────────────────────────────

fn narration(texts: &[&str]) -> Narration {
    Narration::prepare("af_heart", &units(texts)).unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_ticket_names_its_reader_until_it_lapses() {
    let tickets = Tickets::new(Duration::from_mins(10));
    let reader = Username::parse("Ada").unwrap();
    let id = tickets.issue(reader.clone(), narration(&["Hello."]));
    let (owner, said) = tickets.redeem(&id).expect("live");
    assert_eq!((owner, said.units.clone()), (reader, units(&["Hello."])));
    assert!(
        tickets.redeem(&id).is_some(),
        "redeemable more than once — browsers fetch twice"
    );
    assert!(tickets.redeem("not-a-ticket").is_none());
    tokio::time::advance(Duration::from_mins(11)).await;
    assert!(tickets.redeem(&id).is_none(), "lapsed");
}

// ── NARRATOR ──────────────────────────────────────────────────────────────────────────────────

/// Says every unit as its own text; fails any unit containing "FAIL". Records what it was asked.
#[derive(Default)]
struct FakeEngine {
    asked: Mutex<Vec<String>>,
}

impl SpeechEngine for FakeEngine {
    async fn synthesize(&self, _voice: Voice, text: &str) -> Result<Bytes, ListenError> {
        self.asked.lock().unwrap().push(text.to_owned());
        if text.contains("FAIL") {
            return Err(ListenError::EngineFailed("fake".to_owned()));
        }
        Ok(Bytes::from(format!("[{text}]")))
    }
}

fn narrator(dir: &std::path::Path) -> (Narrator<FakeEngine>, Arc<FakeEngine>) {
    let engine = Arc::new(FakeEngine::default());
    let narrator = Narrator {
        engine: Arc::clone(&engine),
        cache: Arc::new(AudioCache::new(dir, 16)),
        synth_gate: Arc::new(Semaphore::new(1)),
    };
    (narrator, engine)
}

/// Drain a stream into (the bytes it produced, whether it ended in an error).
async fn drain(narrator: Narrator<FakeEngine>, said: Narration) -> (String, bool) {
    let admission = Arc::new(Admission::new(1, 1));
    let hold = admission.admit("ada").unwrap();
    let mut stream = Box::pin(narrator.stream(Arc::new(said), 0, hold));
    let mut out = String::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => out.push_str(std::str::from_utf8(&bytes).unwrap()),
            Err(_) => return (out, true),
        }
    }
    drop(stream);
    assert_eq!(
        admission.in_flight(),
        0,
        "the place is given back when the stream is dropped"
    );
    (out, false)
}

#[tokio::test]
async fn units_play_in_order_and_a_repeat_comes_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (narrator, engine) = narrator(dir.path());
    let (out, failed) = drain(narrator.clone(), narration(&["One.", "Two."])).await;
    assert_eq!((out.as_str(), failed), ("[One.][Two.]", false));
    let (again, _) = drain(narrator, narration(&["One.", "Two."])).await;
    assert_eq!(again, "[One.][Two.]");
    assert_eq!(
        engine.asked.lock().unwrap().len(),
        2,
        "the second listen never reached the engine"
    );
}

#[tokio::test]
async fn one_failed_unit_is_skipped_and_the_lesson_goes_on() {
    let dir = tempfile::tempdir().unwrap();
    let (narrator, _) = narrator(dir.path());
    let (out, failed) = drain(narrator, narration(&["One.", "FAIL", "Three."])).await;
    assert_eq!((out.as_str(), failed), ("[One.][Three.]", false));
}

#[tokio::test]
async fn two_failures_in_a_row_end_the_stream_with_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (narrator, engine) = narrator(dir.path());
    let (out, failed) = drain(narrator, narration(&["One.", "FAIL a", "FAIL b", "Four."])).await;
    assert_eq!((out.as_str(), failed), ("[One.]", true));
    assert!(
        !engine.asked.lock().unwrap().contains(&"Four.".to_owned()),
        "nothing is synthesised after giving up"
    );
}

// ── MP3 TIMING ────────────────────────────────────────────────────────────────────────────────

/// A run of `count` Layer III frames with the given header, each zero after its header.
fn frames(header: [u8; 4], count: usize) -> Vec<u8> {
    let (length, _) = super::mp3::frame(header).expect("a valid header");
    let mut one = vec![0_u8; length];
    one[..4].copy_from_slice(&header);
    one.repeat(count)
}

#[test]
fn an_mpeg1_frame_is_1152_samples_and_the_length_its_bitrate_says() {
    // 128 kbit/s, 44.1 kHz: 144 × 128000 / 44100 = 417 bytes.
    let (length, secs) = super::mp3::frame([0xFF, 0xFB, 0x90, 0x64]).unwrap();
    assert_eq!(length, 417);
    assert!((secs - 1152.0 / 44_100.0).abs() < 1e-9);
}

#[test]
fn an_mpeg2_frame_is_576_samples() {
    // What the engine emits: MPEG-2, 128 kbit/s, 24 kHz — 72 × 128000 / 24000 = 384 bytes.
    let (length, secs) = super::mp3::frame([0xFF, 0xF3, 0xC4, 0xC4]).unwrap();
    assert_eq!(length, 384);
    assert!((secs - 0.024).abs() < 1e-9);
}

#[test]
fn not_a_header_is_not_a_frame() {
    assert_eq!(super::mp3::frame([0x49, 0x44, 0x33, 0x04]), None, "an ID3 tag");
    assert_eq!(super::mp3::frame([0xFF, 0xFD, 0x90, 0x64]), None, "Layer II");
    assert_eq!(
        super::mp3::frame([0xFF, 0xFB, 0xF0, 0x64]),
        None,
        "bitrate index 15"
    );
}

#[test]
fn a_duration_counts_frames_and_skips_tags_and_junk() {
    let mut audio = b"ID3\x04\x00\x00\x00\x00\x00\x05abcde".to_vec(); // a 15-byte tag
    audio.extend(frames([0xFF, 0xF3, 0xC4, 0xC4], 125)); // 125 × 24 ms = 3 s
    audio.extend(b"junk");
    audio.extend(frames([0xFF, 0xF3, 0xC4, 0xC4], 25)); // + 0.6 s
    assert!((super::mp3::duration_secs(&audio) - 3.6).abs() < 1e-9);
    assert!(super::mp3::duration_secs(&[]).abs() < f64::EPSILON);
}

#[tokio::test]
async fn marks_report_cached_units_and_stop_at_the_first_one_not_yet_said() {
    let dir = tempfile::tempdir().unwrap();
    let (narrator, engine) = narrator(dir.path());
    let said = narration(&["One.", "Two.", "Three."]);
    let one_second = frames([0xFF, 0xF3, 0xC4, 0xC4], 125);
    for text in ["One.", "Two."] {
        let key = cache_key(said.voice, text);
        narrator.cache.put(&key, Bytes::from(one_second.repeat(1))).await;
    }
    let durations = narrator.durations(&said, 0, 64).await;
    assert_eq!(durations.len(), 2, "Three. is not synthesised yet");
    assert!(durations.iter().all(|d| (d - 3.0).abs() < 1e-9));
    assert_eq!(
        narrator.durations(&said, 1, 1).await.len(),
        1,
        "from and limit are honoured"
    );
    assert!(
        engine.asked.lock().unwrap().is_empty(),
        "marks never reach the engine"
    );
}

#[tokio::test(start_paused = true)]
async fn using_a_ticket_renews_it() {
    let tickets = Tickets::new(Duration::from_mins(10));
    let id = tickets.issue(Username::parse("ada").unwrap(), narration(&["Hello."]));
    tokio::time::advance(Duration::from_mins(8)).await;
    assert!(tickets.redeem(&id).is_some());
    tokio::time::advance(Duration::from_mins(8)).await;
    assert!(
        tickets.redeem(&id).is_some(),
        "16 minutes in, but used 8 minutes ago"
    );
    tokio::time::advance(Duration::from_mins(11)).await;
    assert!(tickets.redeem(&id).is_none());
}
