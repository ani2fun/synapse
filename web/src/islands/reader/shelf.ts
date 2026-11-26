// The sidebar's browse rows, rendered in the browser. The public rail is server-rendered by
// `Sidebar.astro` from the ANONYMOUS index, so a private book is absent from it for everyone;
// the index fetched with the bearer carries the books this reader is listed for, marked
// `private`. This module turns those into the same `.reader-sidebar__book` rows the server
// draws — same classes, same glyph slot — so the private shelf reads as part of one list.
//
// Pure string HTML with every catalog string escaped: titles come from content repositories,
// and they land in `innerHTML`.
import type { SynapseIndex } from "../../lib/api/client";
import type { components } from "../../lib/api/schema.gen";
import { BOOK_ICON, LOCK_ICON } from "../../lib/catalog/sidebarIcons";
import { allBooksOf, firstLessonPath } from "../../lib/catalog/tree";

type Book = components["schemas"]["BookDto"];

/** Marks every node the shelf inserts, so a re-run (sign-in, sign-out) can find and replace it. */
export const SHELF_ATTR = "data-private-shelf";

export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** One `<li>` per book: a link to its first lesson, or a dimmed span when it has none yet. The
 *  current book is marked active, as the server marks it. */
export function browseRowsHtml(books: Book[], currentSlug: string | null, { locked = false } = {}): string {
  const icon = locked ? LOCK_ICON : BOOK_ICON;
  return books
    .map((book) => {
      const title = `${icon}<span>${escapeHtml(book.title)}</span>`;
      const path = firstLessonPath(book);
      if (!path) return `<li><span class="reader-sidebar__book reader-sidebar__book--empty">${title}</span></li>`;
      const active = book.slug === currentSlug;
      const cls = active ? "reader-sidebar__book reader-sidebar__book--active" : "reader-sidebar__book";
      const current = active ? ' aria-current="page"' : "";
      return `<li><a class="${cls}" href="/synapse/${path}"${current}>${title}</a></li>`;
    })
    .join("");
}

/** The "Your private books" section: an eyebrow and its list, both marked with SHELF_ATTR. Empty
 *  string for no books — a reader on no list gets no heading over nothing. */
export function privateShelfHtml(books: Book[], currentSlug: string | null): string {
  if (books.length === 0) return "";
  return (
    `<div class="reader-sidebar__eyebrow reader-sidebar__eyebrow--shelf" ${SHELF_ATTR}>Your private books</div>` +
    `<ul class="reader-sidebar__browse-list" ${SHELF_ATTR}>${browseRowsHtml(books, currentSlug, { locked: true })}</ul>`
  );
}

/** The private books this index admits the reader to, alphabetical by title — a shelf scanned by
 *  name, not by where `/admin` grouped each book. */
export function privateShelfBooks(index: SynapseIndex): Book[] {
  return allBooksOf(index)
    .filter((book) => book.private === true)
    .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" }));
}
