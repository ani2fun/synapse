//! `LessonFileRef` derivation. One theme: a sidecar is reached from its lesson, and the SOURCE
//! comes along. Deriving a sidecar as a bare string would let two sources with the same interior
//! layout cross-serve each other's editorials and judge suites.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn lesson(path: &str) -> LessonFileRef {
    LessonFileRef::new("java", path)
}

#[test]
fn a_stem_sidecar_replaces_the_md_suffix() {
    let file = lesson("02-arrays/01-sum/sum.md");
    assert_eq!(
        file.sidecar(".tests.json").path,
        "02-arrays/01-sum/sum.tests.json"
    );
    assert_eq!(
        file.sidecar(".editorial.md").path,
        "02-arrays/01-sum/sum.editorial.md"
    );
}

#[test]
fn a_stem_sidecar_keeps_the_order_prefix() {
    // Sidecars are matched on the REAL file stem, `NN-` included: `01-flip.md` pairs with
    // `01-flip.tests.json`, never `flip.tests.json`.
    let file = lesson("02-flip-characters/01-flip-characters.md");
    assert_eq!(
        file.sidecar(".tests.json").path,
        "02-flip-characters/01-flip-characters.tests.json"
    );
}

#[test]
fn a_neighbour_resolves_against_the_lessons_directory() {
    let file = lesson("06-case-studies/url-shortener.md");
    assert_eq!(
        file.neighbour("_c4-docs/rusApi.md").path,
        "06-case-studies/_c4-docs/rusApi.md"
    );
}

#[test]
fn a_neighbour_of_a_root_level_lesson_has_no_directory_to_prepend() {
    assert_eq!(
        lesson("index.md").neighbour("_c4-docs/x.md").path,
        "_c4-docs/x.md"
    );
}

#[test]
fn derivation_never_leaves_the_owning_source() {
    // The invariant the whole type exists for.
    let file = lesson("a/b.md");
    assert_eq!(file.sidecar(".tests.json").source_id, "java");
    assert_eq!(file.neighbour("_c4-docs/x.md").source_id, "java");
}
