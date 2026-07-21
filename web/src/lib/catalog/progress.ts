// Reading progress — pure, so vitest covers it natively; nothing here touches `localStorage`
// directly (that is `../storage.ts`'s job — this module only parses/serialises the string
// storage hands back and does the reading-order math).
//
// Two facts are kept, and they are deliberately in SEPARATE localStorage keys rather than one
// packed record: the set of lessons finished (`reader-progress`), and the last lesson opened
// (`reader-last`). `prefs.ts` packs four fields into one `|`-joined string parsed by an
// exact-arity slice pattern, which means adding a fifth field silently resets every existing
// reader's saved settings — that trap is not rebuilt here: one key, one job, and a key that
// fails to parse costs only itself.
//
// The done-set is newline-separated because it is a LIST, not a fixed record — there is no
// arity to get wrong, and an unrecognised or blank line is skipped rather than poisoning the
// rest. Lesson paths are `/`-joined slugs, so they can contain neither a newline nor a `|`.

import { readingOrder } from "./tree";
import type { components } from "../api/schema.gen";

type Book = components["schemas"]["BookDto"];

/** How far down a lesson counts as "read to the end". Not 1.0: the last pixel is unreachable on
 *  many devices (rubber-banding, a footer inside the scroll container, sub-pixel rounding), and
 *  a threshold nobody can cross is a feature nobody has. */
export const END_THRESHOLD = 0.98;

/**
 * Has the reader reached the end, given the scroll offset and the scrollable track?
 *
 * Both traps are handled explicitly:
 * - `track <= 0` means the lesson is SHORTER than the viewport. A naive `scroll / track` ratio
 *   pins at 0 there and the reader can never finish a short lesson. There is nothing to scroll,
 *   which is precisely the case where it has all been seen.
 * - a non-finite ratio (0/0) is not "at the end" — it is a page that has not laid out yet.
 */
export function isAtEnd(scroll: number, track: number): boolean {
  if (track <= 0) return true;
  const ratio = scroll / track;
  return Number.isFinite(ratio) && ratio >= END_THRESHOLD;
}

/** Read the completed set. Absent or unreadable storage is an empty set — never an error, and
 *  never a partial parse: progress is a convenience, and losing it must not break the reader. */
export function parse(stored: string | null | undefined): Set<string> {
  const lines = (stored ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
  return new Set(lines);
}

/** Sorted so the serialised form is stable — an unordered set would rewrite the whole value on
 *  every commit and make the stored string churn for no reason. */
export function serialize(done: Set<string>): string {
  return [...done].sort().join("\n");
}

/** How many of a book's lessons are finished. Counts against `readingOrder`, which is the same
 *  list the sidebar and the card already use, so the denominator can never disagree with what
 *  the reader can see. */
export function completedCount(book: Book, done: Set<string>): number {
  return readingOrder(book).filter(({ path }) => done.has(path)).length;
}

/** The first unfinished lesson of a book, in reading order — `null` when the book is finished. */
export function nextUnread(book: Book, done: Set<string>): string | null {
  const entry = readingOrder(book).find(({ path }) => !done.has(path));
  return entry ? entry.path : null;
}
