//! The walker's naming rules: what a servable name is, and how a file or folder name becomes a
//! slug, a title and an order. Pure string functions, re-exported from `walker`.

/// Non-empty, every char alphanumeric, `-`, or `_`.
///
/// This is the traversal guard. Layers above it parse permissively ON PURPOSE — `BoardFile::parse`
/// will happily split `../../secret.svg` into a stem and an extension — because exactly one place
/// should decide what a servable name is, and this is that place.
///
/// ```
/// use synapse_server::catalog::domain::walker::slug_like;
///
/// assert!(slug_like("01-intro"));
/// assert!(slug_like("binary_search"));
///
/// // The cases the layers above hand over rather than rejecting themselves.
/// assert!(!slug_like(""));
/// assert!(!slug_like("../../secret"));
/// assert!(!slug_like("a/b"));
/// assert!(!slug_like("a b"));
/// ```
pub fn slug_like(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Every `/`-segment slug-like — rejects empty segments and `..` (traversal guard).
pub fn lesson_path_like(s: &str) -> bool {
    !s.is_empty() && s.split('/').all(slug_like)
}

/// Strip a leading numeric order prefix and one optional separator: `01-foo`→`foo`,
/// `1.bar`→`bar`, `10_baz`→`baz`, `01foo`→`foo`.
pub fn strip_order_prefix(s: &str) -> &str {
    let rest = s.trim_start_matches(|c: char| c.is_ascii_digit());
    if rest.len() == s.len() {
        return s;
    }
    rest.strip_prefix(['.', '_', '-']).unwrap_or(rest)
}

/// `01-singly-linked-list.md` → `Singly Linked List`.
pub fn humanise(name: &str) -> String {
    let base = strip_order_prefix(name);
    let base = base.strip_suffix(".md").unwrap_or(base);
    base.split(['-', '_', '.'])
        .filter(|w| !w.is_empty())
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lowercased alphanumerics; `_` kept; other runs collapse to a single `-`; edges trimmed.
/// `Hello World!` → `hello-world`, `foo--bar` → `foo-bar`, `-trim-` → `trim`.
pub fn slugify(segment: &str) -> String {
    let mut out = String::new();
    for c in segment.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == '_' {
            out.push('_');
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

pub(super) fn order_prefix(s: &str) -> Option<i32> {
    let digits = &s[..s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len()];
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}
