import { describe, expect, it } from "vitest";

import { chipLine } from "./guidance";
import type { CanvasBody } from "./model";
import {
  bestComplexity,
  blankBody,
  entryTitle,
  exportEnvelope,
  fileStem,
  filledCount,
  isBlank,
  normalizeBody,
  TOTAL_AREAS,
  toWire,
} from "./model";

function body(overrides: Partial<CanvasBody> = {}): CanvasBody {
  return { ...blankBody(), ...overrides };
}

describe("filledCount", () => {
  it("reads a brand-new canvas as empty even though the starter ideas are NAMED", () => {
    // The two starter rows ship with "Brute force" / "Optimized" already in them. Counting a
    // name would show 1/8 before the reader has typed a word, which is a meter that lies.
    expect(filledCount(blankBody())).toBe(0);
    expect(isBlank(blankBody())).toBe(true);
  });

  it("counts each non-blank area once", () => {
    expect(filledCount(body({ problem: "Two sum", constraints: "n ≤ 1e4" }))).toBe(2);
  });

  it("ignores whitespace-only areas", () => {
    expect(filledCount(body({ problem: "   \n  " }))).toBe(0);
  });

  it("counts Ideas as ONE area, and only once an idea has a description", () => {
    const ideas = blankBody().ideas;
    expect(filledCount(body({ ideas }))).toBe(0);
    ideas[0]!.desc = "sort, then walk from both ends";
    expect(filledCount(body({ ideas }))).toBe(1);
    ideas[1]!.desc = "hash map of complements";
    expect(filledCount(body({ ideas }))).toBe(1);
  });

  it("never exceeds the total the meter divides by", () => {
    const full = body({
      problem: "a", constraints: "a", maintenance: "a",
      inputs: "a", ret: "a", errors: "a", tests: "a",
    });
    full.ideas[0]!.desc = "a";
    expect(filledCount(full)).toBe(TOTAL_AREAS);
  });
});

describe("bestComplexity", () => {
  it("is a dash while no idea names a time", () => {
    expect(bestComplexity(blankBody())).toBe("—");
  });

  it("reads the LAST timed idea — the canvas refines downward", () => {
    const b = blankBody();
    b.ideas[0]!.time = "O(n²)";
    b.ideas[0]!.space = "O(1)";
    b.ideas[1]!.time = "O(n)";
    b.ideas[1]!.space = "O(n)";
    expect(bestComplexity(b)).toBe("O(n) / O(n)");
  });

  it("skips an untimed idea below a timed one", () => {
    const b = blankBody();
    b.ideas[0]!.time = "O(n²)";
    b.ideas[0]!.space = "O(1)";
    expect(bestComplexity(b)).toBe("O(n²) / O(1)");
  });

  it("marks a missing space as unknown rather than blank", () => {
    const b = blankBody();
    b.ideas[0]!.time = "O(n)";
    expect(bestComplexity(b)).toBe("O(n) / ?");
  });
});

describe("entryTitle", () => {
  it("takes the first line of Problem", () => {
    expect(entryTitle(body({ problem: "Return the two indices\nnot the values" }))).toBe(
      "Return the two indices",
    );
  });

  it("falls back when Problem is empty", () => {
    expect(entryTitle(blankBody())).toBe("Untitled canvas");
  });

  it("truncates to fit a table cell", () => {
    expect(entryTitle(body({ problem: "x".repeat(200) }))).toHaveLength(72);
  });
});

describe("normalizeBody / toWire", () => {
  it("round-trips, with `ret` travelling as `return`", () => {
    const original = body({ problem: "p", ret: "int[] of two indices" });
    const wire = toWire(original);
    expect(wire.return).toBe("int[] of two indices");
    expect("ret" in wire).toBe(false);
    const back = normalizeBody(wire);
    expect(back.ret).toBe("int[] of two indices");
    expect(back.problem).toBe("p");
  });

  it("tolerates a body missing fields — an older build's draft still opens", () => {
    const back = normalizeBody({ problem: "only this" });
    expect(back.problem).toBe("only this");
    expect(back.constraints).toBe("");
    expect(back.tests).toBe("");
  });

  it("tolerates null and undefined", () => {
    expect(normalizeBody(null).problem).toBe("");
    expect(normalizeBody(undefined).ideas).toHaveLength(2);
  });

  it("restores the starter ideas when a stored body had none", () => {
    expect(normalizeBody({ problem: "p", ideas: [] }).ideas).toHaveLength(2);
  });

  it("mints fresh ids — stored ideas carry none, and the list is keyed on them", () => {
    const ideas = normalizeBody({ ideas: [{ name: "a", description: "b", time: "", space: "" }] }).ideas;
    expect(ideas[0]!.id).toBeTruthy();
    expect(ideas[0]!.desc).toBe("b");
  });
});

describe("exportEnvelope", () => {
  it("stamps the schema version and the problem it came from", () => {
    const envelope = exportEnvelope("Two Sum", ["dsa", "arrays", "two-sum"], { draft: blankBody() });
    expect(envelope.schema).toBe("algorithm-design-canvas/v1");
    expect(envelope.problem).toBe("Two Sum");
    expect(envelope.problemPath).toBe("dsa/arrays/two-sum");
    expect(envelope.draft).toBeDefined();
    expect(envelope.entries).toBeUndefined();
  });

  it("carries entries when exporting those instead", () => {
    const envelope = exportEnvelope("Two Sum", ["two-sum"], { entries: [] });
    expect(envelope.entries).toEqual([]);
    expect(envelope.draft).toBeUndefined();
  });
});

describe("fileStem", () => {
  it("slugifies the last path segment", () => {
    expect(fileStem(["dsa", "arrays", "Move Zeroes"])).toBe("move-zeroes");
  });

  it("falls back when the path is empty", () => {
    expect(fileStem([])).toBe("canvas");
  });
});

describe("chipLine", () => {
  it("starts the first line without a leading newline", () => {
    expect(chipLine("", "max N")).toBe("· max N — ");
  });

  it("opens a new line when the buffer does not already end in one", () => {
    expect(chipLine("· sorted? — yes", "max N")).toBe("· sorted? — yes\n· max N — ");
  });

  it("does not double the newline when the buffer already ends in one", () => {
    expect(chipLine("· sorted? — yes\n", "max N")).toBe("· sorted? — yes\n· max N — ");
  });
});
