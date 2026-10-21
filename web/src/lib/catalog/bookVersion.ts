// The book's version chip — which commit of which content repository the page on screen was read
// from. ONE renderer for the three places that draw it (the lesson page, the problem frame and
// the private lesson island), so the server-rendered and browser-rendered pages cannot disagree.
//
// The payload carries the FULL commit; shortening it is presentation and happens here. Nothing is
// rendered when the payload carries no version: a source with no commit on disk (a fixture, a
// local directory) has no honest answer to show.

import type { components } from "../api/schema.gen";

export type BookVersion = components["schemas"]["BookVersionDto"];

/** Where a commit lives on GitHub. `repo` is `owner/name`. */
export function commitUrl(repo: string, commit: string): string {
  return `https://github.com/${repo}/commit/${commit}`;
}

/** GitHub's own short form — unambiguous in a content repository's handful of commits. */
const SHORT_SHA = 7;

function esc(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * `<span class="book-version">` holding a `book-version__label` ("Book version") and the short
 * commit as `book-version__sha` — a link to the commit when the repository is known, plain text
 * when it is not — or `""` when there is no version at all. Every interpolated value is escaped.
 */
export function bookVersionHtml(version: BookVersion | null | undefined): string {
  if (!version) return "";
  const short = esc(version.commit.slice(0, SHORT_SHA));
  const full = esc(version.commit);
  const sha = version.repo
    ? `<a class="book-version__sha" href="${esc(commitUrl(version.repo, version.commit))}" ` +
      `target="_blank" rel="noopener noreferrer" title="${full}">${short}</a>`
    : `<code class="book-version__sha" title="${full}">${short}</code>`;
  return `<span class="book-version"><span class="book-version__label">Book version</span>${sha}</span>`;
}
