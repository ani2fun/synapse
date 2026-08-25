//! `BookEntryDto` is internally tagged on `kind`, which puts it one field name away from
//! serializing the discriminator twice — silently, because the derive cannot see inside the
//! newtype. These tests are what says the tag still means exactly one thing.

#![allow(clippy::unwrap_used)]

use super::*;

fn lesson(kind: Option<&str>) -> BookEntryDto {
    BookEntryDto::Lesson(LessonDto {
        slug: "two-sum".to_owned(),
        title: "Two Sum".to_owned(),
        order: Some(1),
        essential: true,
        lesson_kind: kind.map(str::to_owned),
    })
}

/// `BookEntryDto` is internally tagged on `kind`, so a `LessonDto` field named `kind` would
/// serialize the key twice — silently, because the derive cannot see inside the newtype.
/// This is the test that says the discriminator still means exactly one thing.
#[test]
fn the_lesson_kind_field_does_not_collide_with_the_enum_tag() {
    let json = serde_json::to_string(&lesson(Some("problem"))).unwrap();
    assert_eq!(json.matches("\"kind\":").count(), 1, "duplicate tag in {json}");
    assert!(json.contains(r#""kind":"lesson""#), "{json}");
    assert!(json.contains(r#""lessonKind":"problem""#), "{json}");
}

#[test]
fn a_lesson_round_trips_through_the_wire() {
    let entry = lesson(Some("problem"));
    let json = serde_json::to_string(&entry).unwrap();
    assert_eq!(serde_json::from_str::<BookEntryDto>(&json).unwrap(), entry);
}

/// Prose is the common case and pays nothing: the key is absent, not null.
#[test]
fn prose_lessons_carry_no_kind_at_all() {
    let json = serde_json::to_string(&lesson(None)).unwrap();
    assert!(!json.contains("lessonKind"), "{json}");
    assert_eq!(serde_json::from_str::<BookEntryDto>(&json).unwrap(), lesson(None));
}
