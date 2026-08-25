//! Querying: what matches, what wins, and what refuses to blow up.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::index::{doc, index_of};
use crate::catalog::domain::search::*;

#[test]
fn several_words_narrow_rather_than_widen() {
    let index = index_of(vec![
        doc("Windows", "A lesson about windows only."),
        doc("Functions", "A lesson about functions only."),
        doc("Both", "This covers window functions together."),
    ]);
    let hits = index.search("window functions", 10);
    assert_eq!(hits.len(), 1, "every term must match");
    assert_eq!(hits[0].title, "Both");
}

/// The whole point of a palette: results appear before the word is finished.
#[test]
fn the_last_term_matches_by_prefix() {
    let index = index_of(vec![doc("Window Functions", "Prose about frames.")]);
    assert_eq!(
        index.search("windo", 10).len(),
        1,
        "a partial word still finds it"
    );
    assert_eq!(
        index.search("window func", 10).len(),
        1,
        "the last term is the partial one"
    );
}

/// Only the LAST term is a prefix — an earlier partial word would silently widen a query the
/// reader believes they have narrowed.
#[test]
fn an_earlier_term_is_matched_exactly() {
    let index = index_of(vec![doc("Window Functions", "Prose about frames.")]);
    assert!(
        index.search("windo functions", 10).is_empty(),
        "a leading partial does not expand"
    );
}

/// A term everybody uses carries no information, so a multi-word query must be decided by its
/// RARE term. This is why there is no stopword list: inverse document frequency already reduces
/// "the" to noise, and a list would additionally have broken every phrase containing it.
///
/// Both documents match `the quorum`. One is stuffed with the common word, the other with the
/// rare one — without idf the stuffed document wins on sheer count.
#[test]
fn a_multi_term_query_is_decided_by_its_rare_term() {
    let mut docs: Vec<DocInput> = (0..20)
        .map(|i| doc(&format!("Lesson {i}"), "the system the design the data"))
        .collect();
    docs.push(doc("Common Heavy", "the the the the the the the the quorum"));
    docs.push(doc("Rare Heavy", "quorum quorum quorum the"));
    let index = index_of(docs);

    let hits = index.search("the quorum", 10);
    assert_eq!(hits.len(), 2, "both contain both words");
    assert_eq!(hits[0].title, "Rare Heavy", "the informative term decides");
}

/// A word in every document still answers — it just stops discriminating. Dropping it at index
/// time is what would have made it un-findable.
#[test]
fn a_ubiquitous_term_still_matches() {
    let docs: Vec<DocInput> = (0..20)
        .map(|i| doc(&format!("Lesson {i}"), "the system the design"))
        .collect();
    let index = index_of(docs);
    assert!(index.search("the", 50).len() > 1);
}

#[test]
fn the_limit_is_respected() {
    let docs: Vec<DocInput> = (0..30)
        .map(|i| doc(&format!("Lesson {i}"), "shared term here"))
        .collect();
    let index = index_of(docs);
    assert_eq!(index.search("shared", 5).len(), 5);
    assert_eq!(index.search("shared", 0).len(), 0);
}

#[test]
fn a_query_with_no_terms_answers_nothing() {
    let index = index_of(vec![doc("Anything", "Some prose.")]);
    for query in ["", "   ", "!!!", "-- ,. --"] {
        assert!(index.search(query, 10).is_empty(), "query: {query:?}");
    }
}

#[test]
fn a_query_matching_nothing_answers_nothing() {
    let index = index_of(vec![doc("Anything", "Some prose.")]);
    assert!(index.search("zzzznotpresent", 10).is_empty());
}

/// A pasted paragraph must not become a thousand-term intersection.
#[test]
fn an_absurd_query_is_truncated_rather_than_refused() {
    let index = index_of(vec![doc("Anything", "alpha beta gamma delta")]);
    let long = (0..500).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
    assert!(index.search(&long, 10).is_empty(), "it answers; it does not hang");
}

/// A one-character query would expand to most of the vocabulary, so it stays exact.
#[test]
fn a_single_character_query_does_not_fan_out() {
    let index = index_of(vec![
        doc("C Language", "A lesson about C."),
        doc("Caching", "Prose about caches."),
    ]);
    let hits = index.search("c", 10);
    assert!(
        hits.iter().all(|h| h.title == "C Language"),
        "a lone character matches itself, not everything starting with it"
    );
}

#[test]
fn results_come_back_in_descending_score() {
    let index = index_of(vec![
        doc("Passing Mention", "We note sharding once."),
        doc("Sharding", "Sharding, sharding, and more sharding."),
    ]);
    let hits = index.search("sharding", 10);
    assert_eq!(hits.len(), 2);
    assert!(hits[0].score >= hits[1].score);
    assert_eq!(hits[0].title, "Sharding");
}

/// Terms sitting together beat the same terms scattered — the document is about the phrase.
#[test]
fn adjacent_terms_beat_scattered_ones() {
    let scattered = format!("window {} function", "filler ".repeat(150));
    let index = index_of(vec![
        doc("Scattered", &scattered),
        doc("Adjacent", "the window function clause explained"),
    ]);
    let hits = index.search("window function", 10);
    assert_eq!(hits[0].title, "Adjacent");
}
