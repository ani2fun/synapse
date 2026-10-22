// The unit is the cache key and the engine's unit of work, so its rules are pinned here: never
// across a block, never over the server's limit, packed toward the target, and a one-sentence
// opening so the first audio is quick.
import { describe, expect, it } from "vitest";

import { hardSplit, packSentences, sentences, splitIntoUnits, TARGET_CHARS, unitSpans } from "./units";

describe("sentences", () => {
  it("splits on terminal punctuation followed by whitespace", () => {
    expect(sentences("A join pairs rows. Why? Because “it” matters! Done…")).toEqual([
      "A join pairs rows.",
      "Why?",
      "Because “it” matters!",
      "Done…",
    ]);
  });

  it("keeps a closing quote or bracket with its sentence", () => {
    expect(sentences('He said "stop." Then (quietly.) left.')).toEqual(['He said "stop."', "Then (quietly.)", "left."]);
  });
});

describe("hardSplit", () => {
  it("cuts on the last space before the limit", () => {
    expect(hardSplit("aaa bbb ccc", 7)).toEqual(["aaa bbb", "ccc"]);
  });

  it("cuts mid-word only when there is no space", () => {
    expect(hardSplit("abcdefghij", 4)).toEqual(["abcd", "efgh", "ij"]);
  });
});

describe("packSentences", () => {
  const s = (n: number, ch = "a") => `${ch.repeat(n - 1)}.`;

  it("joins whole sentences up to the target and never past it", () => {
    const packed = packSentences([s(100), s(100), s(100), s(50)], 300);
    expect(packed.map((u) => u.length)).toEqual([201, 151]);
    expect(packed.every((u) => u.length <= 300)).toBe(true);
  });

  it("gives a sentence longer than the target a unit of its own", () => {
    expect(packSentences([s(20), s(350), s(20)], 300).map((u) => u.length)).toEqual([20, 350, 20]);
  });

  it("loses no words and keeps their order", () => {
    const parts = ["One.", "Two.", "Three.", "Four."];
    expect(packSentences(parts, 12).join(" ")).toBe(parts.join(" "));
  });

  it("packs nothing into nothing", () => {
    expect(packSentences([], 300)).toEqual([]);
  });
});

describe("splitIntoUnits", () => {
  it("opens with one sentence alone, then packs", () => {
    const units = splitIntoUnits(["First. Second. Third."], 400);
    expect(units[0]).toBe("First.");
    expect(units.slice(1).join(" ")).toBe("Second. Third.");
  });

  it("never joins across blocks", () => {
    const units = splitIntoUnits(["Intro.", "Alpha. Beta.", "Gamma."], 400);
    expect(units).toContain("Gamma.");
    expect(units.some((u) => u.includes("Beta. Gamma"))).toBe(false);
  });

  it("keeps every unit within the server's limit", () => {
    const long = `${"word ".repeat(200).trim()}.`;
    const units = splitIntoUnits([long, long], 400);
    expect(units.every((u) => u.length <= 400)).toBe(true);
    expect(units.every((u) => u.length <= Math.max(TARGET_CHARS, 400))).toBe(true);
  });

  it("yields the same units for a block wherever the listen started", () => {
    const later = "Alpha one. Beta two. Gamma three.";
    const fromStart = splitIntoUnits(["Intro here.", later], 400);
    const fromEarlier = splitIntoUnits(["Other intro. With two sentences.", later], 400);
    expect(fromStart.slice(-packSentences(sentences(later), TARGET_CHARS).length)).toEqual(
      fromEarlier.slice(-packSentences(sentences(later), TARGET_CHARS).length),
    );
  });
});

describe("unitSpans", () => {
  it("points every unit back at the exact characters of its block", () => {
    const blocks = ["First one. Second one here.", "A heading.", "x ".repeat(250).trim() + "."];
    for (const span of unitSpans(blocks, 400)) {
      expect(blocks[span.block].slice(span.start, span.end)).toBe(span.text);
    }
  });

  it("walks forward, so a repeated sentence maps to its own occurrence", () => {
    const spans = unitSpans(["Intro.", "Again. Again."], 6);
    const inSecond = spans.filter((s) => s.block === 1);
    expect(inSecond.map((s) => s.start)).toEqual([0, 7]);
  });
});
