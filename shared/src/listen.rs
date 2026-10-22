//! The listen wire contract — a lesson read aloud by the speech engine, for readers on the listen
//! allowlist.
//!
//! The client sends UNITS: short runs of whole sentences it has already cut from the lesson's
//! prose. The unit is what the engine synthesises and what the server caches, so it is kept short
//! — the engine's memory and its time-to-first-audio both grow with the length of one request,
//! and two readers playing the same lesson share every unit they have in common.

use serde::{Deserialize, Serialize};

/// The longest unit the server accepts, in characters. The client aims at ~300 and hard-splits on
/// a word boundary before this.
pub const LISTEN_MAX_UNIT_CHARS: usize = 400;

/// The most text one listen may carry, in characters — roughly ten thousand words, more than the
/// longest lesson in the catalog. "Listen from here" sends the rest of the lesson in one request,
/// because one request is one continuous audio stream, and one stream is what keeps playing on a
/// locked phone.
pub const LISTEN_MAX_REQUEST_CHARS: usize = 60_000;

/// The voices a reader may pick, `(engine id, label)`. The first is the default. The server
/// refuses any other id, so a new voice is one line here.
pub const LISTEN_VOICES: &[(&str, &str)] = &[
    ("af_heart", "Heart · US"),
    ("am_michael", "Michael · US"),
    ("bf_emma", "Emma · UK"),
    ("bm_george", "George · UK"),
];

/// One voice on offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ListenVoiceDto {
    pub id: String,
    pub label: String,
}

/// `GET /api/listen/config` — answered for everyone, because the lesson page asks before it knows
/// who is reading. An anonymous or unlisted reader gets `canListen: false` and the page loads
/// nothing. A deployment without a speech engine has no such route at all (404).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ListenConfigDto {
    pub can_listen: bool,
    pub voices: Vec<ListenVoiceDto>,
    pub max_unit_chars: usize,
    pub max_request_chars: usize,
}

/// `POST /api/listen` body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ListenRequestDto {
    /// One of [`LISTEN_VOICES`].
    pub voice: String,
    /// The text to read, in order, each at most [`LISTEN_MAX_UNIT_CHARS`].
    pub units: Vec<String>,
}

/// What a listen request bought: a short-lived address the `<audio>` element can play. An
/// `<audio src>` cannot send a bearer, so the bearer is spent HERE and the address carries a
/// one-off ticket instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ListenTicketDto {
    /// `/api/listen/{ticket}.mp3`.
    pub url: String,
    /// How many units the stream will read, after blank ones are dropped.
    pub units: usize,
    /// Seconds the address stays valid for STARTING playback. A stream already playing is not
    /// cut off when it lapses.
    pub expires_in: u64,
}
