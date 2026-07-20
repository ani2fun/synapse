//! Reading progress (step 51) — pure, so `cargo test` covers it natively.
//!
//! Two facts are kept, and they are deliberately in SEPARATE localStorage keys rather than one
//! packed record: the set of lessons finished, and the last lesson opened. `prefs.rs` packs four
//! fields into one `|`-joined string parsed by an exact-arity slice pattern
//! (`let [s, l, f, w] = … else { return DEFAULT_PREFS }`), which means adding a fifth field
//! silently resets every existing reader's saved settings. That trap is worth not rebuilding:
//! one key, one job, and a key that fails to parse costs only itself.
//!
//! The done-set is newline-separated because it is a LIST, not a fixed record — there is no
//! arity to get wrong, and an unrecognised or blank line is skipped rather than poisoning the
//! rest. Lesson paths are `/`-joined slugs, so they can contain neither a newline nor a `|`.

use std::collections::BTreeSet;

use synapse_shared::catalog::BookDto;

use super::reading_order;

/// How far down a lesson counts as "read to the end". Not 1.0: the last pixel is unreachable
/// on many devices (rubber-banding, a footer inside the scroll container, sub-pixel rounding),
/// and a threshold nobody can cross is a feature nobody has.
pub const END_THRESHOLD: f64 = 0.98;

/// Has the reader reached the end, given the scroll offset and the scrollable track?
///
/// Lives here rather than inline in `ChromeState::recompute` because it is the load-bearing
/// decision of the whole feature and the view layer cannot be tested — the preview environment
/// reports `innerHeight: 0`, so the scroll path is not exercisable in a browser at all.
///
/// Both traps are handled explicitly:
/// - `track <= 0` means the lesson is SHORTER than the viewport. A naive `scroll / track`
///   ratio pins at 0.0 there and the reader can never finish a short lesson. There is nothing
///   to scroll, which is precisely the case where it has all been seen.
/// - a non-finite ratio (0/0) is not "at the end" — it is a page that has not laid out yet.
pub fn is_at_end(scroll: f64, track: f64) -> bool {
    if track <= 0.0 {
        return true;
    }
    let ratio = scroll / track;
    ratio.is_finite() && ratio >= END_THRESHOLD
}

/// Read the completed set. Absent or unreadable storage is an empty set — never an error, and
/// never a partial parse: progress is a convenience, and losing it must not break the reader.
pub fn parse(stored: Option<&str>) -> BTreeSet<String> {
    stored
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `BTreeSet` so the serialised form is stable — an unordered set would rewrite the whole value
/// on every commit and make the stored string churn for no reason.
pub fn serialize(done: &BTreeSet<String>) -> String {
    done.iter().cloned().collect::<Vec<_>>().join("\n")
}

/// How many of a book's lessons are finished. Counts against `reading_order`, which is the same
/// list the sidebar and the card already use, so the denominator can never disagree with what
/// the reader can see.
pub fn completed_count(book: &BookDto, done: &BTreeSet<String>) -> usize {
    reading_order(book)
        .iter()
        .filter(|(path, _)| done.contains(path))
        .count()
}

/// The first unfinished lesson of a book, in reading order — `None` when the book is finished.
pub fn next_unread(book: &BookDto, done: &BTreeSet<String>) -> Option<String> {
    reading_order(book)
        .into_iter()
        .find(|(path, _)| !done.contains(path))
        .map(|(path, _)| path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use synapse_shared::catalog::{BookEntryDto, LessonDto};

    fn lesson(slug: &str) -> BookEntryDto {
        BookEntryDto::Lesson(LessonDto {
            slug: slug.to_owned(),
            title: slug.to_owned(),
            order: None,
            essential: true,
            lesson_kind: None,
        })
    }

    fn book() -> BookDto {
        BookDto {
            slug: "dsa".to_owned(),
            title: "DSA".to_owned(),
            description: String::new(),
            tags: vec![],
            estimated_reading_minutes: None,
            order: None,
            category_path: vec!["learn".to_owned()],
            entries: vec![lesson("intro"), lesson("arrays"), lesson("lists")],
        }
    }

    fn set(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn absent_or_blank_storage_is_an_empty_set() {
        assert!(parse(None).is_empty());
        assert!(parse(Some("")).is_empty());
        assert!(
            parse(Some("\n\n  \n")).is_empty(),
            "blank lines are skipped, not stored"
        );
    }

    #[test]
    fn a_round_trip_preserves_the_set() {
        let done = set(&["learn/dsa/arrays", "learn/dsa/intro"]);
        assert_eq!(parse(Some(&serialize(&done))), done);
    }

    #[test]
    fn the_serialised_form_is_stable_across_insertion_orders() {
        let a = set(&["b", "a", "c"]);
        let b = set(&["c", "b", "a"]);
        assert_eq!(
            serialize(&a),
            serialize(&b),
            "a BTreeSet keeps the stored string from churning on every commit"
        );
    }

    #[test]
    fn a_stray_line_costs_only_itself() {
        // The whole reason this is a list and not a positional record: garbage in the middle
        // does not take the rest of the value down with it (cf. prefs.rs).
        let done = parse(Some("learn/dsa/intro\n\n   \nlearn/dsa/arrays\n"));
        assert_eq!(done.len(), 2);
        assert!(done.contains("learn/dsa/intro"));
        assert!(done.contains("learn/dsa/arrays"));
    }

    #[test]
    fn counting_uses_full_paths_not_slugs() {
        let done = set(&["learn/dsa/intro", "learn/dsa/arrays"]);
        assert_eq!(completed_count(&book(), &done), 2);
        // A bare slug must NOT count — two books can both have an `intro`.
        assert_eq!(completed_count(&book(), &set(&["intro"])), 0);
    }

    #[test]
    fn a_path_from_another_book_does_not_inflate_the_count() {
        assert_eq!(completed_count(&book(), &set(&["learn/python/intro"])), 0);
    }

    #[test]
    fn a_lesson_shorter_than_the_viewport_counts_as_read() {
        // The trap: `scroll / track` pins at 0.0 when there is nothing to scroll, so a naive
        // threshold means a short lesson can never be finished.
        assert!(is_at_end(0.0, 0.0));
        assert!(is_at_end(0.0, -120.0), "a negative track is the same case");
    }

    #[test]
    fn the_end_is_just_short_of_the_bottom() {
        let track = 1000.0;
        assert!(!is_at_end(0.0, track));
        assert!(!is_at_end(970.0, track), "97% is not there yet");
        assert!(
            is_at_end(980.0, track),
            "98% counts — the last pixel is unreachable"
        );
        assert!(is_at_end(1000.0, track));
        assert!(is_at_end(1200.0, track), "overscroll must not un-finish it");
    }

    #[test]
    fn a_page_that_has_not_laid_out_is_not_finished() {
        assert!(!is_at_end(f64::NAN, 1000.0));
        assert!(!is_at_end(1.0, f64::INFINITY));
    }

    #[test]
    fn next_unread_walks_reading_order_and_ends_at_none() {
        let b = book();
        assert_eq!(next_unread(&b, &BTreeSet::new()).unwrap(), "learn/dsa/intro");
        assert_eq!(
            next_unread(&b, &set(&["learn/dsa/intro"])).unwrap(),
            "learn/dsa/arrays",
            "resumes at the first GAP, not after the last finished lesson"
        );
        // Out-of-order reading resumes at the gap, which is the point of using the set.
        assert_eq!(
            next_unread(&b, &set(&["learn/dsa/arrays"])).unwrap(),
            "learn/dsa/intro"
        );
        let all = set(&["learn/dsa/intro", "learn/dsa/arrays", "learn/dsa/lists"]);
        assert_eq!(next_unread(&b, &all), None, "a finished book has nothing next");
    }
}
