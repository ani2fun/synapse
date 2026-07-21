// Pure catalog navigation over the WIRE DTOs (oracle: client/src/catalog/logic/mod.rs — the
// `logic` layer, no DOM, no fetch). A03 ported only the three walk helpers the SSR sidebar
// needed — `bookPrefix`, `readingOrder`, `bookOf`. A04 pays down the rest of the debt: every
// function `mod.rs` exports at its own file scope, ported faithfully, with the 18 parity tests
// `logic_tests.rs` carries (see tree.test.ts, same case names in camelCase).
//
// NOT ported here (mod.rs's SUBMODULES, each with its own test file the Rust side already
// covers, and each feeding a page or island this branch has not reached yet):
//   - editorial.rs  — the editorial-tab parser; arrives with the problem workbench (A07).
//   - pane.rs        — the two-pane split-percentage clamp; also A07.
//   - prefs.rs       — the four-field reading-preferences pack; the FAB it feeds is A05.
// `progress.rs` is the one submodule THIS step does own — it ported separately, to
// `web/src/lib/catalog/progress.ts`, because the library landing (A04's other deliverable)
// needs it directly.

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
 * The chapter a lesson belongs to FOR COUNTING purposes — its path minus the last segment,
 * except when that chapter exists only to hold it. (oracle: `counting_chapter`, private there —
 * kept private here too, `chapterProblems` is the only caller.)
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
 * them. `null` when the path isn't in the book, or isn't itself a problem. (oracle:
 * `chapter_problems`)
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

/** Where a book's cover card points: its first lesson in reading order. (oracle: `first_lesson_path`) */
export function firstLessonPath(book: Book): string | null {
  const order = readingOrder(book);
  return order.length > 0 ? order[0].path : null;
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

/** The book with a globally-unique slug, DFS through categories. (oracle: `find_book`) */
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

/** Recursive lesson-leaf count — the card's "N lessons" line. (oracle: `lesson_count`) */
export function lessonCount(book: Book): number {
  return readingOrder(book).length;
}

/** DIRECT chapter children only (the oracle counts top-level chapters on the card). (oracle: `chapter_count`) */
export function chapterCount(book: Book): number {
  return book.entries.filter((entry) => entry.kind === "chapter").length;
}

/** One hop of a click's composed path, target-first: `[tagName, classAttr, dataId]`. */
export type C4PathHop = [tag: string, classes: string, dataId: string | null];

/**
 * Resolve a click inside the LikeC4 viewer to an element FQN (oracle: `resolve_c4_node` /
 * `C4NodeResolver`). Walking target-first: a `<button>` BEFORE the node is one of LikeC4's own
 * controls (relationships/details) — let the viewer keep it. A node must carry the EXACT
 * `react-flow__node` class token (edges carry random-hash ids but not the token) and a
 * non-empty `data-id` — the dotted element FQN.
 */
export function resolveC4Node(path: C4PathHop[]): string | null {
  for (const [tag, classes, dataId] of path) {
    if (tag.toLowerCase() === "button") return null;
    const isNode = classes.split(/\s+/).some((c) => c === "react-flow__node");
    if (isNode) return dataId && dataId.length > 0 ? dataId : null;
  }
  return null;
}

// ─────────────────────────────────────────────────────────────────────────────
// PROBLEM CONTENT SPLIT (oracle: `problem_content_split` / `ProblemContent`) — the first
// `<details` at line start OUTSIDE a code fence divides description from editorial.
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
// SIDEBAR FILTER (oracle: `prune_entries` / `SidebarFilter`) — case-insensitive substring on
// titles. A matching chapter keeps ALL its lessons; otherwise it survives only through
// surviving descendants.
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

// ─────────────────────────────────────────────────────────────────────────────
// MINIMAP SPREAD (oracle: `spread_fractions` / `ReaderMiniMap.spread`) — de-overlap heading
// fractions: min gap 0.05 (capped 1/(n+1)); forward pass pushes apart, backward clamps.
// ─────────────────────────────────────────────────────────────────────────────

export function spreadFractions(fractions: number[]): number[] {
  const n = fractions.length;
  if (n === 0) return [];
  const gap = Math.min(0.05, 1 / (n + 1));
  const out = [...fractions].sort((a, b) => a - b);
  for (let i = 1; i < n; i += 1) {
    if (out[i] < out[i - 1] + gap) out[i] = out[i - 1] + gap;
  }
  for (let i = n - 1; i >= 0; i -= 1) {
    const above = n - 1 - i;
    const ceiling = 1 - gap - above * gap;
    if (out[i] > ceiling) out[i] = ceiling;
    if (i > 0 && out[i] < out[i - 1] + gap) out[i - 1] = out[i] - gap;
  }
  return out.map((value) => Math.min(Math.max(value, gap), 1 - gap));
}
