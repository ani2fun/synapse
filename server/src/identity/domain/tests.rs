//! One theme: two spellings of the same person are the same key, and no spelling of nobody is
//! a key at all.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashSet;

use super::*;

#[test]
fn every_spelling_of_a_name_canonicalises_to_one() {
    for raw in ["Ada", "ADA", "  ada  ", "aDa"] {
        assert_eq!(Username::parse(raw).unwrap().as_str(), "ada", "raw: {raw:?}");
    }
}

/// The property the three comparison surfaces actually depend on: a grant written from one
/// spelling is found by a caller who arrives under another.
#[test]
fn a_set_finds_a_member_registered_under_a_different_spelling() {
    let granted: HashSet<Username> = [" GRACE ", "Ada"]
        .iter()
        .filter_map(|r| Username::parse(r))
        .collect();
    assert_eq!(granted.len(), 2);
    assert!(granted.contains(&Username::parse("grace").unwrap()));
    assert!(granted.contains(&Username::parse("ADA").unwrap()));
    assert!(!granted.contains(&Username::parse("hopper").unwrap()));
}

#[test]
fn a_name_that_is_only_whitespace_is_nobody() {
    for raw in ["", "   ", "\t\n"] {
        assert!(Username::parse(raw).is_none(), "raw: {raw:?}");
    }
}

/// Display is what the audit line and the 403's detail print; it must not leak the wrapper.
#[test]
fn display_is_the_canonical_name_alone() {
    let name = Username::parse(" Tester ").unwrap();
    assert_eq!(name.to_string(), "tester");
    assert_eq!(format!("'{name}' is not an admin"), "'tester' is not an admin");
}

#[test]
fn into_string_hands_back_the_canonical_form_for_the_wire() {
    assert_eq!(Username::parse("TeStEr").unwrap().into_string(), "tester");
}
