//! Pure catalog navigation over the WIRE DTOs (the logic layer: no leptos, no web-sys — the
//! purity gate greps it; plain `cargo test` covers it natively).

pub mod editorial;
pub mod pane;
pub mod prefs;
pub mod progress;

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

/// The chapter a lesson belongs to FOR COUNTING purposes — its path minus the last segment,
/// except when that chapter exists only to hold it.
///
/// Most problems are authored as `…/problems/<slug>/<slug>.md`, giving each one a chapter of its
/// own (36 of this content's 61 chapters have that shape). Scoping on the raw parent would make
/// every problem read "1 / 1". The sidebar already flattens a chapter whose only child is a
/// lesson of the SAME slug, and the counter has to agree with it — so when the parent segment
/// and the lesson slug match, the real chapter is one level up.
fn counting_chapter(path: &str) -> Option<String> {
    let (parent, slug) = path.rsplit_once('/')?;
    match parent.rsplit_once('/') {
        Some((grandparent, chapter)) if chapter == slug => Some(grandparent.to_owned()),
        _ => Some(parent.to_owned()),
    }
}

/// The problems of `lesson_path`'s own chapter in reading order, and where that path sits among
/// them. `None` when the path isn't in the book, or isn't itself a problem.
///
/// Scoping comes from the paths rather than the tree, because `reading_order` flattens chapters
/// away. Book-root lessons fall out of the same rule — their parent is the book prefix — so a
/// book with no chapters still counts correctly.
pub fn chapter_problems(book: &BookDto, lesson_path: &str) -> Option<(Vec<String>, usize)> {
    let here = counting_chapter(lesson_path)?;
    let problems: Vec<String> = reading_order(book)
        .into_iter()
        .filter(|(path, lesson)| {
            lesson.lesson_kind.as_deref() == Some("problem")
                && counting_chapter(path).as_deref() == Some(&here)
        })
        .map(|(path, _)| path)
        .collect();
    let at = problems.iter().position(|path| path == lesson_path)?;
    Some((problems, at))
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

/// The book with a globally-unique slug, DFS through categories (oracle: `CatalogNav.findBook`).
pub fn find_book<'a>(index: &'a SynapseIndexDto, slug: &str) -> Option<&'a BookDto> {
    fn dfs<'a>(entries: &'a [CatalogEntryDto], slug: &str) -> Option<&'a BookDto> {
        entries.iter().find_map(|entry| match entry {
            CatalogEntryDto::Book(book) if book.slug == slug => Some(book),
            CatalogEntryDto::Book(_) => None,
            CatalogEntryDto::Category(category) => dfs(&category.entries, slug),
        })
    }
    dfs(&index.entries, slug)
}

/// Recursive lesson-leaf count — the card's "N lessons" line.
pub fn lesson_count(book: &BookDto) -> usize {
    reading_order(book).len()
}

/// DIRECT chapter children only (the oracle counts top-level chapters on the card).
pub fn chapter_count(book: &BookDto) -> usize {
    book.entries
        .iter()
        .filter(|entry| matches!(entry, BookEntryDto::Chapter(_)))
        .count()
}

/// One hop of a click's composed path, target-first: `(tag_name, class_attr, data_id)`.
pub type C4PathHop = (String, String, Option<String>);

/// Resolve a click inside the LikeC4 viewer to an element FQN (oracle: `C4NodeResolver`).
/// Walking target-first: a `<button>` BEFORE the node is one of LikeC4's own controls
/// (relationships/details) — let the viewer keep it. A node must carry the EXACT
/// `react-flow__node` class token (edges carry random-hash ids but not the token) and a
/// non-empty `data-id` — the dotted element FQN.
pub fn resolve_c4_node(path: &[C4PathHop]) -> Option<String> {
    for (tag, classes, data_id) in path {
        if tag.eq_ignore_ascii_case("button") {
            return None;
        }
        let is_node = classes.split_whitespace().any(|c| c == "react-flow__node");
        if is_node {
            return data_id.clone().filter(|id| !id.is_empty());
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// PROBLEM CONTENT SPLIT (oracle: ProblemContent) — the first `<details` at line
// start OUTSIDE a code fence divides description from editorial.
// ─────────────────────────────────────────────────────────────────────────────

pub fn problem_content_split(raw: &str) -> (String, String) {
    let lines: Vec<&str> = raw.lines().collect();
    let mut in_fence = false;
    let mut boundary: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && line.starts_with("<details") {
            boundary = Some(i);
            break;
        }
    }
    match boundary {
        Some(at) => (
            lines[..at].join("\n").trim_end().to_owned(),
            lines[at..].join("\n"),
        ),
        None => (raw.to_owned(), String::new()),
    }
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;

// ─────────────────────────────────────────────────────────────────────────────
// SIDEBAR FILTER (oracle: SidebarFilter) — case-insensitive substring on titles.
// A matching chapter keeps ALL its lessons; otherwise it survives only through
// surviving descendants.
// ─────────────────────────────────────────────────────────────────────────────

pub fn prune_entries(entries: &[BookEntryDto], query: &str) -> Vec<BookEntryDto> {
    fn walk(entries: &[BookEntryDto], needle: &str) -> Vec<BookEntryDto> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                BookEntryDto::Lesson(lesson) => lesson
                    .title
                    .to_lowercase()
                    .contains(needle)
                    .then(|| entry.clone()),
                BookEntryDto::Chapter(chapter) => {
                    if chapter.title.to_lowercase().contains(needle) {
                        return Some(entry.clone());
                    }
                    let kids = walk(&chapter.entries, needle);
                    (!kids.is_empty()).then(|| {
                        BookEntryDto::Chapter(synapse_shared::catalog::ChapterDto {
                            entries: kids,
                            ..chapter.clone()
                        })
                    })
                }
            })
            .collect()
    }
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return entries.to_vec();
    }
    walk(entries, &needle)
}

// ─────────────────────────────────────────────────────────────────────────────
// MINIMAP SPREAD (oracle: ReaderMiniMap.spread) — de-overlap heading fractions:
// min gap 0.05 (capped 1/(n+1)); forward pass pushes apart, backward clamps.
// ─────────────────────────────────────────────────────────────────────────────

pub fn spread_fractions(fractions: &[f64]) -> Vec<f64> {
    let n = fractions.len();
    if n == 0 {
        return Vec::new();
    }
    #[allow(clippy::cast_precision_loss)]
    let gap = f64::min(0.05, 1.0 / (n as f64 + 1.0));
    let mut out: Vec<f64> = fractions.to_vec();
    out.sort_by(f64::total_cmp);
    for i in 1..n {
        if out[i] < out[i - 1] + gap {
            out[i] = out[i - 1] + gap;
        }
    }
    for i in (0..n).rev() {
        let ceiling = 1.0 - gap - {
            #[allow(clippy::cast_precision_loss)]
            let above = (n - 1 - i) as f64;
            above * gap
        };
        if out[i] > ceiling {
            out[i] = ceiling;
        }
        if i > 0 && out[i] < out[i - 1] + gap {
            out[i - 1] = out[i] - gap;
        }
    }
    for value in &mut out {
        *value = value.clamp(gap, 1.0 - gap);
    }
    out
}
