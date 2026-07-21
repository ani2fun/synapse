//! The pure catalog assembler — turns the raw content tree into the browsable catalog + the
//! lesson-file map, enforcing the `SYNAPSE_ROOT` conventions and refusing to paper over slug
//! collisions.

use std::collections::{BTreeMap, BTreeSet};

use crate::catalog::domain::catalog::{
    Book, BookEntry, CatalogEntry, Category, Lesson, SynapseContentCatalog, SynapseContentError, WalkResult,
};
use crate::catalog::domain::content_tree::{BookMeta, CategoryMeta, ContentEntry};
use crate::catalog::domain::frontmatter;

pub const MAX_CHAPTER_DEPTH: usize = 6;
pub const DEFAULT_ESSENTIAL: bool = true;

/// Dirs that are never content (checked order-prefix-stripped).
///
/// `examples` and `c4` are aux dirs a book may carry alongside its chapters.
///
/// `local-only` is different in kind and is here for a REASON WORTH KNOWING. The content tree
/// carries material that must never be served — most of it adapted from a commercial course,
/// kept for personal study (ADR-RS002). Relying solely on a `.gitignore` rule in the CONTENT
/// repository would put the protection in a different repo, a different layer, and a different
/// moment (push time) from the thing it protects: a stray `git add -f`, a blanket `git add -A`,
/// or a restructure that moves a book out of that folder would publish it silently, and the
/// server would index it happily (it would appear in `/api/synapse/index`, in `/sitemap.xml`,
/// and in `lesson_view`).
///
/// Naming it here makes it unservable by construction, in the repo that does the serving. The
/// `_`-prefix rule above is the other half; either alone suffices, and having both is
/// deliberate.
const RESERVED_AUX_DIRS: [&str; 3] = ["examples", "c4", "local-only"];

// ─────────────────────────────────────────────────────────────────────────────
// NAMING RULES — the public helpers the whole context leans on
// ─────────────────────────────────────────────────────────────────────────────

/// Non-empty, every char alphanumeric, `-`, or `_`.
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

fn order_prefix(s: &str) -> Option<i32> {
    let digits = &s[..s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len()];
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Eligible content dir: slug-like, not `_*`/`.*`, and not a reserved aux dir.
fn includes_as_content(name: &str) -> bool {
    slug_like(name)
        && !name.starts_with('_')
        && !name.starts_with('.')
        && !RESERVED_AUX_DIRS.contains(&strip_order_prefix(name))
}

/// A lesson source: `.md`, not the `.editorial.md` sidecar, not `_*`/`.*`.
/// Case-sensitive on purpose: content extensions are lowercase by convention, and `.MD` should
/// NOT silently become a lesson.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_lesson_file(name: &str) -> bool {
    name.ends_with(".md")
        && !name.ends_with(".editorial.md")
        && !name.starts_with('_')
        && !name.starts_with('.')
}

/// Book-interior sort key: `index(.md)` first, then numeric prefixes, then the rest.
fn interior_order(name: &str) -> i32 {
    if name.strip_suffix(".md").unwrap_or(name) == "index" {
        return -1;
    }
    order_prefix(name).unwrap_or(i32::MAX)
}

// ─────────────────────────────────────────────────────────────────────────────
// THE WALK
// ─────────────────────────────────────────────────────────────────────────────

/// Assemble the catalog from the raw tree. Lenient about missing metadata (ADR-0001), strict
/// about convention violations (duplicate slugs, over-deep chapters, non-slug lesson paths).
pub fn walk(roots: &[ContentEntry]) -> Result<WalkResult, SynapseContentError> {
    let mut state = WalkState::default();
    let entries = build_level(roots, &[], &[], &mut state)?;
    Ok(WalkResult {
        catalog: SynapseContentCatalog { entries },
        lesson_files: state.lesson_files,
    })
}

#[derive(Default)]
struct WalkState {
    seen_book_slugs: BTreeSet<String>,
    lesson_files: BTreeMap<String, BTreeMap<String, String>>,
}

/// One library level (the root or a category's children): books and sub-categories, sorted by
/// `(order ?: MAX, dir name lowercased)`. Root files and ineligible dirs are skipped; empty
/// categories vanish.
fn build_level(
    children: &[ContentEntry],
    category_path: &[String],
    dir_path: &[String],
    state: &mut WalkState,
) -> Result<Vec<CatalogEntry>, SynapseContentError> {
    let mut level: Vec<(i32, String, CatalogEntry)> = Vec::new();
    for child in children {
        let ContentEntry::Dir {
            name,
            book_meta,
            category_meta,
            children,
        } = child
        else {
            continue;
        };
        if !includes_as_content(name) {
            continue;
        }
        if let Some(meta) = book_meta {
            let book = build_book(name, meta, children, category_path, dir_path, state)?;
            level.push((
                book.order.unwrap_or(i32::MAX),
                name.to_lowercase(),
                CatalogEntry::Book(book),
            ));
        } else if let Some(category) = build_category(
            name,
            category_meta.as_ref(),
            children,
            category_path,
            dir_path,
            state,
        )? {
            level.push((
                category.order.unwrap_or(i32::MAX),
                name.to_lowercase(),
                CatalogEntry::Category(category),
            ));
        }
    }
    level.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    Ok(level.into_iter().map(|(_, _, entry)| entry).collect())
}

/// A category exists only if at least one book lives beneath it.
fn build_category(
    name: &str,
    meta: Option<&CategoryMeta>,
    children: &[ContentEntry],
    category_path: &[String],
    dir_path: &[String],
    state: &mut WalkState,
) -> Result<Option<Category>, SynapseContentError> {
    let slug = slugify(strip_order_prefix(name));
    let mut inner_categories = category_path.to_vec();
    inner_categories.push(slug.clone());
    let mut inner_dirs = dir_path.to_vec();
    inner_dirs.push(name.to_owned());

    let entries = build_level(children, &inner_categories, &inner_dirs, state)?;
    if entries.is_empty() {
        return Ok(None);
    }
    Ok(Some(Category {
        slug,
        title: meta
            .and_then(|m| m.title.clone())
            .unwrap_or_else(|| humanise(name)),
        description: meta.and_then(|m| m.description.clone()),
        icon: meta.and_then(|m| m.icon.clone()),
        order: meta.and_then(|m| m.order).or_else(|| order_prefix(name)),
        entries,
    }))
}

fn build_book(
    name: &str,
    meta: &BookMeta,
    children: &[ContentEntry],
    category_path: &[String],
    dir_path: &[String],
    state: &mut WalkState,
) -> Result<Book, SynapseContentError> {
    // An explicit book.json slug overrides the folder-derived one; file paths keep the folder.
    let slug = meta
        .slug
        .as_deref()
        .map(slugify)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slugify(strip_order_prefix(name)));
    if !state.seen_book_slugs.insert(slug.clone()) {
        return Err(SynapseContentError::DuplicateBookSlug(slug));
    }

    let mut book_dirs = dir_path.to_vec();
    book_dirs.push(name.to_owned());
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    let mut duplicates: BTreeSet<String> = BTreeSet::new();
    let entries = build_book_entries(children, &[], &book_dirs, &mut files, &mut duplicates)?;
    if !duplicates.is_empty() {
        return Err(SynapseContentError::DuplicateLessonSlug {
            book_slug: slug,
            slugs: duplicates.into_iter().collect(),
        });
    }
    state.lesson_files.insert(slug.clone(), files);

    Ok(Book {
        slug,
        title: meta.title.clone().unwrap_or_else(|| humanise(name)),
        description: meta.description.clone().unwrap_or_default(),
        tags: meta.tags.clone().unwrap_or_default(),
        estimated_reading_minutes: meta.estimated_reading_minutes,
        order: meta.order.or_else(|| order_prefix(name)),
        category_path: category_path.to_vec(),
        entries,
    })
}

/// One book-interior level: chapters (eligible dirs) and lessons (eligible `.md` files),
/// sorted by `(index-first/numeric-prefix, name lowercased)`.
fn build_book_entries(
    children: &[ContentEntry],
    chapter_slugs: &[String],
    dir_path: &[String],
    files: &mut BTreeMap<String, String>,
    duplicates: &mut BTreeSet<String>,
) -> Result<Vec<BookEntry>, SynapseContentError> {
    let mut level: Vec<(i32, String, BookEntry)> = Vec::new();
    for child in children {
        match child {
            ContentEntry::Dir { name, children, .. } if includes_as_content(name) => {
                let chapter_slug = slugify(strip_order_prefix(name));
                let mut slugs = chapter_slugs.to_vec();
                slugs.push(chapter_slug.clone());
                if slugs.len() > MAX_CHAPTER_DEPTH {
                    return Err(SynapseContentError::MaxChapterDepthExceeded(slugs.join("/")));
                }
                let mut dirs = dir_path.to_vec();
                dirs.push(name.to_owned());
                let entries = build_book_entries(children, &slugs, &dirs, files, duplicates)?;
                level.push((
                    interior_order(name),
                    name.to_lowercase(),
                    BookEntry::Chapter {
                        slug: chapter_slug,
                        title: humanise(name),
                        order: order_prefix(name),
                        entries,
                    },
                ));
            }
            ContentEntry::File { name, content } if is_lesson_file(name) => {
                let stem = name.strip_suffix(".md").unwrap_or(name);
                let lesson_slug = slugify(strip_order_prefix(stem));
                let mut path_segments = chapter_slugs.to_vec();
                path_segments.push(lesson_slug.clone());
                let slug_path = path_segments.join("/");
                if !lesson_path_like(&slug_path) {
                    return Err(SynapseContentError::InvalidSlug {
                        path_in_book: slug_path,
                        slug: lesson_slug,
                    });
                }
                let mut file_path = dir_path.to_vec();
                file_path.push(name.to_owned());
                if files.insert(slug_path.clone(), file_path.join("/")).is_some() {
                    duplicates.insert(slug_path);
                    continue;
                }
                level.push((
                    interior_order(name),
                    name.to_lowercase(),
                    BookEntry::Lesson(Lesson {
                        slug: lesson_slug,
                        title: frontmatter::extract_title(content, &humanise(name)),
                        order: order_prefix(name),
                        essential: frontmatter::extract_essential(content).unwrap_or(DEFAULT_ESSENTIAL),
                        description: frontmatter::extract_summary(content),
                        kind: frontmatter::extract_kind(content),
                    }),
                ));
            }
            _ => {}
        }
    }
    level.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    Ok(level.into_iter().map(|(_, _, entry)| entry).collect())
}

#[cfg(test)]
#[path = "walker_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "walker_exclusion_tests.rs"]
mod exclusion_tests;
