//! Full-text search over the merged library — the index and the ranking, both pure.
//!
//! Search used to see a lesson's title and its ancestors' titles and nothing else, so a concept
//! discussed at length in prose but never named in a heading was unreachable. This indexes the
//! bodies, across every mounted source.
//!
//! **It can only ever contain what the catalog already serves.** Documents arrive from the walk's
//! `lesson_files`, so `local-only-content/`, `_`-prefixed files and the losing copy of a
//! duplicated book slug are absent here for the same reason they are absent from the site — not
//! by a second exclusion rule that could drift from the first.
//!
//! **Ranking is BM25-shaped, and the idf half is load-bearing rather than decorative:** a term in
//! every document scores near nothing, which is what makes a stopword list unnecessary. Stopword
//! lists are a trap here — drop "the" at index time and the phrase "the go scheduler" quietly
//! stops working, with nothing to tell a reader why.

mod documents;
mod snippet;
mod text;

use std::collections::BTreeMap;
use std::ops::Bound;

pub use documents::index_of;
pub use snippet::Segment;

/// Saturation: how fast repeating a term stops helping. The BM25 default.
const K1: f32 = 1.2;
/// How much a document's length is held against it. Below the 0.75 default on purpose — this
/// corpus runs from 2 KB lessons to a 181 KB chapter, and aggressive normalisation would bury the
/// long design chapters that are often the right answer.
const B: f32 = 0.6;
/// A prefix must be at least this long before it expands, or one keystroke fans out to thousands
/// of terms.
const MIN_PREFIX: usize = 2;
/// Ceiling on how many terms one prefix may expand to.
const MAX_EXPANSIONS: usize = 64;
/// Ceiling on query terms, so a pasted paragraph cannot become a thousand-term intersection.
const MAX_TERMS: usize = 8;
/// Candidates carried into the snippet pass, as a multiple of the caller's limit — wide enough
/// that the proximity adjustment can still reorder the top.
const CANDIDATE_FACTOR: usize = 3;

/// A count as a float.
///
/// `f32` represents every integer below 2^24 — sixteen million — exactly. Document counts, token
/// counts and term lengths here are four orders of magnitude under that, so this is lossless for
/// every value the index can hold; the lint is warning about 64-bit inputs that cannot occur.
#[allow(clippy::cast_precision_loss)]
fn count(n: usize) -> f32 {
    n as f32
}

/// What a reader is looking at when they find a hit. An editorial is a problem's SOLUTION, so the
/// distinction has to survive to the surface: stumbling onto one spoils the exercise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Lesson,
    Editorial,
}

/// Where a term was found. The spread is wide because a title IS the document's claim about
/// itself, while a body mention may be an aside and a code mention may be incidental.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Title,
    Heading,
    Crumb,
    Summary,
    Body,
    Code,
}

/// One document to index, as the walk hands it over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocInput {
    pub title: String,
    /// Ancestor titles, outermost first — the category and book a reader sees under a result.
    pub breadcrumb: Vec<String>,
    /// The lesson's URL path, no leading slash.
    pub url: String,
    pub kind: DocKind,
    pub book_slug: String,
    pub source_id: String,
    /// Frontmatter `summary:`, when the lesson carries one.
    pub summary: Option<String>,
    /// The raw markdown body.
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Doc {
    title: String,
    breadcrumb: Vec<String>,
    url: String,
    kind: DocKind,
    book_slug: String,
    source_id: String,
    /// Token count, for length normalisation.
    len: u32,
    /// Markdown flattened to prose, kept so a snippet can quote the reader's own words back.
    /// Positions are NOT stored: scanning this for the handful of documents that survive ranking
    /// is cheaper than carrying every offset for all of them.
    text: String,
}

/// One document's occurrences of one term, per field.
///
/// A single posting per (term, document) rather than one per field: it makes the weighted sum a
/// direct read, and it lets the build loop append in O(1) because documents arrive in ascending
/// id, so the posting being updated is always the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Posting {
    doc: u32,
    title: u16,
    heading: u16,
    crumb: u16,
    summary: u16,
    body: u16,
    code: u16,
}

impl Posting {
    /// A posting for a term's FIRST occurrence in one document.
    fn first(doc: u32, field: Field) -> Self {
        let mut posting = Self {
            doc,
            ..Self::default()
        };
        posting.bump(field, 1);
        posting
    }

    fn bump(&mut self, field: Field, by: u16) {
        let slot = match field {
            Field::Title => &mut self.title,
            Field::Heading => &mut self.heading,
            Field::Crumb => &mut self.crumb,
            Field::Summary => &mut self.summary,
            Field::Body => &mut self.body,
            Field::Code => &mut self.code,
        };
        *slot = slot.saturating_add(by);
    }

    /// The weighted term frequency this posting contributes.
    fn weighted(self) -> f32 {
        6.0 * f32::from(self.title)
            + 3.0 * f32::from(self.heading)
            + 2.0 * f32::from(self.crumb)
            + 2.0 * f32::from(self.summary)
            + 1.0 * f32::from(self.body)
            + 0.3 * f32::from(self.code)
    }
}

/// What a search returns, owned — results are few and the caller maps them straight to the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub title: String,
    pub breadcrumb: Vec<String>,
    pub url: String,
    pub kind: DocKind,
    pub book_slug: String,
    pub source_id: String,
    /// The quote, already split into matched and unmatched runs.
    pub snippet: Vec<Segment>,
    pub score: f32,
}

#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    docs: Vec<Doc>,
    /// A `BTreeMap`, and the ordering is load-bearing: `score_prefix` walks `range` from the
    /// prefix, so sorted order IS the prefix scan. A hash map would be a faster build and no
    /// as-you-type search at all.
    terms: BTreeMap<Box<str>, Vec<Posting>>,
    total_len: usize,
}

#[derive(Debug, Default)]
pub struct IndexBuilder {
    index: SearchIndex,
}

impl SearchIndex {
    #[must_use]
    pub fn builder() -> IndexBuilder {
        IndexBuilder::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Rank documents against a query.
    ///
    /// Every term must match — with several words a reader is narrowing, not widening. The LAST
    /// term matches by prefix so results appear while typing, which is the whole point of a
    /// palette: `windo` should find `window` before the word is finished.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let mut terms = text::tokenize(query);
        terms.truncate(MAX_TERMS);
        if terms.is_empty() || limit == 0 || self.docs.is_empty() {
            return Vec::new();
        }

        let mut scores = vec![0.0_f32; self.docs.len()];
        let mut matched = vec![0_u32; self.docs.len()];
        let split = terms.len().saturating_sub(1);
        for term in &terms[..split] {
            self.score_exact(term, &mut scores, &mut matched);
        }
        if let Some(last) = terms.last() {
            self.score_prefix(last, &mut scores, &mut matched);
        }

        let wanted = u32::try_from(terms.len()).unwrap_or(u32::MAX);
        let mut ranked: Vec<(usize, f32)> = matched
            .iter()
            .enumerate()
            .filter(|&(_, count)| *count == wanted)
            .filter_map(|(doc, _)| scores.get(doc).map(|score| (doc, *score)))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        ranked.truncate(limit.saturating_mul(CANDIDATE_FACTOR));

        let mut hits: Vec<SearchHit> = ranked
            .into_iter()
            .filter_map(|(doc, score)| self.hit(doc, &terms, score))
            .collect();
        // The snippet pass knows something ranking cannot: how close the words actually sit. Two
        // documents mentioning "window" and "function" paragraphs apart lose to one that says
        // "window function".
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.title.len().cmp(&b.title.len()))
        });
        hits.truncate(limit);
        hits
    }

    fn score_exact(&self, term: &str, scores: &mut [f32], matched: &mut [u32]) {
        let Some(postings) = self.terms.get(term) else {
            return;
        };
        self.apply(postings, 1.0, scores, matched);
    }

    /// The as-you-type half. A prefix can expand to many terms; each document takes its BEST
    /// expansion rather than their sum, or a page repeating `window`, `windowing` and `windows`
    /// would outrank the page actually about windows. Longer expansions are discounted, so an
    /// exact word still beats a distant relative.
    fn score_prefix(&self, prefix: &str, scores: &mut [f32], matched: &mut [u32]) {
        if prefix.chars().count() < MIN_PREFIX {
            self.score_exact(prefix, scores, matched);
            return;
        }
        let mut best: BTreeMap<u32, f32> = BTreeMap::new();
        // An unbounded range from the prefix, walked until the keys stop sharing it — sorted-map
        // order IS the prefix scan, which is why the terms live in a `BTreeMap`.
        let from = (Bound::Included(prefix), Bound::Unbounded);
        for (term, postings) in self
            .terms
            .range::<str, _>(from)
            .take_while(|(term, _)| term.starts_with(prefix))
            .take(MAX_EXPANSIONS)
        {
            let weight = count(prefix.len()) / count(term.len().max(1));
            let idf = self.idf(postings.len());
            for posting in postings {
                let value = idf * self.saturate(posting) * weight;
                best.entry(posting.doc)
                    .and_modify(|current| *current = current.max(value))
                    .or_insert(value);
            }
        }
        for (doc, value) in best {
            let doc = doc as usize;
            if let Some(score) = scores.get_mut(doc) {
                *score += value;
            }
            if let Some(count) = matched.get_mut(doc) {
                *count += 1;
            }
        }
    }

    fn apply(&self, postings: &[Posting], weight: f32, scores: &mut [f32], matched: &mut [u32]) {
        let idf = self.idf(postings.len());
        for posting in postings {
            let doc = posting.doc as usize;
            if let Some(score) = scores.get_mut(doc) {
                *score += idf * self.saturate(posting) * weight;
            }
            if let Some(count) = matched.get_mut(doc) {
                *count += 1;
            }
        }
    }

    /// Term saturation with length normalisation. Repeating a word helps less each time, and a
    /// long document does not win merely by being long.
    fn saturate(&self, posting: &Posting) -> f32 {
        let wtf = posting.weighted();
        let len = self
            .docs
            .get(posting.doc as usize)
            .map_or(1.0, |doc| f32::from(u16::try_from(doc.len).unwrap_or(u16::MAX)));
        let average = self.average_len();
        wtf / (wtf + K1 * (1.0 - B + B * (len / average)))
    }

    fn average_len(&self) -> f32 {
        let docs = count(self.docs.len().max(1));
        (count(self.total_len) / docs).max(1.0)
    }

    /// Inverse document frequency. A term in every document carries no information and lands near
    /// zero — which is how "the" stops mattering without anyone maintaining a list.
    fn idf(&self, df: usize) -> f32 {
        let total = count(self.docs.len().max(1));
        let df = count(df.max(1));
        (1.0 + (total - df + 0.5) / (df + 0.5)).ln()
    }

    fn hit(&self, doc: usize, terms: &[String], score: f32) -> Option<SearchHit> {
        let found = self.docs.get(doc)?;
        let excerpt = snippet::excerpt(&found.text, terms);
        Some(SearchHit {
            title: found.title.clone(),
            breadcrumb: found.breadcrumb.clone(),
            url: found.url.clone(),
            kind: found.kind,
            book_slug: found.book_slug.clone(),
            source_id: found.source_id.clone(),
            snippet: excerpt.segments,
            score: score * excerpt.proximity,
        })
    }
}

impl IndexBuilder {
    pub fn add(&mut self, input: DocInput) {
        let doc = u32::try_from(self.index.docs.len()).unwrap_or(u32::MAX);
        let parts = text::split(&input.body);
        let crumb = input.breadcrumb.join(" ");

        let mut len = 0_u32;
        len += self.index_field(doc, Field::Title, &input.title);
        len += self.index_field(doc, Field::Crumb, &crumb);
        len += self.index_field(doc, Field::Heading, &parts.headings);
        if let Some(summary) = &input.summary {
            len += self.index_field(doc, Field::Summary, summary);
        }
        len += self.index_field(doc, Field::Body, &parts.prose);
        len += self.index_field(doc, Field::Code, &parts.code);

        self.index.total_len = self.index.total_len.saturating_add(len as usize);
        self.index.docs.push(Doc {
            title: input.title,
            breadcrumb: input.breadcrumb,
            url: input.url,
            kind: input.kind,
            book_slug: input.book_slug,
            source_id: input.source_id,
            len,
            text: text::flatten(&parts.prose),
        });
    }

    #[must_use]
    pub fn build(self) -> SearchIndex {
        self.index
    }

    /// Returns the token count, so the caller can accumulate the document's length.
    fn index_field(&mut self, doc: u32, field: Field, source: &str) -> u32 {
        let mut count = 0_u32;
        for token in text::tokens(source) {
            count += 1;
            // `get_mut` BORROWS the token. `entry` takes ownership, so it allocated a key for every
            // OCCURRENCE of a term rather than every term — and a corpus is mostly repetition, so
            // nearly all of those allocations were thrown away by the map on arrival.
            if let Some(postings) = self.index.terms.get_mut(token.as_ref()) {
                // Documents are added in ascending id, so the posting for THIS document — if the
                // term has been seen in it already — is always the last one.
                match postings.last_mut() {
                    Some(last) if last.doc == doc => last.bump(field, 1),
                    _ => postings.push(Posting::first(doc, field)),
                }
            } else {
                // Genuinely new: the one place a key is allocated.
                self.index.terms.insert(
                    token.into_owned().into_boxed_str(),
                    vec![Posting::first(doc, field)],
                );
            }
        }
        count
    }
}

#[cfg(test)]
mod tests;
