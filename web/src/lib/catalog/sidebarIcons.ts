// The reader sidebar's glyphs, as SVG strings. `Sidebar.astro` renders the public rail on the
// server and `islands/reader/shelf.ts` renders the private shelf (and a private lesson's whole
// rail) in the browser; both draw from here so the two halves of one list never disagree.

/** "Back to Main": the book face points away from itself. */
export const CHEVRON_ICON =
  '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m15 18-6-6 6-6" /></svg>';

/** A house on the browse face's toprow: that face IS the dashboard — every course you own — so
 *  the row names it rather than pointing somewhere, the way the book face's chevron does. */
export const HOME_ICON =
  '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m2.25 12 8.954-8.955c.44-.439 1.152-.439 1.591 0L21.75 12M4.5 9.75v10.125c0 .621.504 1.125 1.125 1.125H9.75v-4.875c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125V21h4.125c.621 0 1.125-.504 1.125-1.125V9.75M8.25 21h8.25" /></svg>';

/** A generic book glyph in front of each browse-list book — the catalog carries no per-book icon,
 *  so it stays consistent rather than topical. */
export const BOOK_ICON =
  '<svg class="reader-sidebar__book-ic" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 4.5A2.5 2.5 0 0 1 6.5 2H20v18H6.5A2.5 2.5 0 0 0 4 22.5z" /><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" /></svg>';

/** A padlock in place of the book glyph: a book only this reader (and its listed readers) sees. */
export const LOCK_ICON =
  '<svg class="reader-sidebar__book-ic" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="4.5" y="10.5" width="15" height="10.5" rx="2" /><path d="M8 10.5V7a4 4 0 0 1 8 0v3.5" /></svg>';
