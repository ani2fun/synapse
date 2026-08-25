//! What a proposed edit must satisfy before it is allowed anywhere near the forge, plus the
//! drift fingerprint the whole optimistic-concurrency story rests on.
//!
//! The guardrails are deliberately few and mechanical. Judging whether prose is any GOOD is the
//! reviewer's job on the pull request — this layer only refuses changes that would break the
//! page for every reader, which is the one thing review is bad at catching quickly.

use crate::platform::frontmatter::fields_and_body;

/// A lesson well past any real length (the longest in the catalog is a small fraction of this).
/// It exists to bound what one request can push at the forge, not to have an opinion on length.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidEdit {
    #[error("the proposed file is empty")]
    Empty,
    #[error("the proposed file is {bytes} bytes, over the {cap} byte limit")]
    TooLarge { bytes: usize, cap: usize },
    #[error("the frontmatter fence (the '---' block at the top) is missing or unclosed")]
    FrontmatterLost,
    #[error("the lesson has no title left — restore the frontmatter 'title:' or a '# ' heading")]
    TitleLost,
}

/// Normalise and check a proposal against the file it started from. Returns the text to commit.
///
/// The frontmatter rules are CONDITIONAL on the original: a lesson that never had a fence is not
/// required to grow one, but a lesson that had one may not lose it — dropping the fence silently
/// changes the page's title, summary and Open Graph tags, and it is the single easiest thing to
/// destroy by selecting all and pasting.
pub fn validate(original: &str, proposed: &str) -> Result<String, InvalidEdit> {
    let text = normalise(proposed);
    if text.trim().is_empty() {
        return Err(InvalidEdit::Empty);
    }
    if text.len() > MAX_SOURCE_BYTES {
        return Err(InvalidEdit::TooLarge {
            bytes: text.len(),
            cap: MAX_SOURCE_BYTES,
        });
    }
    if has_fence(original) && !has_fence(&text) {
        return Err(InvalidEdit::FrontmatterLost);
    }
    if title_of(&text).is_none() {
        return Err(InvalidEdit::TitleLost);
    }
    Ok(text)
}

/// CRLF and lone CR collapse to LF, and the file ends with exactly one newline — a Windows
/// browser must not turn every line of a page into a diff, and a missing final newline is the
/// most common noise line in a review.
pub fn normalise(source: &str) -> String {
    let mut text = source.replace("\r\n", "\n").replace('\r', "\n");
    while text.ends_with("\n\n") {
        text.pop();
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// A stable digest of the NORMALISED text, used to notice that the file moved under an editor
/// who has had it open for a while.
///
/// FNV-1a-64, hand-rolled and deliberately not a cryptographic hash: this is a drift detector,
/// not a security boundary. Nothing is authorised by it — the server re-reads the file itself
/// before committing, and the forge's own blob-sha check is what actually guards the branch — so
/// a hash chosen for collision RESISTANCE would buy nothing and cost a dependency.
pub fn fingerprint(source: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in normalise(source).as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

/// A frontmatter fence exists when the first line is `---` and a closing `---` follows — the
/// same leniency contract `platform::frontmatter` parses by.
fn has_fence(content: &str) -> bool {
    let mut lines = content.split('\n').map(|l| l.trim_end_matches('\r').trim_end());
    lines.next() == Some("---") && lines.any(|line| line == "---")
}

/// Frontmatter `title:`, else the first `# ` heading — the same order the catalog resolves a
/// lesson's title in. `None` means the page would render with no title at all.
fn title_of(content: &str) -> Option<String> {
    let (fields, body) = fields_and_body(content);
    fields
        .get("title")
        .cloned()
        .or_else(|| {
            body.lines()
                .find_map(|line| line.strip_prefix("# ").map(|rest| rest.trim().to_owned()))
        })
        .filter(|title| !title.is_empty())
}

#[cfg(test)]
mod tests;
