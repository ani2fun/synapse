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
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty() && token.chars().count() <= MAX_TOKEN)
        .map(str::to_lowercase)
        .collect()
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
#[path = "text_tests.rs"]
mod tests;
