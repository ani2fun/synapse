//! Alias resolution for the languages a submission can name. Global uniqueness is the load-
//! bearing property: two languages sharing an alias would let iteration order pick the answer.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use super::*;

#[test]
fn resolves_canonical_and_secondary_aliases() {
    assert_eq!(Language::resolve("python"), Some(Language::Python));
    assert_eq!(Language::resolve("py"), Some(Language::Python));
    assert_eq!(Language::resolve("c++"), Some(Language::Cpp));
    assert_eq!(Language::resolve("node"), Some(Language::JavaScript));
}

#[test]
fn resolution_is_case_insensitive_and_trimmed() {
    assert_eq!(Language::resolve("  PyThOn3  "), Some(Language::Python));
    assert_eq!(Language::resolve("JAVA"), Some(Language::Java));
}

#[test]
fn unknown_and_blank_resolve_to_none() {
    assert_eq!(Language::resolve("cobol"), None);
    assert_eq!(Language::resolve("   "), None);
    assert_eq!(Language::resolve(""), None);
}

#[test]
fn aliases_are_globally_unique_and_round_trip() {
    let mut seen = BTreeSet::new();
    for lang in Language::ALL {
        assert!(!lang.label().is_empty());
        assert!(!lang.aliases().is_empty());
        for alias in lang.aliases() {
            assert!(seen.insert(*alias), "duplicate alias {alias}");
            assert_eq!(
                Language::resolve(alias),
                Some(lang),
                "alias {alias} must round-trip"
            );
        }
    }
}
