//! `Range` parsing for media responses: one bounded range is honoured, and anything malformed
//! or past the end falls back to serving the whole file.

#![allow(clippy::unwrap_used)]

use super::parse_range;

#[test]
fn parses_a_single_bounded_range() {
    assert_eq!(parse_range("bytes=0-4", 10), Some((0, 4)));
    assert_eq!(parse_range("bytes=5-", 10), Some((5, 9)));
}

#[test]
fn out_of_bounds_or_malformed_falls_back_to_the_full_response() {
    assert_eq!(parse_range("bytes=5-20", 10), None);
    assert_eq!(parse_range("bytes=7-5", 10), None);
    assert_eq!(parse_range("bites=0-4", 10), None);
    assert_eq!(parse_range("bytes=0-4", 0), None);
}
