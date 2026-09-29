use synapse_shared::notes::NOTE_MAX_UTF16;

use super::{NotesError, check_length};

#[test]
fn a_note_at_the_limit_is_accepted() {
    assert_eq!(check_length(&"x".repeat(NOTE_MAX_UTF16)), Ok(()));
    assert_eq!(check_length(""), Ok(()));
}

#[test]
fn a_note_past_the_limit_is_refused_with_its_length() {
    assert_eq!(
        check_length(&"x".repeat(NOTE_MAX_UTF16 + 1)),
        Err(NotesError::TooLong {
            len: NOTE_MAX_UTF16 + 1,
            max: NOTE_MAX_UTF16
        })
    );
}

/// The limit is the BROWSER's unit. An emoji outside the BMP is one `char` but two UTF-16 code
/// units, and the textarea counts it as two — so the server must too, or the two disagree about
/// which notes fit.
#[test]
fn the_limit_counts_utf16_code_units_like_the_textarea() {
    let astral = "😀".repeat(NOTE_MAX_UTF16 / 2 + 1);
    assert_eq!(
        astral.chars().count(),
        NOTE_MAX_UTF16 / 2 + 1,
        "under the limit by chars"
    );
    assert!(matches!(check_length(&astral), Err(NotesError::TooLong { .. })));
}
