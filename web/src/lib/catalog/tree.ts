// Pure catalog navigation over the WIRE DTOs — no DOM, no fetch.
//
// Related pure logic lives in its own module rather than here: `editorial.ts` (the editorial-tab
// parser, feeding the problem workbench), `pane.ts` (the two-pane split-percentage clamp, also
// the problem workbench), `prefs.ts` (the four-field reading-preferences pack, feeding the FAB),
// and `progress.ts` (reading progress, feeding the library landing directly).

import type { components } from "../api/schema.gen";

type SynapseIndex = components["schemas"]["SynapseIndexDto"];
type CatalogEntry = components["schemas"]["CatalogEntryDto"];
type Book = components["schemas"]["BookDto"];
type BookEntry = components["schemas"]["BookEntryDto"];
type Lesson = components["schemas"]["LessonDto"];
type Chapter = components["schemas"]["ChapterDto"];

/** One lesson in reading order: its full directory-mirror path and the lesson itself. */
export interface ReadingOrderEntry {
  path: string;
  lesson: Lesson;
}

/** A book's URL prefix segments: `categoryPath + slug`. */
export function bookPrefix(book: Book): string[] {
  return [...book.categoryPath, book.slug];
}

/**
 * Every lesson of a book with its FULL directory-mirror path, pre-order — the sidebar's and the
 * library card's source of truth.
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
 * The chapter a lesson belongs to FOR COUNTING purposes — its path minus the last segment,
 * except when that chapter exists only to hold it. Kept private — `chapterProblems` is the
 * only caller.
 *
 * Most problems are authored as `…/problems/<slug>/<slug>.md`, giving each one a chapter of its
 * own. Scoping on the raw parent would make every problem read "1 / 1" — so when the parent
 * segment and the lesson slug match, the real chapter is one level up.
 */
function countingChapter(path: string): string | null {
  const lastSlash = path.lastIndexOf("/");
  if (lastSlash === -1) return null;
  const parent = path.slice(0, lastSlash);
  const slug = path.slice(lastSlash + 1);
  const secondSlash = parent.lastIndexOf("/");
  if (secondSlash === -1) return parent;
  const grandparent = parent.slice(0, secondSlash);
  const chapter = parent.slice(secondSlash + 1);
  return chapter === slug ? grandparent : parent;
}

/**
 * The problems of `lessonPath`'s own chapter in reading order, and where that path sits among
 * them. `null` when the path isn't in the book, or isn't itself a problem.
 */
export function chapterProblems(book: Book, lessonPath: string): { problems: string[]; at: number } | null {
  const here = countingChapter(lessonPath);
  if (here === null) return null;
  const problems = readingOrder(book)
    .filter(({ path, lesson }) => lesson.lessonKind === "problem" && countingChapter(path) === here)
    .map(({ path }) => path);
  const at = problems.indexOf(lessonPath);
  return at === -1 ? null : { problems, at };
}

/** Where a book's cover card points: its first lesson in reading order. */
export function firstLessonPath(book: Book): string | null {
  const order = readingOrder(book);
  return order.length > 0 ? order[0].path : null;
}

/**
 * The title of the chapter a lesson sits in — its nearest chapter ancestor — or `null` when the
 * lesson hangs directly off the book root. Feeds the reader's eyebrow (the "you are here" line
 * above the lesson title).
 */
export function chapterTitleOf(book: Book, lessonPath: string): string | null {
  let found: string | null = null;
  const walk = (entries: BookEntry[], prefix: string[], chapter: string | null): void => {
    for (const entry of entries) {
      const path = [...prefix, entry.slug].join("/");
      if (entry.kind === "lesson") {
        if (path === lessonPath) found = chapter;
      } else {
        walk(entry.entries, [...prefix, entry.slug], entry.title);
      }
    }
  };
  walk(book.entries, bookPrefix(book), null);
  return found;
}

/**
 * The book a lesson path belongs to: the entry whose `categoryPath + slug` prefixes the path.
 * A book matches on its first segment and returns immediately; a category matches and recurses
 * with the rest — so a category-nested book (`programming-languages/python/…`) still resolves.
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

/** Every book in the catalog, DFS through categories — the reader's "Back to Main" browse list. */
export function allBooksOf(index: SynapseIndex): Book[] {
  const out: Book[] = [];
  const walk = (entries: CatalogEntry[]): void => {
    for (const entry of entries) {
      if (entry.kind === "book") out.push(entry);
      else walk(entry.entries);
    }
  };
  walk(index.entries);
  return out;
}

/** The book with a globally-unique slug, DFS through categories. */
export function findBook(index: SynapseIndex, slug: string): Book | null {
  const dfs = (entries: CatalogEntry[]): Book | null => {
    for (const entry of entries) {
      if (entry.kind === "book") {
        if (entry.slug === slug) return entry;
      } else {
        const found = dfs(entry.entries);
        if (found) return found;
      }
    }
    return null;
  };
  return dfs(index.entries);
}

/** Recursive lesson-leaf count — the card's "N lessons" line. */
export function lessonCount(book: Book): number {
  return readingOrder(book).length;
}

/** DIRECT chapter children only — the card counts top-level chapters, not nested ones. */
export function chapterCount(book: Book): number {
  return book.entries.filter((entry) => entry.kind === "chapter").length;
}

// ─────────────────────────────────────────────────────────────────────────────
// PROBLEM CONTENT SPLIT — the first `<details` at line start OUTSIDE a code fence divides
// description from editorial.
// ─────────────────────────────────────────────────────────────────────────────

export function problemContentSplit(raw: string): [description: string, editorial: string] {
  const lines = raw.split("\n");
  let inFence = false;
  let boundary: number | null = null;
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (line.trimStart().startsWith("```")) inFence = !inFence;
    if (!inFence && line.startsWith("<details")) {
      boundary = i;
      break;
    }
  }
  if (boundary === null) return [raw, ""];
  const description = lines.slice(0, boundary).join("\n").trimEnd();
  const editorial = lines.slice(boundary).join("\n");
  return [description, editorial];
}

// ─────────────────────────────────────────────────────────────────────────────
// SIDEBAR FILTER — case-insensitive substring on titles. A matching chapter keeps ALL its
// lessons; otherwise it survives only through surviving descendants.
// ─────────────────────────────────────────────────────────────────────────────

export function pruneEntries(entries: BookEntry[], query: string): BookEntry[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return entries;

  const walk = (nodes: BookEntry[]): BookEntry[] => {
    const out: BookEntry[] = [];
    for (const entry of nodes) {
      if (entry.kind === "lesson") {
        if (entry.title.toLowerCase().includes(needle)) out.push(entry);
        continue;
      }
      if (entry.title.toLowerCase().includes(needle)) {
        out.push(entry);
        continue;
      }
      const kids = walk(entry.entries);
      if (kids.length > 0) {
        const pruned: Chapter & { kind: "chapter" } = { ...entry, entries: kids };
        out.push(pruned);
      }
    }
    return out;
  };
  return walk(entries);
}

