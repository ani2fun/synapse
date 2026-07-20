//! Lenient YAML-ish frontmatter (oracle: `Frontmatter.scala`, ADR-0001). The MECHANISM —
//! fence split, quote strip, inline list — is `platform::frontmatter` since step 62 (it was
//! a byte-identical twin of blog's); this module keeps the catalog's VOCABULARY: which
//! fields a lesson has and how they degrade.

use crate::catalog::domain::lesson::{LessonFrontmatter, Parsed};
pub(crate) use crate::platform::frontmatter::{fields_and_body, parse_inline_list};

/// Frontmatter `title:` → first body `# ` heading → the caller's fallback.
pub fn extract_title(content: &str, fallback: &str) -> String {
    let (fields, body) = fields_and_body(content);
    fields
        .get("title")
        .cloned()
        .or_else(|| first_h1(&body))
        .unwrap_or_else(|| fallback.to_owned())
}

/// Frontmatter `summary:` — the lesson's own one-line description, used for the `<meta
/// name="description">` and Open Graph tags the server injects (step 50). Blank is `None`:
/// an empty description tag is worse than none, because a crawler will show it.
pub fn extract_summary(content: &str) -> Option<String> {
    fields_and_body(content)
        .0
        .get("summary")
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// `Some` only when the fence carries a literal `essential: true|false`.
pub fn extract_essential(content: &str) -> Option<bool> {
    match fields_and_body(content).0.get("essential").map(String::as_str) {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    }
}

/// The full lesson parse: typed frontmatter (title falls back like `extract_title`) + the body
/// with the fence stripped.
pub fn parse(content: &str, fallback_title: &str) -> Parsed {
    let (fields, body) = fields_and_body(content);
    let title = fields
        .get("title")
        .cloned()
        .or_else(|| first_h1(&body))
        .unwrap_or_else(|| fallback_title.to_owned());
    Parsed {
        frontmatter: LessonFrontmatter {
            title,
            summary: fields.get("summary").cloned(),
            essential: match fields.get("essential").map(String::as_str) {
                Some("true") => Some(true),
                Some("false") => Some(false),
                _ => None,
            },
            kind: fields.get("kind").cloned(),
            difficulty: fields.get("difficulty").cloned(),
            topics: fields.get("topics").map(|v| parse_inline_list(v)),
        },
        body,
    }
}

fn first_h1(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(|rest| rest.trim().to_owned()))
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
