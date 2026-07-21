// The document head's pure half. There is no DOM-touching half needed here: Astro's
// `output: "server"` re-renders the whole document, head included, on every navigation
// (layouts/Base.astro's props), so there is no stale tab to patch client-side. Only the FORMAT
// matters, because a per-page title still has to match `platform::static_routes` wherever the
// server computes one.

/** The site name, and the fallback title for any page without one of its own. */
export const SITE_NAME = "Synapse";

/**
 * `Book · Lesson — Synapse`, matching `platform::static_routes::title_for` exactly. The book
 * leads because the left of the string is what survives truncation in a tab strip.
 */
export function titleForLesson(bookTitle: string | null, lessonTitle: string): string {
  return bookTitle ? `${bookTitle} · ${lessonTitle} — ${SITE_NAME}` : `${lessonTitle} — ${SITE_NAME}`;
}
