// Twenty-two cases. The fixtures mirror the REAL authored content in synapse-content/dsa: the
// flat single-approach format, the multi-approach format (`## Brute / ## Optimal …` with `###`
// subsections incl. `### Edge Case`), and the degradation shapes (arbitrary headings, plain
// fences, empty files).

import { describe, expect, it } from "vitest";

import {
  activeSection,
  complexityProse,
  firstSolutionMeta,
  parseEditorial,
  prettyO,
  sectionKind,
} from "./editorial";
import type { SectionDoc } from "./editorial";
import { sectionIndex } from "./pane";

const FLAT = `## Intuition

Iterate and test each candidate.

## Approach

1. Initialize a list.
2. Iterate from 1 to n.

## Solution

\`\`\`python solution time=O(N^2) space=O(1)
# Iterate from 1 to n
print(1)
\`\`\`

\`\`\`java solution time=O(N^2) space=O(1)
class Main {}
\`\`\`

## Complexity Analysis

**Time Complexity:** O(N^2). The overall complexity will be O(N^2), where N is the number of rows.

**Space Complexity:** O(1) – Using a couple of variables i.e., constant space.
`;

const MULTI = `## Brute

### Intuition

Check every value.

### Approach

1. Iterate from 1 to n.

### Edge Case

Watch out for 1.

### Solution

\`\`\`python solution time=O(N) space=O(K)
# a comment, not a heading
print(1)
\`\`\`

### Complexity Analysis

**Time Complexity:** O(N) – Iterating N times.

**Space Complexity:** O(K), where K is the number of divisors.

## Optimal

### Intuition

Pairs mirror around the square root.

### Approach

1. Iterate to sqrt(n).

### Solution

\`\`\`python solution time=O(sqrt(N)+K*log(K)) space=O(sqrt(N))
print(2)
\`\`\`

### Complexity Analysis

**Time Complexity:** O(sqrt(N)) + O(K*log(K)) – Gather then sort.

**Space Complexity:** O(sqrt(N)) – At most 2*sqrt(N) divisors.
`;

const labels = (sections: SectionDoc[]): string[] => sections.map((s) => s.label);

// ─────────────────────────────────────────────────────────────────────────────
// PARSE — the two authored formats
// ─────────────────────────────────────────────────────────────────────────────

describe("editorial — the two authored formats", () => {
  it("the flat format is one approach with canonical sections", () => {
    const doc = parseEditorial(FLAT);
    expect(doc.multi).toBe(false);
    expect(doc.preamble).toBe("");
    expect(doc.approaches.length).toBe(1);
    const approach = doc.approaches[0]!;
    expect(approach.label).toBe("");
    expect(labels(approach.sections)).toEqual(["Intuition", "Approach", "Solution", "Complexity Analysis"]);
    expect(approach.sections.map((s) => s.kind)).toEqual(["Intuition", "Approach", "Solution", "Complexity"]);
    // Bodies carry no heading line, and the fence bodies stay intact.
    expect(approach.sections[0]!.md.startsWith("Iterate and test")).toBe(true);
    expect(approach.sections[2]!.md.includes("```python solution")).toBe(true);
    expect(approach.time).toBe("O(N^2)");
    expect(approach.space).toBe("O(1)");
  });

  it("the multi format is detected and split per approach", () => {
    const doc = parseEditorial(MULTI);
    expect(doc.multi).toBe(true);
    expect(doc.approaches.length).toBe(2);
    expect(doc.approaches[0]!.label).toBe("Brute");
    expect(doc.approaches[1]!.label).toBe("Optimal");
    expect(labels(doc.approaches[0]!.sections)).toEqual([
      "Intuition",
      "Approach",
      "Edge Case",
      "Solution",
      "Complexity Analysis",
    ]);
    expect(doc.approaches[0]!.sections[2]!.kind).toBe("Other");
    expect(doc.approaches[0]!.time).toBe("O(N)");
    expect(doc.approaches[0]!.space).toBe("O(K)");
    expect(doc.approaches[1]!.time).toBe("O(sqrt(N)+K*log(K))");
    expect(doc.approaches[1]!.space).toBe("O(sqrt(N))");
  });

  it("prose before the first heading is the preamble", () => {
    let doc = parseEditorial(`A word up front.\n\n${FLAT}`);
    expect(doc.preamble).toBe("A word up front.");
    // In the multi format, prose under an approach heading BEFORE its first `###` becomes an
    // unlabeled leading section of that approach.
    doc = parseEditorial(MULTI.replace("## Brute\n\n### Intuition", "## Brute\n\nA framing line.\n\n### Intuition"));
    expect(doc.approaches[0]!.sections[0]!.label).toBe("");
    expect(doc.approaches[0]!.sections[0]!.md).toBe("A framing line.");
    expect(doc.approaches[0]!.sections[1]!.label).toBe("Intuition");
  });

  it("two sections without canonical subheadings stay single", () => {
    // Arbitrary legacy headings: sectioned, but no stepper.
    const doc = parseEditorial("## Walkthrough\n\nProse.\n\n## Proof\n\nMore prose.");
    expect(doc.multi).toBe(false);
    expect(doc.approaches.length).toBe(1);
    expect(labels(doc.approaches[0]!.sections)).toEqual(["Walkthrough", "Proof"]);
    expect(doc.approaches[0]!.sections.every((s) => s.kind === "Other")).toBe(true);
  });

  it("headings inside code fences are content", () => {
    const md =
      "## Solution\n\n```python solution time=O(N) space=O(1)\n## not a heading\n### also not\nprint(1)\n```\n";
    const doc = parseEditorial(md);
    expect(doc.approaches[0]!.sections.length).toBe(1);
    expect(doc.approaches[0]!.sections[0]!.md.includes("## not a heading")).toBe(true);
  });

  it("a single h1 document falls back to h1 sections", () => {
    const doc = parseEditorial("# Idea\n\nProse.\n\n# Code\n\nMore.");
    expect(labels(doc.approaches[0]!.sections)).toEqual(["Idea", "Code"]);
  });

  it("a headingless or empty document degrades", () => {
    const doc = parseEditorial("Just prose, nothing else.");
    expect(doc.multi).toBe(false);
    expect(doc.preamble).toBe("Just prose, nothing else.");
    expect(doc.approaches.length).toBe(1);
    expect(doc.approaches[0]!.sections.length).toBe(0);

    const empty = parseEditorial("   \n  ");
    expect(empty.approaches.length).toBe(0);
    expect(empty.preamble).toBe("");
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// SPOILER WRAPPER — the inline editorial arrives wearing `<details>`
// ─────────────────────────────────────────────────────────────────────────────

describe("editorial — the spoiler wrapper", () => {
  it("the outer spoiler wrapper comes off", () => {
    const wrapped = `<details>\n<summary>Editorial</summary>\n\n${FLAT}\n</details>`;
    const doc = parseEditorial(wrapped);
    expect(doc.approaches[0]!.sections.length).toBe(4);
    expect(doc.preamble.includes("<summary")).toBe(false);
    for (const section of doc.approaches[0]!.sections) {
      expect(section.md.includes("</details>")).toBe(false);
    }
  });

  it("a nested details inside a section survives", () => {
    const wrapped =
      "<details>\n<summary>Editorial</summary>\n\n## Intuition\n\n<details>\n<summary>Hint</summary>\nA hint.\n</details>\n\n## Approach\n\nSteps.\n</details>";
    const doc = parseEditorial(wrapped);
    const intuition = doc.approaches[0]!.sections[0]!;
    expect(intuition.md.includes("<summary>Hint</summary>")).toBe(true);
    expect(intuition.md.includes("<details>")).toBe(true);
    // The nested closer stays; only the OUTER closer (the last one) was removed.
    expect(intuition.md.includes("</details>")).toBe(true);
    expect(doc.approaches[0]!.sections[1]!.md.includes("</details>")).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// SOLUTION SYNTHESIS — fences without a Solution heading
// ─────────────────────────────────────────────────────────────────────────────

describe("editorial — solution synthesis", () => {
  it("fences without a solution heading synthesize one", () => {
    const md =
      "## Brute\n\n### Intuition\n\nIdea.\n\n### Approach\n\n1. Steps.\n\n```python solution time=O(N) space=O(1)\nprint(1)\n```\n\n### Complexity Analysis\n\n**Time Complexity:** O(N) – Linear.\n\n## Optimal\n\n### Intuition\n\nBetter idea.\n\n### Approach\n\n```python solution time=O(1) space=O(1)\nprint(2)\n```\n";
    const doc = parseEditorial(md);
    expect(doc.multi).toBe(true);
    expect(labels(doc.approaches[0]!.sections)).toEqual(["Intuition", "Approach", "Solution", "Complexity Analysis"]);
    expect(doc.approaches[0]!.sections[1]!.md).toBe("1. Steps.");
    expect(doc.approaches[0]!.sections[2]!.md.startsWith("```python solution")).toBe(true);
    // The second approach's fence was the WHOLE Approach body — the section becomes the Solution
    // outright rather than leaving an empty husk behind.
    expect(labels(doc.approaches[1]!.sections)).toEqual(["Intuition", "Solution"]);
  });

  it("an explicit solution heading suppresses synthesis", () => {
    const doc = parseEditorial(FLAT);
    const solutions = doc.approaches[0]!.sections.filter((s) => s.kind === "Solution").length;
    expect(solutions).toBe(1);
  });

  it("plain fences trigger neither synthesis nor claims", () => {
    const md = "## Solution steps\n\n```python\nprint(1)\n```\n";
    const doc = parseEditorial(md);
    expect(labels(doc.approaches[0]!.sections)).toEqual(["Solution steps"]);
    expect(doc.approaches[0]!.time).toBeNull();
    expect(doc.approaches[0]!.space).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// KINDS AND CLAIMS
// ─────────────────────────────────────────────────────────────────────────────

describe("editorial — kinds and claims", () => {
  it("section kinds match however they were typed", () => {
    expect(sectionKind("Intuition")).toBe("Intuition");
    expect(sectionKind("  APPROACH ")).toBe("Approach");
    expect(sectionKind("Solution")).toBe("Solution");
    expect(sectionKind("Solutions")).toBe("Solution");
    expect(sectionKind("Code")).toBe("Solution");
    expect(sectionKind("Complexity")).toBe("Complexity");
    expect(sectionKind("Complexity  Analysis")).toBe("Complexity");
    expect(sectionKind("Edge Case")).toBe("Other");
    expect(sectionKind("")).toBe("Other");
  });

  it("the first solution meta wins and spaced values survive", () => {
    const md =
      "prose\n\n```python solution time=O(log(min(N1, N2))) space=O(1)\nprint(1)\n```\n\n```java solution time=O(N) space=O(N)\nx\n```";
    expect(firstSolutionMeta(md)).toBe("python solution time=O(log(min(N1, N2))) space=O(1)");
    const doc = parseEditorial(`## Solution\n\n${md}`);
    expect(doc.approaches[0]!.time).toBe("O(log(min(N1, N2)))");
    expect(firstSolutionMeta("```python\nprint(1)\n```")).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// COMPLEXITY PROSE — every separator the real content uses
// ─────────────────────────────────────────────────────────────────────────────

describe("editorial — complexity prose", () => {
  it("reads the dash form", () => {
    const parsed = complexityProse(
      "**Time Complexity:** O(N) – Iterating N times.\n\n**Space Complexity:** O(1) – Constant space.",
    )!;
    expect(parsed.time).toEqual(["O(N)", "Iterating N times."]);
    expect(parsed.space).toEqual(["O(1)", "Constant space."]);
  });

  it("reads period and comma forms", () => {
    // Both shapes verified in the real content.
    const parsed = complexityProse(
      "**Time Complexity:** O(N^2). The overall complexity will be O(N^2).\n\n**Space Complexity:** O(K), where K is the number of divisors.",
    )!;
    expect(parsed.time).toEqual(["O(N^2)", "The overall complexity will be O(N^2)."]);
    expect(parsed.space).toEqual(["O(K)", "where K is the number of divisors."]);
  });

  it("a compound value runs to the real separator", () => {
    let parsed = complexityProse("**Time Complexity:** O(sqrt(N)) + O(K*Log(K)) – Gather then sort.")!;
    expect(parsed.time).toEqual(["O(sqrt(N)) + O(K*Log(K))", "Gather then sort."]);
    // Separator characters INSIDE parentheses never split the value.
    parsed = complexityProse("**Time Complexity:** O(min(N1, N2)) – Bounded by the smaller.")!;
    expect(parsed.time).toEqual(["O(min(N1, N2))", "Bounded by the smaller."]);
  });

  it("a wrapped paragraph joins before parsing", () => {
    const parsed = complexityProse("**Time Complexity:** O(N)\n– Split across\nlines.")!;
    expect(parsed.time).toEqual(["O(N)", "Split across lines."]);
  });

  it("missing axes and garbage degrade", () => {
    const onlyTime = complexityProse("**Time Complexity:** O(N) – Linear.")!;
    expect(onlyTime.space).toBeNull();
    expect(complexityProse("No markers here at all.")).toBeNull();
    // A marker without any O-group is not card material.
    expect(complexityProse("**Time Complexity:** basically fast")).toBeNull();
    // Markers inside fences are code, not claims.
    expect(complexityProse("```\n**Time Complexity:** O(N) – nope\n```")).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// DISPLAY HELPERS
// ─────────────────────────────────────────────────────────────────────────────

describe("editorial — display helpers", () => {
  it("prettyO matches the design rendering", () => {
    expect(prettyO("O(sqrt(N))")).toBe("O(√N)");
    expect(prettyO("O(sqrt(N)+K*log(K))")).toBe("O(√N+K·log(K))");
    expect(prettyO("O(N^2)")).toBe("O(N²)");
    expect(prettyO("O(N^1.5)")).toBe("O(N^1.5)");
    expect(prettyO("O(log N)")).toBe("O(log N)");
    expect(prettyO("O(N)")).toBe("O(N)");
    // An unclosed sqrt( is passed through rather than mangled.
    expect(prettyO("O(sqrt(N")).toBe("O(sqrt(N");
  });

  it("the scroll spy picks the last section past the threshold", () => {
    expect(activeSection([], 84.0)).toBe(0);
    expect(activeSection([200.0, 400.0], 84.0)).toBe(0);
    expect(activeSection([-300.0, 20.0, 400.0], 84.0)).toBe(1);
    expect(activeSection([-300.0, -20.0, 60.0], 84.0)).toBe(2);
  });

  it("approach restore reuses the section matcher", () => {
    const approachLabels = ["Brute", "Optimal"];
    expect(sectionIndex(approachLabels, "optimal")).toBe(1);
    expect(sectionIndex(approachLabels, "gone")).toBe(0);
  });
});
