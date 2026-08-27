use super::addressable;

/// The hash and the slug are joined to a path inside the renderer's cache directory, so this is a
/// traversal guard, not a tidiness check. It is deliberately stricter than "rejects `..`": the
/// alphabet is an allowlist, so a form nobody has thought of yet is refused by default.
#[test]
fn an_addressable_segment_is_a_hash_or_a_slug() {
    for value in ["47dbee4a", "root", "url-shortener", "a_b-2", "board"] {
        assert!(addressable(value), "{value} should address a board");
    }
}

#[test]
fn traversal_and_separators_are_refused() {
    for value in [
        "", ".", "..", "../etc", ".hidden", // a leading dot is what `.` and `..` have in common
        "a/b",     // a separator would reach out of the cache directory
        "a\\b", "a.svg", // the extension is stripped before this runs; a dot here is not a board
        "a b", "a%2fb", // percent-encoding is decoded by the router BEFORE this sees it
        "héllo",
    ] {
        assert!(!addressable(value), "{value:?} should not address a board");
    }
}

#[test]
fn an_over_long_segment_is_refused() {
    // Bounded so a request cannot become a very long path on the renderer's filesystem.
    assert!(addressable(&"a".repeat(128)));
    assert!(!addressable(&"a".repeat(129)));
}
