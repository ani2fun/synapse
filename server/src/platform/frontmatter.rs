//! The frontmatter MECHANISM: the lenient fence splitter and its two helpers,
//! shared by catalog and blog. The line worth defending is finer than "no duplication":
//! contexts own their VOCABULARY — which fields exist and what they mean
//! (`LessonFrontmatter` vs `BlogPost` stay exactly where they were) — while fence
//! *parsing* is generic-subdomain mechanics that says nothing about either context. Pure
//! text functions, no dependencies; the domain-purity gate is untouched.
//!
//! Leniency contract: a fence exists only when the FIRST line is `---` and a
//! closing `---` follows; anything malformed degrades to "no fence" (empty fields, the
//! whole content as body) — missing metadata never fails a page.

use std::collections::BTreeMap;

/// Split content into (fence fields, body). No valid fence → empty map + the whole content.
pub(crate) fn fields_and_body(content: &str) -> (BTreeMap<String, String>, String) {
    let lines: Vec<&str> = content
        .split('\n')
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

/// Inline flow-style lists only: `[a, b, "c d"]` → `["a", "b", "c d"]`.
pub(crate) fn parse_inline_list(value: &str) -> Vec<String> {
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

pub(crate) fn strip_matching_quotes(s: &str) -> &str {
    for quote in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(quote) && s.ends_with(quote) {
            return &s[1..s.len() - 1];
        }
    }
    s
}
