//! Pure catalog navigation (oracle: `CatalogResolver.scala`) — descend categories to a book,
//! then walk its interior to a lesson; and the pre-order reading sequence prev/next hangs off.

use crate::catalog::domain::catalog::{Book, BookEntry, CatalogEntry, Lesson, SynapseContentCatalog};

/// Resolve a full slug path (`category…/book/chapter…/lesson`) to `(book, in-book path, lesson)`.
/// The first matching BOOK slug stops the category descent; the remainder must end at a lesson
/// (a chapter or the bare book prefix resolves to `None`).
pub fn resolve_lesson<'a>(
    catalog: &'a SynapseContentCatalog,
    path: &[String],
) -> Option<(&'a Book, String, &'a Lesson)> {
    resolve_in_entries(&catalog.entries, path)
}

fn resolve_in_entries<'a>(
    entries: &'a [CatalogEntry],
    path: &[String],
) -> Option<(&'a Book, String, &'a Lesson)> {
    let (first, rest) = path.split_first()?;
    let entry = entries.iter().find(|e| e.slug() == first)?;
    match entry {
        CatalogEntry::Book(book) => {
            let lesson = lesson_at(&book.entries, rest)?;
            Some((book, rest.join("/"), lesson))
        }
        CatalogEntry::Category(category) => resolve_in_entries(&category.entries, rest),
    }
}

fn lesson_at<'a>(entries: &'a [BookEntry], path: &[String]) -> Option<&'a Lesson> {
    let (first, rest) = path.split_first()?;
    entries.iter().find_map(|entry| match entry {
        BookEntry::Lesson(lesson) if rest.is_empty() && lesson.slug == *first => Some(lesson),
        BookEntry::Chapter { slug, entries, .. } if slug == first && !rest.is_empty() => {
            lesson_at(entries, rest)
        }
        _ => None,
    })
}

/// Every lesson of a book with its in-book slug-path, pre-order — the reading sequence.
pub fn lessons_in_reading_order(book: &Book) -> Vec<(String, &Lesson)> {
    fn collect<'a>(entries: &'a [BookEntry], prefix: &[String], out: &mut Vec<(String, &'a Lesson)>) {
        for entry in entries {
            match entry {
                BookEntry::Lesson(lesson) => {
                    let mut segments = prefix.to_vec();
                    segments.push(lesson.slug.clone());
                    out.push((segments.join("/"), lesson));
                }
                BookEntry::Chapter { slug, entries, .. } => {
                    let mut segments = prefix.to_vec();
                    segments.push(slug.clone());
                    collect(entries, &segments, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(&book.entries, &[], &mut out);
    out
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
