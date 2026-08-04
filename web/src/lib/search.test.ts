// Ranking and flattening, then the merge of the instant local match with the server's full-text
// hits — one fixture library throughout.

import { describe, expect, it } from "vitest";
import { entries, merge, search } from "./search";
import type { components } from "./api/schema.gen";

type SynapseIndex = components["schemas"]["SynapseIndexDto"];
type BlogSummary = components["schemas"]["BlogSummaryDto"];
type SearchHit = components["schemas"]["SearchHitDto"];

function lesson(slug: string, title: string): components["schemas"]["BookEntryDto"] {
  return { kind: "lesson", slug, title, essential: false };
}

function fixture(): { index: SynapseIndex; blog: BlogSummary[] } {
  const book: components["schemas"]["CatalogEntryDto"] = {
    kind: "book",
    slug: "dsa",
    title: "DSA",
    description: "",
    tags: [],
    categoryPath: ["cat"],
    entries: [
      {
        kind: "chapter",
        slug: "arrays",
        title: "Arrays",
        entries: [lesson("two-sum", "Two Sum"), lesson("binary-search", "Binary Search")],
      },
    ],
  };
  const index: SynapseIndex = {
    entries: [
      {
        kind: "category",
        slug: "cat",
        title: "Foundations",
        entries: [book],
      },
    ],
  };
  const blog: BlogSummary[] = [
    {
      slug: "hello",
      title: "Two Ferments",
      tags: [],
      publishedAt: "2026-06-01",
    },
  ];
  return { index, blog };
}

describe("search", () => {
  it("flattenYieldsLessonsBookAndBlogWithBreadcrumbs", () => {
    const { index, blog } = fixture();
    const all = entries(index, blog);
    const labels = all.map((e) => e.label);
    expect(labels).toEqual(["DSA", "Two Sum", "Binary Search", "Two Ferments"]);

    const twoSum = all.find((e) => e.label === "Two Sum");
    expect(twoSum?.kind).toBe("lesson");
    expect(twoSum?.sublabel).toBe("Foundations › DSA › Arrays");
    expect(twoSum?.page).toEqual({ kind: "lesson", path: ["cat", "dsa", "arrays", "two-sum"] });

    const bookEntry = all.find((e) => e.label === "DSA");
    expect(bookEntry?.kind).toBe("book");
    expect(bookEntry?.page).toEqual({ kind: "lesson", path: ["cat", "dsa", "arrays", "two-sum"] });
  });

  it("aWordStartMatchBeatsASubsequenceMatch", () => {
    const { index, blog } = fixture();
    const all = entries(index, blog);
    const results = search("bi", all);
    expect(results[0].label).toBe("Binary Search");
  });

  it("substringMatchesCaseInsensitivelyAcrossLessonsAndBlog", () => {
    const { index, blog } = fixture();
    const all = entries(index, blog);
    const labels = search("two", all).map((e) => e.label);
    expect(labels).toContain("Two Sum");
    expect(labels).toContain("Two Ferments");
  });

  it("noMatchIsEmptyAndEmptyQueryIsEverythingCapped", () => {
    const { index, blog } = fixture();
    const all = entries(index, blog);
    expect(search("zzzzz", all)).toHaveLength(0);
    expect(search("", all)).toHaveLength(all.length);
  });

  it("aBookTitleMatchOutranksBreadcrumbOnlyLessons", () => {
    const { index, blog } = fixture();
    const all = entries(index, blog);
    const results = search("dsa", all);
    expect(results[0].kind).toBe("book");
  });
});

/** One full-text hit as the server sends it, with a quote already split into runs. */
function hit(path: string, title: string, extra: Partial<SearchHit> = {}): SearchHit {
  return {
    title,
    breadcrumb: ["Foundations", "DSA"],
    path,
    kind: "lesson",
    bookSlug: "dsa",
    snippet: [
      { text: "an array stores its elements ", marked: false },
      { text: "contiguously", marked: true },
    ],
    ...extra,
  };
}

function library() {
  const { index, blog } = fixture();
  return entries(index, blog);
}

describe("merge", () => {
  it("aProseHitReachesALessonNoTitleMatchWouldHaveFound", () => {
    const local = library();
    // The whole reason the endpoint exists: nothing in the library is titled or filed under
    // "contiguously", so the local matcher finds nothing at all.
    expect(search("contiguously", local)).toHaveLength(0);

    const merged = merge("contiguously", [], [hit("cat/dsa/arrays/hashing", "Hashing")]);
    expect(merged.map((e) => e.label)).toEqual(["Hashing"]);
    expect(merged[0].page).toEqual({ kind: "lesson", path: ["cat", "dsa", "arrays", "hashing"] });
    expect(merged[0].snippet?.map((s) => s.marked)).toEqual([false, true]);
  });

  it("aLiteralTitleMatchLeadsAProseHit", () => {
    const local = library();
    const merged = merge("two", search("two", local), [hit("cat/dsa/arrays/hashing", "Hashing")]);
    expect(merged.map((e) => e.label)).toEqual(["Two Sum", "Two Ferments", "Hashing"]);
  });

  it("aProseHitLeadsAFuzzySubsequenceMatch", () => {
    const local = library();
    // Every local hit for "tos" is a subsequence accident — t·w·**o**·**s**um and friends. A word
    // that genuinely appears in a lesson's prose is a better answer than any of them.
    const ranked = search("tos", local);
    expect(ranked.length).toBeGreaterThan(0);

    const merged = merge("tos", ranked, [hit("cat/dsa/arrays/hashing", "Hashing")]);
    expect(merged[0].label).toBe("Hashing");
    // The fuzzy tail keeps its order behind it — minus the DSA book row, which links to Two Sum
    // and is therefore the same destination already on the list.
    expect(merged.slice(1).map((e) => e.label)).toEqual(["Two Sum", "Two Ferments", "Binary Search"]);
  });

  it("theSameLessonFoundTwiceIsOneRowThatBorrowsTheQuote", () => {
    const local = library();
    const ranked = search("two", local);
    const merged = merge("two", ranked, [hit("cat/dsa/arrays/two-sum", "Two Sum")]);

    expect(merged.filter((e) => e.label === "Two Sum")).toHaveLength(1);
    expect(merged).toHaveLength(ranked.length);
    expect(merged[0].label).toBe("Two Sum");
    expect(merged[0].snippet).toBeDefined();
  });

  it("borrowingAQuoteDoesNotWriteIntoTheCachedLibrary", () => {
    // The palette caches the flattened library for the life of the page. Attaching a snippet to
    // one of those objects would leave it on the row for every later query.
    const local = library();
    const ranked = search("two", local);
    merge("two", ranked, [hit("cat/dsa/arrays/two-sum", "Two Sum")]);

    expect(local.every((e) => e.snippet === undefined)).toBe(true);
    expect(merge("two", ranked, []).every((e) => e.snippet === undefined)).toBe(true);
  });

  it("aBookRowKeepsItsPlaceAndDoesNotBorrowItsFirstLessonsWords", () => {
    const local = library();
    const ranked = search("dsa", local);
    // The book links to its first lesson, so it shares that lesson's URL — a real collision, not
    // a contrived one.
    const merged = merge("dsa", ranked, [hit("cat/dsa/arrays/two-sum", "Two Sum")]);

    expect(merged[0].kind).toBe("book");
    expect(merged[0].snippet).toBeUndefined();
    expect(merged.filter((e) => e.page.kind === "lesson" && e.page.path.includes("two-sum"))).toHaveLength(1);
  });

  it("noProseHitsIsExactlyTheBehaviourBeforeTheEndpointExisted", () => {
    const local = library();
    const ranked = search("two", local);
    expect(merge("two", ranked, [])).toEqual(ranked);
    expect(merge("", ranked, [hit("cat/dsa/arrays/hashing", "Hashing")])).toEqual(ranked);
  });

  it("anEditorialIsItsOwnKindAndAnUnknownOneReadsAsALesson", () => {
    const solution = merge("x", [], [hit("cat/dsa/problems/two-sum", "Two Sum", { kind: "editorial" })]);
    expect(solution[0].kind).toBe("editorial");

    const future = merge("x", [], [hit("cat/dsa/arrays/hashing", "Hashing", { kind: "quiz" })]);
    expect(future[0].kind).toBe("lesson");
  });
});
