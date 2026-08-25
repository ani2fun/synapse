//! Parsing a component doc's leading metadata fence. The absent fence is the common case and
//! yields the full body untouched rather than an error.

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn parses_metadata_and_body() {
    let doc = ComponentDoc::parse("---\ntitle: Reader\nkind: component\ntechnology: Laminar\n---\nThe body.");
    assert_eq!(doc.title.as_deref(), Some("Reader"));
    assert_eq!(doc.kind.as_deref(), Some("component"));
    assert_eq!(doc.technology.as_deref(), Some("Laminar"));
    assert_eq!(doc.body, "The body.");
}

#[test]
fn absent_fence_means_all_none_and_full_body() {
    let doc = ComponentDoc::parse("Just prose.");
    assert_eq!(doc.title, None);
    assert_eq!(doc.kind, None);
    assert_eq!(doc.technology, None);
    assert_eq!(doc.body, "Just prose.");
}
