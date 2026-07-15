//! Pure catalog navigation over the WIRE DTOs (the logic layer: no leptos, no web-sys — the
//! purity gate greps it; plain `cargo test` covers it natively).

use synapse_shared::catalog::{BookDto, BookEntryDto, CatalogEntryDto, LessonDto, SynapseIndexDto};

/// A book's URL prefix segments: `categoryPath + slug`.
pub fn book_prefix(book: &BookDto) -> Vec<String> {
    let mut segments = book.category_path.clone();
    segments.push(book.slug.clone());
    segments
}

/// Every lesson of a book with its FULL directory-mirror path, pre-order — the sidebar's and
/// the library card's source of truth.
pub fn reading_order(book: &BookDto) -> Vec<(String, LessonDto)> {
    fn collect(entries: &[BookEntryDto], prefix: &[String], out: &mut Vec<(String, LessonDto)>) {
        for entry in entries {
            match entry {
                BookEntryDto::Lesson(lesson) => {
                    let mut segments = prefix.to_vec();
                    segments.push(lesson.slug.clone());
                    out.push((segments.join("/"), lesson.clone()));
                }
                BookEntryDto::Chapter(chapter) => {
                    let mut segments = prefix.to_vec();
                    segments.push(chapter.slug.clone());
                    collect(&chapter.entries, &segments, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(&book.entries, &book_prefix(book), &mut out);
    out
}

/// Where a book's cover card points: its first lesson in reading order.
pub fn first_lesson_path(book: &BookDto) -> Option<String> {
    reading_order(book).into_iter().next().map(|(path, _)| path)
}

/// The book a lesson path belongs to: the entry whose `categoryPath + slug` prefixes the path.
pub fn book_of<'a>(index: &'a SynapseIndexDto, lesson_path: &[String]) -> Option<&'a BookDto> {
    fn find<'a>(entries: &'a [CatalogEntryDto], path: &[String]) -> Option<&'a BookDto> {
        let (first, rest) = path.split_first()?;
        entries.iter().find_map(|entry| match entry {
            CatalogEntryDto::Book(book) if book.slug == *first => Some(book),
            CatalogEntryDto::Category(category) if category.slug == *first => find(&category.entries, rest),
            _ => None,
        })
    }
    find(&index.entries, lesson_path)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
