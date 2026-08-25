//! Composing N walked sources into ONE catalog.
//!
//! `walker` interprets a single tree's conventions and is strict — a duplicate slug inside one
//! checkout is an authoring bug and fails. This module does a different job: it takes catalogs
//! that were each already valid and resolves what only shows up when they meet. Those conflicts
//! are survivable by design, so they come back as `WalkResult::warnings` rather than errors —
//! refusing to serve the whole library because one satellite clashes is worse than serving the
//! winner and saying so.
//!
//! Source ORDER is the caller's and it is load-bearing: the primary checkout comes first,
//! satellites follow. That is what makes a content migration safe in either direction — while a
//! book exists both in the monorepo and in its new repo, the monorepo's copy is the one that
//! serves, so the satellite can be verified before anything is deleted.

use std::collections::{BTreeMap, BTreeSet};

use crate::catalog::domain::catalog::{
    Book, CatalogEntry, CatalogWarning, Category, LessonFileRef, SynapseContentError, WalkResult,
};
use crate::catalog::domain::content_tree::SourceTree;
use crate::catalog::domain::walker::{self, humanise};

type BookFiles = BTreeMap<String, BTreeMap<String, LessonFileRef>>;

/// Where a source's content lands in the library. A source with no placement keeps whatever
/// position its own directory structure implies — which is exactly the primary checkout, whose
/// nesting IS the library.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Placement {
    pub source_id: String,
    /// Category slug path to graft under; empty means the top level.
    pub grouping: Vec<String>,
    /// The registered book's position, and the ONLY thing that decides it — see [`OrderedBy`].
    pub order: Option<i32>,
}

/// Who decides where a book sits. Exactly one authority per book, never a fallback between two.
///
/// The two used to blur together: a placement's `order` overrode `book.json` when present, and
/// silently deferred to it when absent. A registration that said nothing about position therefore
/// inherited whatever number the repository last happened to carry — so the library's order had
/// two sources of truth that could disagree, with nothing to report when they did. Since a book
/// can migrate between repositories, the stale one is exactly the one nobody thinks to look at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderedBy {
    /// The source's own `book.json`. Two cases: the walked spine, whose directory nesting IS the
    /// library, and any book nested inside a category a satellite brought with it — a placement
    /// positions the source, not the shelves inside it.
    BookJson,
    /// The registration row, whatever it says — INCLUDING nothing. A row without an order means
    /// unpositioned, not "whatever the repository last said", which is what makes deleting a
    /// satellite's `order` field a no-op rather than a reshuffle.
    Row(Option<i32>),
}

impl OrderedBy {
    fn apply(self, book: &mut Book) {
        if let Self::Row(order) = self {
            book.order = order;
        }
    }
}

/// Walk every source, then compose. Per-source rules stay strict; cross-source ones warn.
pub fn assemble(sources: &[SourceTree], placements: &[Placement]) -> Result<WalkResult, SynapseContentError> {
    let mut merge = Merge::default();
    for (index, source) in sources.iter().enumerate() {
        let walk = walker::walk_source(source)?;
        let placement = placements.iter().find(|p| p.source_id == source.id);
        merge.absorb_source(&source.id, placement, walk, index == 0);
    }
    Ok(merge.finish())
}

#[derive(Default)]
struct Merge {
    out: WalkResult,
    /// Book slug → the source that won it, for the duplicate warning.
    owner: BTreeMap<String, String>,
    /// Category slug path → the source whose `category.json` DECLARED it. Only these contest each
    /// other; a path absent here is still upgradable by a real declaration.
    declared: BTreeMap<Vec<String>, String>,
    /// Category slug paths whose metadata has already been applied, declared or not. A directory
    /// name furnishes a title and an `NN-` order too, and the first source to supply them wins —
    /// but supplying them is not a claim, so it must not make the next source look like a rival.
    furnished: BTreeSet<Vec<String>>,
    /// Levels a later source inserted into, and which therefore need re-sorting.
    touched: BTreeSet<Vec<String>>,
}

impl Merge {
    fn absorb_source(
        &mut self,
        source_id: &str,
        placement: Option<&Placement>,
        walk: WalkResult,
        is_first: bool,
    ) {
        self.out.warnings.extend(walk.warnings);
        let grouping = placement.map(|p| p.grouping.clone()).unwrap_or_default();
        // A placement IS a registration row, and its absence IS the walked spine — so which
        // authority applies is already settled here, once, rather than re-derived per book.
        let ordered_by = placement.map_or(OrderedBy::BookJson, |p| OrderedBy::Row(p.order));
        let mut files = walk.lesson_files;
        for entry in walk.catalog.entries {
            self.absorb(source_id, &grouping, &mut files, ordered_by, entry, is_first);
        }
    }

    fn absorb(
        &mut self,
        source_id: &str,
        path: &[String],
        files: &mut BookFiles,
        ordered_by: OrderedBy,
        entry: CatalogEntry,
        is_first: bool,
    ) {
        match entry {
            CatalogEntry::Book(mut book) => {
                ordered_by.apply(&mut book);
                self.graft_book(source_id, path, files, book, is_first);
            }
            CatalogEntry::Category(category) => {
                let mut inner = path.to_vec();
                inner.push(category.slug.clone());
                self.adopt_declaration(source_id, &inner, &category);
                for child in category.entries {
                    // A placement positions the SOURCE, not the shelves inside it: a book nested
                    // in a category the satellite brought with it is ordered by its own metadata.
                    self.absorb(source_id, &inner, files, OrderedBy::BookJson, child, is_first);
                }
            }
        }
    }

    /// First source wins a contested slug. Its `lesson_files` entry must be left untouched:
    /// overwriting that map is the subtle version of this bug, where the kept book keeps its
    /// catalog entry but starts serving the SKIPPED book's file paths.
    fn graft_book(
        &mut self,
        source_id: &str,
        path: &[String],
        files: &mut BookFiles,
        mut book: Book,
        is_first: bool,
    ) {
        let book_files = files.remove(&book.slug).unwrap_or_default();
        if let Some(kept_source) = self.owner.get(&book.slug) {
            self.out.warnings.push(CatalogWarning::DuplicateBookSlug {
                slug: book.slug.clone(),
                kept_source: kept_source.clone(),
                skipped_source: source_id.to_owned(),
            });
            return;
        }

        // `resolver::book_prefix` builds every URL — the index, prev/next, the sitemap — from
        // `category_path`. A book grafted without rewriting it is linked at one path and listed
        // at another, and only prev/next makes the mismatch visible. `path` is authoritative and
        // already absolute: the recursion has reproduced the source's own nesting on top of the
        // grouping, so the walker's within-source value is replaced, never appended to.
        book.category_path = path.to_vec();

        self.owner.insert(book.slug.clone(), source_id.to_owned());
        self.out.lesson_files.insert(book.slug.clone(), book_files);
        if !is_first {
            self.touched.insert(path.to_vec());
        }
        self.level_at(path).push(CatalogEntry::Book(book));
    }

    /// First metadata wins, whole-node: per-field merging would make "which repo owns this
    /// category's icon" unanswerable.
    ///
    /// Two rules, deliberately separate. A real `category.json` OUTRANKS whatever a directory name
    /// furnished, so a declaration arriving after a synthesized node upgrades it. But only two
    /// declarations are a conflict — two sources that merely nest the same grouping are just
    /// agreeing about where a book lives, and warning about that would report a dispute nobody is
    /// having.
    fn adopt_declaration(&mut self, source_id: &str, path: &[String], category: &Category) {
        if category.declared {
            if let Some(kept) = self.declared.get(path) {
                self.out.warnings.push(CatalogWarning::CategoryRedeclared {
                    slug: category.slug.clone(),
                    kept_source: kept.clone(),
                    ignored_source: source_id.to_owned(),
                });
                return;
            }
            self.declared.insert(path.to_vec(), source_id.to_owned());
        } else if self.furnished.contains(path) {
            return;
        }

        self.furnished.insert(path.to_vec());
        if let Some(node) = self.category_at(path) {
            node.title.clone_from(&category.title);
            node.description.clone_from(&category.description);
            node.icon.clone_from(&category.icon);
            node.order = category.order;
            node.declared = category.declared;
        }
    }

    /// The entry list at a slug path, creating synthesized categories along the way. A synthesized
    /// node carries no icon and no order, so it sorts last until some source declares it.
    fn level_at(&mut self, path: &[String]) -> &mut Vec<CatalogEntry> {
        let mut entries = &mut self.out.catalog.entries;
        for slug in path {
            let index = category_index(entries, slug);
            entries = match &mut entries[index] {
                CatalogEntry::Category(category) => &mut category.entries,
                CatalogEntry::Book(_) => unreachable!("category_index only ever returns a category"),
            };
        }
        entries
    }

    /// The category node itself, for adopting a declaration onto. `None` only for an empty path —
    /// the library root is not a category.
    fn category_at(&mut self, path: &[String]) -> Option<&mut Category> {
        let (last, parents) = path.split_last()?;
        let entries = self.level_at(parents);
        let index = category_index(entries, last);
        match &mut entries[index] {
            CatalogEntry::Category(category) => Some(category),
            CatalogEntry::Book(_) => None,
        }
    }

    fn finish(mut self) -> WalkResult {
        // Only levels a graft touched are re-sorted. `build_level` ordered by DIRECTORY name,
        // which survives on neither `Category` nor `Book`, so re-sorting an untouched level would
        // silently reshuffle it by slug. Leaving them alone is also what makes merge-of-one the
        // exact identity, which the walker's own suites then pin.
        for path in std::mem::take(&mut self.touched) {
            sort_level(self.level_at(&path));
        }
        prune_empty(&mut self.out.catalog.entries);
        self.out
    }
}

/// The index of the category with this slug, synthesizing one at the end if absent.
fn category_index(entries: &mut Vec<CatalogEntry>, slug: &str) -> usize {
    if let Some(index) = entries
        .iter()
        .position(|e| matches!(e, CatalogEntry::Category(c) if c.slug == slug))
    {
        return index;
    }
    entries.push(CatalogEntry::Category(Category {
        slug: slug.to_owned(),
        declared: false,
        title: humanise(slug),
        description: None,
        icon: None,
        order: None,
        entries: Vec::new(),
    }));
    entries.len() - 1
}

fn sort_level(entries: &mut [CatalogEntry]) {
    entries.sort_by(|a, b| (order_of(a), a.slug()).cmp(&(order_of(b), b.slug())));
}

fn order_of(entry: &CatalogEntry) -> i32 {
    match entry {
        CatalogEntry::Category(c) => c.order.unwrap_or(i32::MAX),
        CatalogEntry::Book(b) => b.order.unwrap_or(i32::MAX),
    }
}

/// Empty categories vanish, matching the walker's own rule — a grouping whose only book lost a
/// slug collision must not survive as a bare heading.
fn prune_empty(entries: &mut Vec<CatalogEntry>) {
    entries.retain_mut(|entry| match entry {
        CatalogEntry::Book(_) => true,
        CatalogEntry::Category(category) => {
            prune_empty(&mut category.entries);
            !category.entries.is_empty()
        }
    });
}

#[cfg(test)]
mod tests;
