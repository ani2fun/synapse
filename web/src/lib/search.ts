// Pure search over the flattened library (oracle: client/src/search/logic/mod.rs): every
// lesson, every book (linked to its first lesson), every blog post — ranked prefix (100) >
// word-start (80) > substring (60) > subsequence (30), with a +10 bonus for matching the LABEL
// over the breadcrumb, kind as the tiebreak (lessons first), shorter labels before longer.
//
// No DOM, no fetch — the palette island (`islands/palette.ts`) is the only caller, and it feeds
// this whatever `fetchIndex()`/`blogList()` already resolved.

import type { Page } from "./routes";
import type { components } from "./api/schema.gen";

type SynapseIndex = components["schemas"]["SynapseIndexDto"];
type CatalogEntry = components["schemas"]["CatalogEntryDto"];
type Book = components["schemas"]["BookDto"];
type BookEntry = components["schemas"]["BookEntryDto"];
type BlogSummary = components["schemas"]["BlogSummaryDto"];

/** oracle: `search::logic::Kind` */
export type SearchKind = "lesson" | "book" | "blog";

export interface SearchEntry {
  label: string;
  sublabel: string;
  kind: SearchKind;
  page: Page;
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
    case "book":
      return 1;
    case "blog":
      return 2;
  }
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
