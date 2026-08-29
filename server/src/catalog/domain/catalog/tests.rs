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
fn derivation_never_leaves_the_owning_source() {
    // The invariant the whole type exists for.
    let file = lesson("a/b.md");
    assert_eq!(file.sidecar(".tests.json").source_id, "java");
}
