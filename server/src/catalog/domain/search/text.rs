//! Turning markdown into terms, and into something quotable.
//!
//! Pure text arithmetic — the whole reason search can be unit-tested without a filesystem.

/// The longest token worth keeping. Guards against base64 blobs and minified fixtures, which
/// would otherwise each add a unique term nobody will ever search for.
const MAX_TOKEN: usize = 32;

/// Lowercase, split on anything not alphanumeric.
///
/// Markdown syntax dissolves for free: `## Heading` yields `heading`, `[text](url)` yields both
/// words, `**bold**` yields `bold`. Single characters are KEPT — this library teaches `C` and
/// `R`, and inverse document frequency already makes genuinely common terms weightless, so a
/// length floor would only lose the rare ones.
pub fn tokenize(text: &str) -> Vec<String> {
    tokens(text).map(std::borrow::Cow::into_owned).collect()
}

/// The same tokens, BORROWED where possible.
///
/// `index_of` runs this over every body in the catalog, and almost all of those tokens are already
/// lowercase — so `to_lowercase`'s unconditional allocation was one of two per token, the other
/// being the map key in `index_field`.
///
/// Worth having, not dramatic: measured A/B on a release build over the production corpus, the
/// index build went 190 ms → 176 ms. It reads much larger under `cargo build` with
/// `render-local-only`, where the corpus is 4.6× bigger and nothing is optimised — a debug profile
/// is not where to judge an allocation change.
///
/// The lowercase test is "no alphabetic character that is not already lowercase" rather than "no
/// uppercase character", because a TITLECASE character (`ǅ`) reports neither upper nor lower and
/// would otherwise be borrowed unchanged where `to_lowercase` would have folded it.
pub fn tokens(text: &str) -> impl Iterator<Item = std::borrow::Cow<'_, str>> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty() && token.chars().count() <= MAX_TOKEN)
        .map(|token| {
            if token.chars().all(|c| !c.is_alphabetic() || c.is_lowercase()) {
                std::borrow::Cow::Borrowed(token)
            } else {
                std::borrow::Cow::Owned(token.to_lowercase())
            }
        })
}

/// The three things a document is made of, kept apart so a title can outrank a passing mention.
pub struct Split {
    pub headings: String,
    /// Prose with fenced code removed — what a snippet is quoted from.
    pub prose: String,
    /// Fenced code, indexed but never quoted. A reader searching `PARTITION BY` expects the SQL
    /// examples to count; they just do not read as prose in a result row.
    pub code: String,
}

/// Separate ATX headings and fenced code from the prose between them.
///
/// Fence state is tracked so a `#` comment inside a shell example is not mistaken for a section
/// title — the single most common way a naive line scanner corrupts an index.
pub fn split(body: &str) -> Split {
    let mut headings = String::new();
    let mut prose = String::new();
    let mut code = String::new();
    let mut fence: Option<char> = None;

    for line in body.lines() {
        let trimmed = line.trim_start();
        let opener = trimmed
            .starts_with("```")
            .then_some('`')
            .or_else(|| trimmed.starts_with("~~~").then_some('~'));
        match (fence, opener) {
            // Only the SAME marker closes a fence, so a ``` inside a ~~~ block stays code.
            (Some(open), Some(found)) if open == found => {
                fence = None;
                continue;
            }
            (None, Some(found)) => {
                fence = Some(found);
                continue;
            }
            _ => {}
        }
        if fence.is_some() {
            push_line(&mut code, line);
        } else if trimmed.starts_with('#') {
            push_line(&mut headings, trimmed.trim_start_matches('#').trim());
        } else {
            push_line(&mut prose, line);
        }
    }
    Split {
        headings,
        prose,
        code,
    }
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

/// Markdown to something a person can read in a result row: drop list and quote markers, collapse
/// blank lines and runs of whitespace into single spaces.
pub fn flatten(prose: &str) -> String {
    let mut out = String::with_capacity(prose.len());
    for line in prose.lines() {
        let cleaned = line
            .trim()
            .trim_start_matches(['>', '-', '*', '+'])
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start_matches(['.', ')'])
            .trim();
        if cleaned.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        for (i, word) in cleaned.split_whitespace().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(word);
        }
    }
    out
}

#[cfg(test)]
mod tests;
