// Parity tests for tree.ts (oracle: client/src/catalog/logic/logic_tests.rs — all 18 cases,
// same fixtures, same assertions, case names ported to camelCase). A03 shipped tree.ts without
// this file; A04 pays the debt off in full, one Rust test function per `it`.

import { describe, expect, it } from "vitest";
import type { components } from "../api/schema.gen";
import {
  bookOf,
  chapterProblems,
  findBook,
  firstLessonPath,
  problemContentSplit,
  pruneEntries,
  readingOrder,
  resolveC4Node,
  spreadFractions,
  chapterCount,
  lessonCount,
  type C4PathHop,
} from "./tree";

type Book = components["schemas"]["BookDto"];
type BookEntry = components["schemas"]["BookEntryDto"];
type SynapseIndex = components["schemas"]["SynapseIndexDto"];

function lesson(slug: string): BookEntry {
  return entry(slug, null);
}

/** A `kind: problem` lesson — what `chapterProblems` counts. */
function problem(slug: string): BookEntry {
  return entry(slug, "problem");
}

function entry(slug: string, kind: string | null): BookEntry {
  return {
    kind: "lesson",
    slug,
    title: slug,
    order: null,
    essential: true,
    lessonKind: kind,
  };
}

function chapter(slug: string, entries: BookEntry[]): BookEntry {
  return { kind: "chapter", slug, title: slug, order: null, entries };
}

function book(): Book {
  return {
    slug: "dsa",
    title: "DSA",
    description: "",
    tags: [],
    estimatedReadingMinutes: null,
    order: null,
    categoryPath: ["learn"],
    entries: [lesson("intro"), chapter("lists", [lesson("singly")])],
  };
}

function index(): SynapseIndex {
  return {
    entries: [
      {
        kind: "category",
        slug: "learn",
        title: "Learn",
        description: null,
        icon: null,
        order: null,
        entries: [{ ...book(), kind: "book" }],
      },
    ],
  };
}

function hop(tag: string, classes: string, id: string | null): C4PathHop {
  return [tag, classes, id];
}

describe("tree", () => {
  it("readingOrderIsPreorderWithFullPaths", () => {
    const paths = readingOrder(book()).map(({ path }) => path);
    expect(paths).toEqual(["learn/dsa/intro", "learn/dsa/lists/singly"]);
  });

  it("firstLessonPathIsTheCoverTarget", () => {
    expect(firstLessonPath(book())).toBe("learn/dsa/intro");
  });

  it("bookOfDescendsCategoriesToTheOwningBook", () => {
    const idx = index();
    expect(bookOf(idx, ["learn", "dsa", "lists", "singly"])?.slug).toBe("dsa");
    expect(bookOf(idx, ["learn", "nope", "x"])).toBeNull();
  });

  it("findBookDescendsCategoriesByGloballyUniqueSlug", () => {
    const idx = index();
    expect(findBook(idx, "dsa")?.title).toBe("DSA");
    expect(findBook(idx, "nope")).toBeNull();
  });

  it("cardCountsLessonsRecursivelyAndChaptersDirectly", () => {
    const b = book();
    expect(lessonCount(b)).toBe(2);
    expect(chapterCount(b)).toBe(1);
  });

  it("aNodeBodyClickResolvesToItsDottedFqn", () => {
    const path: C4PathHop[] = [
      hop("DIV", "likec4-element", null),
      hop("DIV", "react-flow__node react-flow__node-element", "btPersonal.btSmallWeb"),
      hop("DIV", "react-flow__pane", null),
    ];
    expect(resolveC4Node(path)).toBe("btPersonal.btSmallWeb");
  });

  it("aButtonBeforeTheNodeIsLikec4sOwnControl", () => {
    const path: C4PathHop[] = [
      hop("BUTTON", "mantine-ActionIcon-root", null),
      hop("DIV", "react-flow__node", "sfClient"),
    ];
    expect(resolveC4Node(path)).toBeNull();
  });

  it("edgesAndTokenSubstringsNeverResolve", () => {
    const edge: C4PathHop[] = [hop("G", "react-flow__edge", "hash-1a2b")];
    expect(resolveC4Node(edge)).toBeNull();
    const substring: C4PathHop[] = [hop("DIV", "react-flow__node-toolbar", "x")];
    expect(resolveC4Node(substring)).toBeNull();
    const emptyId: C4PathHop[] = [hop("DIV", "react-flow__node", "")];
    expect(resolveC4Node(emptyId)).toBeNull();
  });

  it("pruneKeepsMatchingLessonsAndWholeMatchingChapters", () => {
    const entries = book().entries;
    const hits = pruneEntries(entries, "singly");
    expect(hits).toHaveLength(1);
    expect(hits[0].kind).toBe("chapter");
    if (hits[0].kind === "chapter") expect(hits[0].entries).toHaveLength(1);

    // A matching CHAPTER title keeps all its lessons.
    const all = pruneEntries(entries, "lists");
    expect(all[0].kind).toBe("chapter");
    if (all[0].kind === "chapter") expect(all[0].entries).toHaveLength(1);

    expect(pruneEntries(entries, "zzz")).toHaveLength(0);
    expect(pruneEntries(entries, "  ")).toHaveLength(entries.length);
  });

  it("spreadDeOverlapsAndClampsFractions", () => {
    const out = spreadFractions([0.1, 0.11, 0.12]);
    expect(out[1] - out[0]).toBeGreaterThanOrEqual(0.05 - 1e-9);
    expect(out[2] - out[1]).toBeGreaterThanOrEqual(0.05 - 1e-9);
    const edges = spreadFractions([0.0, 1.0]);
    expect(edges[0]).toBeGreaterThanOrEqual(0.05 - 1e-9);
    expect(edges[1]).toBeLessThanOrEqual(0.95 + 1e-9);
    expect(spreadFractions([])).toEqual([]);
  });

  it("problemSplitDividesAtTheFirstTopLevelDetails", () => {
    const raw =
      "The problem.\n\n```txt\n<details inside a fence>\n```\n\n<details>\n<summary>Editorial</summary>\nanswer\n</details>";
    const [desc, editorial] = problemContentSplit(raw);
    expect(desc.endsWith("```")).toBe(true);
    expect(editorial.startsWith("<details>")).toBe(true);
    const [all, none] = problemContentSplit("No editorial here.");
    expect(all).toBe("No editorial here.");
    expect(none).toBe("");
  });

  // ───────────────────────────────────────────────────────────────────────
  // CHAPTER PROBLEMS — the problem-page counter's source of truth
  // ───────────────────────────────────────────────────────────────────────

  /** Two problem chapters plus prose, so sibling bleed and prose contamination both have a
   *  chance to show up. */
  function problemBook(overrides: Partial<Book> = {}): Book {
    return {
      slug: "dsa",
      title: "DSA",
      description: "",
      tags: [],
      estimatedReadingMinutes: null,
      order: null,
      categoryPath: ["learn"],
      entries: [
        problem("root-problem"),
        chapter("basics", [
          lesson("notes"),
          problem("p1"),
          problem("p2"),
          problem("p3"),
          problem("p4"),
          problem("p5"),
          problem("p6"),
        ]),
        chapter("arrays", [problem("a1"), problem("a2"), problem("a3")]),
      ],
      ...overrides,
    };
  }

  it("countsTheProblemsOfTheCurrentChapterAndLocatesThePath", () => {
    const result = chapterProblems(problemBook(), "learn/dsa/basics/p5");
    expect(result?.problems).toHaveLength(6);
    expect(result?.at).toBe(4);
  });

  it("aProseLessonIsNeitherCountedNorLocated", () => {
    const result = chapterProblems(problemBook(), "learn/dsa/basics/p1");
    expect(result?.problems.some((p) => p.endsWith("/notes"))).toBe(false);
    expect(chapterProblems(problemBook(), "learn/dsa/basics/notes")).toBeNull();
  });

  it("aSiblingChaptersProblemsDoNotBleedIn", () => {
    const result = chapterProblems(problemBook(), "learn/dsa/arrays/a2");
    expect(result?.problems).toHaveLength(3);
    expect(result?.at).toBe(1);
  });

  /** A book that never introduced chapters still counts: the book prefix IS the parent. */
  it("bookRootProblemsCountAgainstEachOther", () => {
    const result = chapterProblems(problemBook(), "learn/dsa/root-problem");
    expect(result?.problems).toEqual(["learn/dsa/root-problem"]);
    expect(result?.at).toBe(0);
  });

  it("anUnknownPathHasNoPosition", () => {
    expect(chapterProblems(problemBook(), "learn/dsa/basics/nope")).toBeNull();
    expect(chapterProblems(problemBook(), "bare")).toBeNull();
  });

  it("aChapterWithNoProblemsYieldsNothing", () => {
    const proseOnly = problemBook({ entries: [chapter("reading", [lesson("one"), lesson("two")])] });
    expect(chapterProblems(proseOnly, "learn/dsa/reading/one")).toBeNull();
  });

  /** The shape the real content uses: `problems/<slug>/<slug>.md`, one chapter per problem.
   *  Scoping on the raw parent would read "1 / 1" on every page; the counter has to flatten
   *  these the way the sidebar already does. */
  it("problemsInAChapterOfTheirOwnStillCountTogether", () => {
    const ownDirs = problemBook({
      entries: [
        chapter("problems", [
          chapter("input-output", [problem("input-output")]),
          chapter("if-else-if", [problem("if-else-if")]),
          chapter("switch-case", [problem("switch-case")]),
        ]),
      ],
    });
    const result = chapterProblems(ownDirs, "learn/dsa/problems/if-else-if/if-else-if");
    expect(result?.problems).toHaveLength(3);
    expect(result?.at).toBe(1);
  });
});
