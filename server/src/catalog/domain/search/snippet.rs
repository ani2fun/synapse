//! Cutting a readable quote out of a document, with the matched words marked.
//!
//! **Segments, not offsets.** The obvious design returns byte ranges and lets the client mark
//! them up — and it is wrong across this particular boundary: Rust indexes strings by UTF-8
//! byte, JavaScript by UTF-16 code unit, and the two agree only while the text is ASCII. Prose
//! about naïve caching or a lesson quoting `—` would silently highlight the wrong span. Handing
//! back already-split pieces makes the disagreement unrepresentable, and lets the client build
//! text nodes instead of parsing HTML.

/// One run of the snippet: either matched or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub marked: bool,
}

/// How much text a snippet shows, in characters.
const WIDTH: usize = 200;

/// What a snippet cost to find, and what it says about the document.
pub struct Excerpt {
    pub segments: Vec<Segment>,
    /// A multiplier on the document's score. Terms found close together mean the document is
    /// ABOUT the phrase rather than merely containing its words in different paragraphs.
    pub proximity: f32,
}

/// Quote the densest cluster of query terms out of `text`.
///
/// Terms are matched case-insensitively on character boundaries throughout: every index here
/// comes from `char_indices`, never from arithmetic on byte positions, because slicing a `&str`
/// mid-character panics and `panic = "deny"` does not catch it.
pub fn excerpt(text: &str, terms: &[String]) -> Excerpt {
    let lowered = text.to_lowercase();
    // Each occurrence carries WHICH term it is, because proximity is about distinct terms sitting
    // together. Counting bare occurrences instead would hand a 25% bonus to any document that
    // merely repeats one word — double-counting term frequency and undoing the very length
    // normalisation that stops long documents winning on bulk.
    let mut hits: Vec<(usize, usize, usize)> = Vec::new();
    for (index, term) in terms.iter().enumerate() {
        let mut from = 0;
        while let Some(found) = lowered.get(from..).and_then(|rest| rest.find(term.as_str())) {
            let start = from + found;
            let end = start + term.len();
            if text.is_char_boundary(start) && text.is_char_boundary(end) {
                hits.push((start, end, index));
            }
            from = end;
        }
    }
    hits.sort_unstable();

    let Some(&(first, _, _)) = hits.first() else {
        return Excerpt {
            segments: vec![Segment {
                text: head(text),
                marked: false,
            }],
            proximity: 1.0,
        };
    };

    let proximity = if clustered(&hits) { 1.25 } else { 1.0 };

    let start = boundary(text, first.saturating_sub(WIDTH / 3), false);
    let end = boundary(text, start.saturating_add(WIDTH).min(text.len()), true);
    let mut segments = Vec::new();
    if start > 0 {
        segments.push(Segment {
            text: "…".to_owned(),
            marked: false,
        });
    }
    let mut cursor = start;
    for (from, to, _) in hits {
        if from < cursor || to > end {
            continue;
        }
        push(&mut segments, text.get(cursor..from), false);
        push(&mut segments, text.get(from..to), true);
        cursor = to;
    }
    push(&mut segments, text.get(cursor..end), false);
    if end < text.len() {
        segments.push(Segment {
            text: "…".to_owned(),
            marked: false,
        });
    }
    Excerpt { segments, proximity }
}

/// Do two DIFFERENT query terms occur within a snippet's width of each other?
///
/// Occurrences arrive position-sorted, so the nearest differing pair is found by walking once and
/// remembering, per term, where it was last seen.
fn clustered(hits: &[(usize, usize, usize)]) -> bool {
    let mut last_seen: Vec<Option<usize>> = Vec::new();
    for &(start, end, term) in hits {
        if last_seen.len() <= term {
            last_seen.resize(term + 1, None);
        }
        for (other, seen) in last_seen.iter().enumerate() {
            if other != term && seen.is_some_and(|at| start.saturating_sub(at) <= WIDTH) {
                return true;
            }
        }
        if let Some(slot) = last_seen.get_mut(term) {
            *slot = Some(end);
        }
    }
    false
}

fn push(segments: &mut Vec<Segment>, text: Option<&str>, marked: bool) {
    let Some(text) = text else { return };
    if text.is_empty() {
        return;
    }
    segments.push(Segment {
        text: text.to_owned(),
        marked,
    });
}

/// The opening of a document, for a hit whose term lives only in a heading or in code — there is
/// nothing in the prose to quote, so the first sentences stand in.
fn head(text: &str) -> String {
    let end = boundary(text, WIDTH.min(text.len()), true);
    let mut out = text.get(..end).unwrap_or_default().to_owned();
    if end < text.len() {
        out.push('…');
    }
    out
}

/// Move `at` to the nearest character boundary that is also a word boundary, so a quote never
/// opens or closes mid-word — and never mid-character, which would panic.
fn boundary(text: &str, at: usize, forward: bool) -> usize {
    let mut at = at.min(text.len());
    loop {
        if at == 0 || at == text.len() {
            return at;
        }
        if text.is_char_boundary(at) && text.as_bytes().get(at).is_some_and(u8::is_ascii_whitespace) {
            return at;
        }
        at = if forward { at + 1 } else { at - 1 };
    }
}

#[cfg(test)]
mod tests;
