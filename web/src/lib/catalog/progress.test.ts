// Parity tests for progress.ts (oracle: client/src/catalog/logic/progress.rs — all 10 cases,
// same fixtures, same assertions, case names ported to camelCase).

import { describe, expect, it } from "vitest";
import type { components } from "../api/schema.gen";
import { completedCount, isAtEnd, nextUnread, parse, serialize } from "./progress";

type Book = components["schemas"]["BookDto"];
type BookEntry = components["schemas"]["BookEntryDto"];

function lesson(slug: string): BookEntry {
  return { kind: "lesson", slug, title: slug, order: null, essential: true, lessonKind: null };
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
    entries: [lesson("intro"), lesson("arrays"), lesson("lists")],
  };
}

function set(paths: string[]): Set<string> {
  return new Set(paths);
}

describe("progress", () => {
  it("absentOrBlankStorageIsAnEmptySet", () => {
    expect(parse(null).size).toBe(0);
    expect(parse("").size).toBe(0);
    expect(parse("\n\n  \n").size).toBe(0);
  });

  it("aRoundTripPreservesTheSet", () => {
    const done = set(["learn/dsa/arrays", "learn/dsa/intro"]);
    expect(parse(serialize(done))).toEqual(done);
  });

  it("theSerialisedFormIsStableAcrossInsertionOrders", () => {
    const a = set(["b", "a", "c"]);
    const b = set(["c", "b", "a"]);
    expect(serialize(a)).toBe(serialize(b));
  });

  it("aStrayLineCostsOnlyItself", () => {
    // The whole reason this is a list and not a positional record: garbage in the middle does
    // not take the rest of the value down with it (cf. the oracle's prefs.rs).
    const done = parse("learn/dsa/intro\n\n   \nlearn/dsa/arrays\n");
    expect(done.size).toBe(2);
    expect(done.has("learn/dsa/intro")).toBe(true);
    expect(done.has("learn/dsa/arrays")).toBe(true);
  });

  it("countingUsesFullPathsNotSlugs", () => {
    const done = set(["learn/dsa/intro", "learn/dsa/arrays"]);
    expect(completedCount(book(), done)).toBe(2);
    // A bare slug must NOT count — two books can both have an `intro`.
    expect(completedCount(book(), set(["intro"]))).toBe(0);
  });

  it("aPathFromAnotherBookDoesNotInflateTheCount", () => {
    expect(completedCount(book(), set(["learn/python/intro"]))).toBe(0);
  });

  it("aLessonShorterThanTheViewportCountsAsRead", () => {
    // The trap: `scroll / track` pins at 0 when there is nothing to scroll, so a naive threshold
    // means a short lesson can never be finished.
    expect(isAtEnd(0, 0)).toBe(true);
    expect(isAtEnd(0, -120)).toBe(true);
  });

  it("theEndIsJustShortOfTheBottom", () => {
    const track = 1000;
    expect(isAtEnd(0, track)).toBe(false);
    expect(isAtEnd(970, track)).toBe(false);
    expect(isAtEnd(980, track)).toBe(true);
    expect(isAtEnd(1000, track)).toBe(true);
    expect(isAtEnd(1200, track)).toBe(true);
  });

  it("aPageThatHasNotLaidOutIsNotFinished", () => {
    expect(isAtEnd(Number.NaN, 1000)).toBe(false);
    expect(isAtEnd(1, Number.POSITIVE_INFINITY)).toBe(false);
  });

  it("nextUnreadWalksReadingOrderAndEndsAtNone", () => {
    const b = book();
    expect(nextUnread(b, set([]))).toBe("learn/dsa/intro");
    expect(nextUnread(b, set(["learn/dsa/intro"]))).toBe("learn/dsa/arrays");
    // Out-of-order reading resumes at the gap, which is the point of using the set.
    expect(nextUnread(b, set(["learn/dsa/arrays"]))).toBe("learn/dsa/intro");
    const all = set(["learn/dsa/intro", "learn/dsa/arrays", "learn/dsa/lists"]);
    expect(nextUnread(b, all)).toBeNull();
  });
});
