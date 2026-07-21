// Oracle: `LibrarySearchSpec` (client/src/search/logic/logic_tests.rs) — the five
// ranking/flattening behaviors, same fixture, same assertions, case names in camelCase.

import { describe, expect, it } from "vitest";
import { entries, search } from "./search";
import type { components } from "./api/schema.gen";

type SynapseIndex = components["schemas"]["SynapseIndexDto"];
type BlogSummary = components["schemas"]["BlogSummaryDto"];

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
