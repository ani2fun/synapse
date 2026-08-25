//! Branch naming. One branch per (contributor, page, attempt), derived — never stored and looked
//! up — so the same edit always lands on the same ref:
//!
//! ```text
//! edit/ani2fun/system-design-from-first-principles/foundations/thinking-in-tradeoffs
//! edit/ani2fun/system-design-from-first-principles/foundations/thinking-in-tradeoffs-2
//! ```
//!
//! The `edit/<user>/…` shape groups a contributor's proposals together in the forge's branch
//! list and stays readable, which matters because the branch name is the first thing a reviewer
//! sees. The `-2`, `-3` suffix appears only when an earlier proposal for the same page was
//! already merged or closed.
//!
//! Everything here is sanitised to git's ref rules rather than trusted. Lesson paths are already
//! slug-like by the catalog's own conventions and usernames come canonicalised from the token
//! verifier, but a branch name is a place where "should be fine" is not good enough: one stray
//! `~` or `..` and every proposal from that contributor fails at the forge.

use crate::authoring::domain::validation::fingerprint;

/// Git imposes no length limit of its own, but refs become FILES in the forge's object store, so
/// a long one hits a filesystem limit rather than a git one. 200 leaves comfortable room under
/// every real limit while fitting the deepest lesson paths the catalog allows whole.
const MAX_BRANCH_LEN: usize = 200;
/// A username long enough to be anybody's, short enough to leave the page path its budget.
const MAX_OWNER_LEN: usize = 40;

const OWNER_FALLBACK: &str = "contributor";
const PAGE_FALLBACK: &str = "lesson";

/// `edit/<username>/<lesson-path>` for the first attempt, `…-2`, `…-3` after.
pub fn branch_for(username: &str, lesson_path: &str, attempt: u32) -> String {
    let owner = truncate(&sanitise_segment(username, OWNER_FALLBACK), MAX_OWNER_LEN);
    let page = sanitise_path(lesson_path);
    let suffix = if attempt <= 1 {
        String::new()
    } else {
        format!("-{attempt}")
    };
    let head = format!("edit/{owner}/");
    // saturating: `head` and `suffix` are both bounded well below MAX_BRANCH_LEN, so this can
    // only reach zero if those bounds ever change — and a zero budget yields the fallback rather
    // than a panic.
    let budget = MAX_BRANCH_LEN.saturating_sub(head.len() + suffix.len());
    format!("{head}{}{suffix}", fit(&page, budget))
}

/// Whether a name is legal as a git branch. The production path always produces one — this is
/// what the tests assert against, so the rules are stated once, executably, instead of living in
/// a comment that drifts.
// `.lock` is git's own reserved ref suffix, not a file extension, and git compares it
// case-sensitively — so the case-insensitive comparison clippy suggests would reject refs git
// accepts.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub fn is_valid_ref(name: &str) -> bool {
    !name.is_empty()
        && !name.contains("..")
        && !name.contains("//")
        && !name.contains("@{")
        && !name.starts_with('/')
        && !name.ends_with('/')
        && !name.ends_with('.')
        && !name.ends_with(".lock")
        && name != "@"
        && name.len() <= MAX_BRANCH_LEN
        && name
            .chars()
            .all(|c| !c.is_ascii_control() && !matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
        && name
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.'))
}

/// Lowercase; `a-z`, `0-9`, `-` and `_` survive; every other run collapses to a single `-`; the
/// edges are trimmed. Empty in, `fallback` out.
fn sanitise_segment(raw: &str, fallback: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == '_' {
            out.push('_');
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The page path, segment by segment, `/` preserved. Empty segments vanish; an empty path
/// degrades to `PAGE_FALLBACK` rather than producing a ref that ends in `/`.
fn sanitise_path(raw: &str) -> String {
    let segments: Vec<String> = raw
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| sanitise_segment(s, ""))
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() {
        PAGE_FALLBACK.to_owned()
    } else {
        segments.join("/")
    }
}

/// Fit the page path into `budget`. Over budget, the TAIL is what survives — the lesson's own
/// name is what a reviewer reads — and a digest of the whole path is appended so two pages that
/// share a tail still get distinct branches.
fn fit(page: &str, budget: usize) -> String {
    if page.len() <= budget {
        return page.to_owned();
    }
    let digest = &fingerprint(page)[..8];
    // `-` + 8 hex; if even that will not fit, the digest alone is the branch — unreadable, but
    // unique and legal, which beats a truncated ref that collides.
    let Some(room) = budget.checked_sub(digest.len() + 1) else {
        return digest.to_owned();
    };
    let tail = tail_from_boundary(page, room);
    if tail.is_empty() {
        digest.to_owned()
    } else {
        format!("{tail}-{digest}")
    }
}

/// The longest `/`-boundary-aligned tail of `page` that fits in `room`, so truncation never
/// leaves half a slug. Falls back to the last whole segment when even that overflows.
fn tail_from_boundary(page: &str, room: usize) -> &str {
    let mut best = "";
    for (index, _) in page.match_indices('/') {
        let candidate = &page[index + 1..];
        if candidate.len() <= room {
            best = candidate;
            break;
        }
    }
    if best.is_empty() {
        let last = page.rsplit('/').next().unwrap_or(page);
        if last.len() <= room { last } else { "" }
    } else {
        best
    }
}

/// Truncate on a char boundary — usernames are sanitised to ASCII above, but this helper must
/// not be the thing that panics if that ever stops being true.
fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_owned();
    }
    let end = (0..=max).rev().find(|i| value.is_char_boundary(*i)).unwrap_or(0);
    value[..end].trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests;
