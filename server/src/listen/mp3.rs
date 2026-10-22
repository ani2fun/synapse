//! How long a unit's MP3 plays, read from its frame headers.
//!
//! The page follows along — it highlights the sentence being spoken — and to do that it needs to
//! know where each unit starts on the stream's clock. The stream is units laid end to end, so a
//! unit starts where the durations of the ones before it add up to, and a duration is the frame
//! count times the samples per frame over the sample rate. Counting frames rather than dividing
//! bytes by a bitrate is what makes this right for a variable-bitrate encode too.
//!
//! Only Layer III is recognised; the engine emits nothing else. Each unit also carries its own
//! `ID3v2` tag (the encoder writes one), which is skipped wherever it sits. Bytes that are neither a
//! tag nor a frame are stepped over one at a time until the next frame header, which is what a
//! decoder does too.

/// Kbit/s by bitrate index, Layer III.
const MPEG1_KBPS: [u32; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const MPEG2_KBPS: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

/// One Layer III frame header: its length in bytes and the seconds of audio it carries, or `None`
/// when the four bytes are not a valid header.
pub fn frame(header: [u8; 4]) -> Option<(usize, f64)> {
    let [sync, b1, b2, _] = header;
    if sync != 0xFF || b1 & 0xE0 != 0xE0 {
        return None;
    }
    // Version: 3 = MPEG-1, 2 = MPEG-2, 0 = MPEG-2.5, 1 = reserved. Layer bits 01 = Layer III.
    let version = (b1 >> 3) & 0b11;
    if version == 1 || (b1 >> 1) & 0b11 != 0b01 {
        return None;
    }
    let bitrate_index = usize::from(b2 >> 4);
    let rate_index = usize::from((b2 >> 2) & 0b11);
    if bitrate_index == 0 || bitrate_index == 15 || rate_index == 3 {
        return None;
    }
    let padding = usize::from((b2 >> 1) & 1);
    let (kbps, rates, samples): (u32, [u32; 3], u32) = match version {
        3 => (MPEG1_KBPS[bitrate_index], [44_100, 48_000, 32_000], 1152),
        2 => (MPEG2_KBPS[bitrate_index], [22_050, 24_000, 16_000], 576),
        _ => (MPEG2_KBPS[bitrate_index], [11_025, 12_000, 8_000], 576),
    };
    let rate = rates[rate_index];
    // bytes = samples/8 × bitrate / sample rate, plus the padding byte.
    let length = usize::try_from(samples / 8 * kbps * 1000 / rate).ok()? + padding;
    Some((length, f64::from(samples) / f64::from(rate)))
}

/// The `ID3v2` tag at the start of `bytes`, as its total length — header, body and footer.
fn id3_length(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 10 || !bytes.starts_with(b"ID3") {
        return None;
    }
    // A synchsafe integer: four bytes of seven bits each.
    let body = bytes[6..10]
        .iter()
        .fold(0_usize, |size, byte| (size << 7) | usize::from(byte & 0x7F));
    let footer = if bytes[5] & 0x10 == 0 { 0 } else { 10 };
    Some(10 + body + footer)
}

/// Seconds of audio in `bytes`.
pub fn duration_secs(bytes: &[u8]) -> f64 {
    let mut at = 0;
    let mut seconds = 0.0;
    while at + 4 <= bytes.len() {
        if let Some(tag) = id3_length(&bytes[at..]) {
            at += tag;
            continue;
        }
        let header = [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]];
        match frame(header) {
            Some((length, frame_secs)) if at + length <= bytes.len() => {
                seconds += frame_secs;
                at += length;
            }
            _ => at += 1,
        }
    }
    seconds
}
