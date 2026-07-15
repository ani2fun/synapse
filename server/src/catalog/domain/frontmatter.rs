//! Lenient YAML-ish frontmatter (oracle: `Frontmatter.scala`, ADR-0001): a fence exists only if
//! the FIRST line is `---` and a terminating `---` follows; anything malformed degrades to "no
//! fence" — missing metadata never fails a lesson.

use std::collections::BTreeMap;

use crate::catalog::domain::lesson::{LessonFrontmatter, Parsed};

/// Split content into (fence fields, body). No valid fence → empty map + the whole content.
pub fn fields_and_body(content: &str) -> (BTreeMap<String, String>, String) {
    let lines: Vec<&str> = content
        .split(['\n'])
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return (BTreeMap::new(), content.to_owned());
    }
    let Some(end) = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
    else {
        return (BTreeMap::new(), content.to_owned());
    };

    let mut fields = BTreeMap::new();
    for line in &lines[1..end] {
        let Some(idx) = line.find(':') else { continue };
        if idx == 0 {
            continue;
        }
        let key = line[..idx].trim().to_owned();
        let value = strip_matching_quotes(line[idx + 1..].trim()).to_owned();
        if !value.is_empty() {
            fields.insert(key, value);
        }
    }
    (fields, lines[end + 1..].join("\n"))
}

/// Frontmatter `title:` → first body `# ` heading → the caller's fallback.
pub fn extract_title(content: &str, fallback: &str) -> String {
    let (fields, body) = fields_and_body(content);
    fields
        .get("title")
        .cloned()
        .or_else(|| first_h1(&body))
        .unwrap_or_else(|| fallback.to_owned())
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

/// Inline flow-style lists only: `[a, b, "c d"]` → `["a", "b", "c d"]`.
pub fn parse_inline_list(value: &str) -> Vec<String> {
    let inner = value
        .trim()
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(value);
    inner
        .split(',')
        .map(|item| strip_matching_quotes(item.trim()).to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

fn strip_matching_quotes(s: &str) -> &str {
    for quote in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(quote) && s.ends_with(quote) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

fn first_h1(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(|rest| rest.trim().to_owned()))
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
