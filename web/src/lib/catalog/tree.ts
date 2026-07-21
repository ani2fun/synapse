// Pure catalog navigation over the WIRE DTOs (oracle: client/src/catalog/logic/mod.rs — the
// `logic` layer, no DOM, no fetch). A03 ports only the three walk helpers the SSR sidebar needs
// — `bookPrefix`, `readingOrder`, `bookOf` — faithfully from the Rust.
//
// A04 formally owns this module and MUST add the 18 parity tests the Rust `logic_tests.rs`
// carries (the reading-order pre-order, the nested-category `bookOf`, the book-prefix shape).
// They are deliberately NOT written here: A04's parity ledger, not A03's.

import type { components } from "../api/schema.gen";

type SynapseIndex = components["schemas"]["SynapseIndexDto"];
type CatalogEntry = components["schemas"]["CatalogEntryDto"];
type Book = components["schemas"]["BookDto"];
type BookEntry = components["schemas"]["BookEntryDto"];
type Lesson = components["schemas"]["LessonDto"];

/** One lesson in reading order: its full directory-mirror path and the lesson itself. */
export interface ReadingOrderEntry {
  path: string;
  lesson: Lesson;
}

/** A book's URL prefix segments: `categoryPath + slug`. (oracle: `book_prefix`) */
export function bookPrefix(book: Book): string[] {
  return [...book.categoryPath, book.slug];
}

/**
 * Every lesson of a book with its FULL directory-mirror path, pre-order — the sidebar's and the
 * library card's source of truth. (oracle: `reading_order`)
 */
export function readingOrder(book: Book): ReadingOrderEntry[] {
  const out: ReadingOrderEntry[] = [];
  const collect = (entries: BookEntry[], prefix: string[]): void => {
    for (const entry of entries) {
      if (entry.kind === "lesson") {
        out.push({ path: [...prefix, entry.slug].join("/"), lesson: entry });
      } else {
        collect(entry.entries, [...prefix, entry.slug]);
      }
    }
  };
  collect(book.entries, bookPrefix(book));
  return out;
}

/**
 * The book a lesson path belongs to: the entry whose `categoryPath + slug` prefixes the path.
 * A book matches on its first segment and returns immediately; a category matches and recurses
 * with the rest — so a category-nested book (`programming-languages/python/…`) still resolves.
 * (oracle: `book_of`)
 */
export function bookOf(index: SynapseIndex, lessonPath: string[]): Book | null {
  const find = (entries: CatalogEntry[], path: string[]): Book | null => {
    if (path.length === 0) return null;
    const [first, ...rest] = path;
    for (const entry of entries) {
      if (entry.kind === "book" && entry.slug === first) return entry;
      if (entry.kind === "category" && entry.slug === first) return find(entry.entries, rest);
    }
    return null;
  };
  return find(index.entries, lessonPath);
}
