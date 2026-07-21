// Seven cases. The fixtures mirror the real authored `.practice-problem` attribute payloads.

import { describe, expect, it } from "vitest";

import { decodePractice, solutionComplexities } from "./practice";

const VARIANTS = JSON.stringify([{ lang: "python", source: "print(1)" }]);

describe("practice — the decode", () => {
  it("decodes a full practice problem", () => {
    const spec = decodePractice(
      "State the problem.",
      VARIANTS,
      JSON.stringify({ args: [{ id: "n", label: "n", type: "int" }], cases: [{ args: { n: "3" } }] }),
      JSON.stringify([{ tag: "", md: "The editorial." }]),
    );
    expect(spec).not.toBeNull();
    expect(spec!.variants.length).toBe(1);
    expect(spec!.spec).not.toBeNull();
    expect(spec!.editorials.length).toBe(1);
    expect(spec!.editorials[0]!.label).toBe("Editorial");
    expect(spec!.editorials[0]!.md).toBe("The editorial.");
  });

  it("a blank statement or empty variants reads as no widget", () => {
    expect(decodePractice("  ", VARIANTS, null, null)).toBeNull();
    expect(decodePractice("Statement", "[]", null, null)).toBeNull();
    expect(decodePractice("Statement", "not json", null, null)).toBeNull();
  });

  it("a malformed spec degrades to no tests and blank editorials drop", () => {
    const spec = decodePractice(
      "Statement",
      VARIANTS,
      "not json",
      JSON.stringify([{ tag: "approach-optimal-1", md: "   " }]),
    );
    expect(spec).not.toBeNull();
    expect(spec!.spec).toBeNull();
    expect(spec!.editorials.length).toBe(0);
  });

  it("approach tags become titled tabs in authoring order", () => {
    const spec = decodePractice(
      "Statement",
      VARIANTS,
      null,
      JSON.stringify([
        { tag: "approach-brute-force-1", md: "Try all." },
        { tag: "approach-optimal-1", md: "Two pointers." },
      ]),
    );
    expect(spec!.editorials.map((a) => a.label)).toEqual(["Brute Force", "Optimal"]);
  });

  it("a repeated kind numbers its tabs", () => {
    const spec = decodePractice(
      "Statement",
      VARIANTS,
      null,
      JSON.stringify([
        { tag: "approach-brute-force-1", md: "A." },
        { tag: "approach-brute-force-2", md: "B." },
        { tag: "approach-optimal-1", md: "C." },
      ]),
    );
    expect(spec!.editorials.map((a) => a.label)).toEqual(["Brute Force 1", "Brute Force 2", "Optimal"]);
  });
});

describe("practice — solution complexities", () => {
  it("extracts time and space claims from a solution meta", () => {
    expect(solutionComplexities("solution time=O(n) space=O(1)")).toEqual([
      ["time", "O(n)"],
      ["space", "O(1)"],
    ]);
    expect(solutionComplexities("solution")).toEqual([]);
  });

  // Real authored metas put spaces INSIDE the O-group — the value runs until its parentheses
  // balance, never to the first space.
  it("a spaced complexity value survives whole", () => {
    expect(solutionComplexities("solution time=O(log N) space=O(1)")).toEqual([
      ["time", "O(log N)"],
      ["space", "O(1)"],
    ]);
    expect(solutionComplexities("solution time=O(log(min(N1, N2))) space=O(min(N1, N2))")).toEqual([
      ["time", "O(log(min(N1, N2)))"],
      ["space", "O(min(N1, N2))"],
    ]);
    // An unbalanced value swallows the rest rather than panicking.
    expect(solutionComplexities("solution time=O(log space=O(1)")).toEqual([["time", "O(log space=O(1)"]]);
  });
});
