// Pure search over the flattened library: every lesson, every book (linked to its first
// lesson), every blog post — ranked prefix (100) > word-start (80) > substring (60) >
// subsequence (30), with a +10 bonus for matching the LABEL over the breadcrumb, kind as the
// tiebreak (lessons first), shorter labels before longer.
//
// That covers "I know the title" at zero latency, and nothing else: titles and breadcrumbs are
// the only strings the browsable index carries. `merge()` below folds in the server's full-text
// hits, which see the prose.
//
// No DOM, no fetch — the palette island (`islands/palette.ts`) is the only caller, and it feeds
// this whatever `fetchIndex()`/`blogList()`/`searchCatalog()` already resolved.

import { pageUrl, segmentsOf, type Page } from "./routes";
import type { components } from "./api/schema.gen";

type SynapseIndex = components["schemas"]["SynapseIndexDto"];
type CatalogEntry = components["schemas"]["CatalogEntryDto"];
type Book = components["schemas"]["BookDto"];
type BookEntry = components["schemas"]["BookEntryDto"];
type BlogSummary = components["schemas"]["BlogSummaryDto"];
type SearchHit = components["schemas"]["SearchHitDto"];
type SnippetSegment = components["schemas"]["SnippetSegmentDto"];

export type SearchKind = "lesson" | "book" | "blog" | "editorial";

export interface SearchEntry {
  label: string;
  sublabel: string;
  kind: SearchKind;
  page: Page;
  /** A quote from the prose, pre-split into matched and unmatched runs. Only a full-text hit
   *  carries one — the browsable index has no body to quote. */
  snippet?: SnippetSegment[];
  /** A fragment appended to the page URL, for destinations that are a TAB rather than a page.
   *  Server-side an editorial has no URL of its own; how a problem page is laid out is knowledge
   *  that belongs here, not on the wire. */
  hash?: string;
}

/** Where a row goes. The hash is part of the destination, not decoration: two rows that differ
 *  only by it are two different places, and dedup has to agree with navigation about that. */
export function entryUrl(entry: SearchEntry): string {
  return `${pageUrl(entry.page)}${entry.hash ?? ""}`;
}

/** Flatten the whole library into searchable entries. */
export function entries(index: SynapseIndex, blog: BlogSummary[]): SearchEntry[] {
  const all: SearchEntry[] = [];
  flattenCatalog(index.entries, [], all);
  for (const post of blog) {
    all.push({
      label: post.title,
      sublabel: "Blog",
      kind: "blog",
      page: { kind: "blogPost", slug: post.slug },
    });
  }
  return all;
}

function flattenCatalog(entries: CatalogEntry[], crumb: string[], out: SearchEntry[]): void {
  for (const entry of entries) {
    if (entry.kind === "category") {
      flattenCatalog(entry.entries, [...crumb, entry.title], out);
    } else {
      flattenBook(entry, crumb, out);
    }
  }
}

function flattenBook(book: Book, crumb: string[], out: SearchEntry[]): void {
  // The book itself: one entry linked to its first lesson (depth-first).
  const first = firstLessonPath(book);
  if (first) {
    out.push({
      label: book.title,
      sublabel: [...crumb, "Book"].join(" › "),
      kind: "book",
      page: { kind: "lesson", path: first },
    });
  }
  const bookCrumb = [...crumb, book.title];
  const prefix = [...book.categoryPath, book.slug];
  flattenEntries(book.entries, bookCrumb, prefix, out);
}

function flattenEntries(entries: BookEntry[], crumb: string[], prefix: string[], out: SearchEntry[]): void {
  for (const entry of entries) {
    if (entry.kind === "chapter") {
      flattenEntries(entry.entries, [...crumb, entry.title], [...prefix, entry.slug], out);
    } else {
      const path = [...prefix, entry.slug];
      out.push({
        label: entry.title,
        sublabel: crumb.join(" › "),
        kind: "lesson",
        page: { kind: "lesson", path },
      });
    }
  }
}

function firstLessonPath(book: Book): string[] | null {
  const dive = (entries: BookEntry[], prefix: string[]): string[] | null => {
    for (const entry of entries) {
      if (entry.kind === "lesson") return [...prefix, entry.slug];
      const found = dive(entry.entries, [...prefix, entry.slug]);
      if (found) return found;
    }
    return null;
  };
  return dive(book.entries, [...book.categoryPath, book.slug]);
}

export const LIMIT = 20;

/** Rank and cap. An empty query returns everything (capped); a no-match query returns nothing. */
export function search(query: string, all: SearchEntry[]): SearchEntry[] {
  const q = query.trim();
  if (q === "") return all.slice(0, LIMIT);

  const ranked = all
    .map((entry) => ({ entry, score: rank(q, entry) }))
    .filter((r): r is { entry: SearchEntry; score: number } => r.score !== null);

  ranked.sort((a, b) => {
    if (a.score !== b.score) return b.score - a.score;
    const ka = kindOrder(a.entry.kind);
    const kb = kindOrder(b.entry.kind);
    if (ka !== kb) return ka - kb;
    return a.entry.label.length - b.entry.label.length;
  });

  return ranked.slice(0, LIMIT).map((r) => r.entry);
}

function kindOrder(kind: SearchKind): number {
  switch (kind) {
    case "lesson":
      return 0;
    case "editorial":
      return 1;
    case "book":
      return 2;
    case "blog":
      return 3;
  }
}

/**
 * A title/breadcrumb match at or above this is a LITERAL one — the query's characters appear in
 * order and unbroken. Below it sits only the subsequence tier, which is fuzzy-typing help: `cts`
 * "matches" *c*on*t*iguou*s* and a dozen other things. Worth keeping, not worth ranking above a
 * word that genuinely appears in a lesson's prose.
 */
const LITERAL = 60;

/**
 * Fold the server's full-text hits into the instant local ones, best first.
 *
 * Three tiers, and the middle one is the point: a literal title or breadcrumb match leads, then
 * prose hits in the server's own ranked order, then the fuzzy subsequence matches. Putting every
 * local hit first would bury a real answer under exactly the structural noise this feature exists
 * to cut through.
 *
 * Deduplicated by DESTINATION, because two rows going to the same page is a bug however differently
 * they were found. When both halves reached the same lesson, the row already placed keeps its
 * position and borrows the quote — a title match that also appears in the prose should show why.
 * The destination INCLUDES the fragment, which is what keeps a problem and its solution — same
 * page, different tabs — from collapsing into one row that can only be right about one of them.
 */
export function merge(query: string, local: SearchEntry[], prose: SearchHit[]): SearchEntry[] {
  const q = query.trim();
  if (q === "") return local.slice(0, LIMIT);

  const out: SearchEntry[] = [];
  const at = new Map<string, number>();
  const add = (entry: SearchEntry): void => {
    const url = entryUrl(entry);
    const seen = at.get(url);
    if (seen === undefined) {
      at.set(url, out.length);
      out.push(entry);
      return;
    }
    const placed = out[seen];
    // A book row links to its first lesson, so it can collide with that lesson's prose hit. It
    // does not get the quote: the row says "Book", and the words are the lesson's.
    if (placed && placed.kind === "lesson" && !placed.snippet && entry.snippet) {
      out[seen] = { ...placed, snippet: entry.snippet };
    }
  };

  const literal = (entry: SearchEntry): boolean => (rank(q, entry) ?? 0) >= LITERAL;
  for (const entry of local) if (literal(entry)) add(entry);
  for (const hit of prose) add(proseEntry(hit));
  for (const entry of local) if (!literal(entry)) add(entry);

  return out.slice(0, LIMIT);
}

/** The fragment a problem page's solution tab answers to. */
export const EDITORIAL_HASH = "#editorial";

function proseEntry(hit: SearchHit): SearchEntry {
  // The wire spells `kind` as an open string; anything unrecognised reads as a lesson, which is
  // the safe way to be wrong — the alternative is a row with no chip at all.
  const editorial = hit.kind === "editorial";
  return {
    label: hit.title,
    sublabel: hit.breadcrumb.join(" › "),
    kind: editorial ? "editorial" : "lesson",
    page: { kind: "lesson", path: segmentsOf(hit.path) },
    snippet: hit.snippet,
    // An editorial shares its problem's URL because it IS that page — a tab on it. The fragment
    // is what makes the row a distinct destination and what lands the reader on the right tab
    // instead of the problem statement they were trying to skip past.
    ...(editorial ? { hash: EDITORIAL_HASH } : {}),
  };
}

/** The label carries a +10 bonus over the breadcrumb; the best of the two wins. */
function rank(query: string, entry: SearchEntry): number | null {
  const onLabel = score(query, entry.label);
  const onCrumb = score(query, entry.sublabel);
  const a = onLabel === null ? null : onLabel + 10;
  if (a === null) return onCrumb;
  if (onCrumb === null) return a;
  return Math.max(a, onCrumb);
}

function score(query: string, text: string): number | null {
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  if (t.startsWith(q)) return 100;
  if (t.split(/[^a-z0-9]+/i).some((word) => word.startsWith(q))) return 80;
  if (t.includes(q)) return 60;
  if (isSubsequence(q, t)) return 30;
  return null;
}

function isSubsequence(query: string, text: string): boolean {
  let want = 0;
  for (const c of text) {
    if (want < query.length && c === query[want]) want += 1;
  }
  return want === query.length;
}
