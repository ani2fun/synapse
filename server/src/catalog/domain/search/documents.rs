//! Turning a finished walk plus the raw trees it came from into indexable documents.
//!
//! The two halves have to meet here rather than earlier. A satellite's breadcrumb is decided by
//! its REGISTRATION — `programming-languages › Java` — not by its own walk, which produces a
//! top-level book knowing nothing about where it was grafted. So the merged catalog is what says
//! where a lesson sits, while only the source trees still hold its prose.
//!
//! Membership follows `lesson_files` exactly, which is what keeps the index honest: a book that
//! lost a duplicate-slug collision is not in that map, and neither are `local-only-content/`,
//! `_`-prefixed files or the reserved aux dirs. The index therefore cannot contain anything the
//! site does not serve — not by re-implementing the exclusions, but by never seeing them.

use std::collections::BTreeMap;

use super::{DocInput, DocKind, SearchIndex};
use crate::catalog::domain::catalog::{CatalogEntry, WalkResult};
use crate::catalog::domain::content_tree::{ContentEntry, SourceTree};
use crate::catalog::domain::resolver;

/// Build the index for a merged library.
pub fn index_of(sources: &[SourceTree], walk: &WalkResult) -> SearchIndex {
    let bodies = bodies(sources);
    let titles = category_titles(walk);
    let mut builder = SearchIndex::builder();

    for book in resolver::all_books(&walk.catalog) {
        let prefix = resolver::book_prefix(book);
        let mut breadcrumb = crumb(&titles, &book.category_path);
        breadcrumb.push(book.title.clone());
        let Some(files) = walk.lesson_files.get(&book.slug) else {
            continue;
        };
        for (in_book, lesson) in resolver::lessons_in_reading_order(book) {
            let Some(file) = files.get(&in_book) else {
                continue;
            };
            let key = (file.source_id.clone(), file.path.clone());
            let Some(body) = bodies.get(&key) else {
                continue;
            };
            builder.add(DocInput {
                title: lesson.title.clone(),
                breadcrumb: breadcrumb.clone(),
                url: format!("{prefix}/{in_book}"),
                kind: DocKind::Lesson,
                book_slug: book.slug.clone(),
                source_id: file.source_id.clone(),
                summary: lesson.description.clone(),
                body: (*body).to_owned(),
            });
        }
    }
    builder.build()
}

/// Every markdown body, keyed the way a `LessonFileRef` names it: the source id plus the path of
/// RAW directory names, order prefixes intact — the same string the walker built and the same one
/// the filesystem adapter opens.
fn bodies(sources: &[SourceTree]) -> BTreeMap<(String, String), &str> {
    fn descend<'a>(
        source: &str,
        entries: &'a [ContentEntry],
        at: &mut Vec<String>,
        out: &mut BTreeMap<(String, String), &'a str>,
    ) {
        for entry in entries {
            match entry {
                ContentEntry::File { name, content } => {
                    let mut path = at.clone();
                    path.push(name.clone());
                    out.insert((source.to_owned(), path.join("/")), content.as_str());
                }
                ContentEntry::Dir { name, children, .. } => {
                    at.push(name.clone());
                    descend(source, children, at, out);
                    at.pop();
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for source in sources {
        descend(&source.id, &source.children, &mut Vec::new(), &mut out);
    }
    out
}

/// Category slug-path → its title, so a breadcrumb reads in words rather than slugs.
fn category_titles(walk: &WalkResult) -> BTreeMap<Vec<String>, String> {
    fn descend(entries: &[CatalogEntry], at: &mut Vec<String>, out: &mut BTreeMap<Vec<String>, String>) {
        for entry in entries {
            if let CatalogEntry::Category(category) = entry {
                at.push(category.slug.clone());
                out.insert(at.clone(), category.title.clone());
                descend(&category.entries, at, out);
                at.pop();
            }
        }
    }
    let mut out = BTreeMap::new();
    descend(&walk.catalog.entries, &mut Vec::new(), &mut out);
    out
}

/// A category path in titles. A slug with no declaration falls back to itself rather than
/// vanishing — a gap in the trail is worse than an unpolished word.
fn crumb(titles: &BTreeMap<Vec<String>, String>, path: &[String]) -> Vec<String> {
    let mut at = Vec::new();
    let mut out = Vec::new();
    for slug in path {
        at.push(slug.clone());
        out.push(titles.get(&at).cloned().unwrap_or_else(|| slug.clone()));
    }
    out
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;
