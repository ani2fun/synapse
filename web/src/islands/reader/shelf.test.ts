// The private shelf's pure half: which books it lists, in what order, and the rows it draws.

import { describe, expect, it } from "vitest";
import type { components } from "../../lib/api/schema.gen";
import { browseRowsHtml, privateShelfBooks, privateShelfHtml, SHELF_ATTR } from "./shelf";

type Book = components["schemas"]["BookDto"];
type SynapseIndex = components["schemas"]["SynapseIndexDto"];

function book(slug: string, title: string, { isPrivate = false, empty = false } = {}): Book {
  return {
    slug,
    title,
    description: "",
    tags: [],
    estimatedReadingMinutes: null,
    order: null,
    categoryPath: ["learn"],
    private: isPrivate,
    entries: empty
      ? []
      : [{ kind: "lesson", slug: "intro", title: "Intro", order: null, essential: true, lessonKind: null }],
  } as Book;
}

function index(books: Book[]): SynapseIndex {
  return {
    entries: [
      {
        kind: "category",
        slug: "learn",
        title: "Learn",
        description: null,
        icon: null,
        order: null,
        entries: books.map((b) => ({ ...b, kind: "book" as const })),
      },
    ],
  };
}

describe("privateShelfBooks", () => {
  it("keepsOnlyPrivateBooksSortedByTitle", () => {
    const shelf = privateShelfBooks(
      index([
        book("french", "learn French", { isPrivate: true }),
        book("dsa", "DSA", {}),
        book("helm", "Dsa helmsman", { isPrivate: true }),
        book("algo", "Algorithms", { isPrivate: true }),
      ]),
    );
    expect(shelf.map((b) => b.slug)).toEqual(["algo", "helm", "french"]);
  });

  it("isEmptyForAnAnonymousIndex", () => {
    expect(privateShelfBooks(index([book("dsa", "DSA")]))).toEqual([]);
  });
});

describe("browseRowsHtml", () => {
  it("linksToTheFirstLessonAndMarksTheCurrentBook", () => {
    const html = browseRowsHtml([book("a", "A"), book("b", "B")], "b");
    expect(html).toContain('href="/synapse/learn/a/intro"');
    expect(html).toContain('class="reader-sidebar__book reader-sidebar__book--active" href="/synapse/learn/b/intro" aria-current="page"');
    expect(html.match(/aria-current/g)).toHaveLength(1);
  });

  it("rendersABookWithNoLessonsAsADimmedSpan", () => {
    const html = browseRowsHtml([book("a", "A", { empty: true })], null);
    expect(html).toContain('<span class="reader-sidebar__book reader-sidebar__book--empty">');
    expect(html).not.toContain("href=");
  });

  it("escapesTitles", () => {
    const html = browseRowsHtml([book("x", `<img src=x onerror="alert(1)">`)], null);
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img src=x onerror=&quot;alert(1)&quot;&gt;");
  });

  it("drawsTheLockGlyphOnlyWhenLocked", () => {
    expect(browseRowsHtml([book("a", "A")], null, { locked: true })).toContain("<rect");
    expect(browseRowsHtml([book("a", "A")], null)).not.toContain("<rect");
  });
});

describe("privateShelfHtml", () => {
  it("headsTheListAndMarksBothNodes", () => {
    const html = privateShelfHtml([book("a", "A", { isPrivate: true })], null);
    expect(html).toContain("Your private books");
    expect(html.match(new RegExp(SHELF_ATTR, "g"))).toHaveLength(2);
  });

  it("rendersNothingForNoBooks", () => {
    expect(privateShelfHtml([], null)).toBe("");
  });
});
