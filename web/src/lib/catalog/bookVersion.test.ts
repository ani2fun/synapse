// The book's version chip: the short commit, linked to GitHub when the repository is known, and
// nothing at all when the payload carries no version.

import { describe, expect, it } from "vitest";
import { bookVersionHtml, commitUrl } from "./bookVersion";

const SHA = "b962f394a9107db4d5d811c75036907c2d25a53d";

describe("bookVersionHtml", () => {
  it("rendersNothingWithoutAVersion", () => {
    expect(bookVersionHtml(null)).toBe("");
    expect(bookVersionHtml(undefined)).toBe("");
  });

  it("linksTheShortCommitToGitHub", () => {
    const html = bookVersionHtml({ commit: SHA, repo: "ani2fun/java-guide" });
    expect(html).toMatch(/^<span class="book-version">/);
    expect(html).toMatch(/<span class="book-version__label">Book version<\/span>/);
    const link = html.match(/<a class="book-version__sha"[^>]*>([^<]*)<\/a>/);
    expect(link?.[1]).toBe("b962f39");
    expect(link?.[0]).toContain(`href="${commitUrl("ani2fun/java-guide", SHA)}"`);
    expect(link?.[0]).toContain(SHA); // the full commit, in the title
    expect(link?.[0]).toContain('rel="noopener');
  });

  it("showsThePlainCommitWhenTheRepositoryIsUnknown", () => {
    const html = bookVersionHtml({ commit: SHA });
    expect(html).not.toContain("<a");
    expect(html).toMatch(/class="book-version__sha"[^>]*>b962f39</);
  });

  it("escapesWhatItInterpolates", () => {
    const html = bookVersionHtml({ commit: SHA, repo: 'o/r"><script>x</script>' });
    expect(html).not.toContain("<script>");
    expect(html).not.toContain('r">');
  });
});

describe("commitUrl", () => {
  it("pointsAtTheCommitPage", () => {
    expect(commitUrl("ani2fun/synapse-content", SHA)).toBe(
      `https://github.com/ani2fun/synapse-content/commit/${SHA}`,
    );
  });
});
