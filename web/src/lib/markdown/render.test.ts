// Pipeline spec for the markdown reader. Pure node — no DOM. Asserts the current scope: the GFM
// core renders as safe HTML, fenced code is shiki-highlighted, and a run-fence group (one or
// more adjacent ```lang run fences, + an optional ```testcases JSON fence) becomes ONE
// `workbench` placeholder the client mounts an editor into. A ```mermaid fence becomes a
// `.mermaid-block` diagram placeholder the client renders as SVG, and ```viz widget fences become
// their own source-carrying placeholders (ADR-S015). ```d2 has its own spec — render.d2.test.ts —
// because it is the one family whose output depends on where the pipeline runs.
import { describe, expect, it } from "vitest";

import { renderLesson } from "./render";
import { decodeAttr } from "./testkit";

function workbenchVariants(html: string): { lang: string; source: string; viz?: string }[] {
  const raw = decodeAttr(html, "data-variants");
  return raw === undefined ? [] : JSON.parse(raw);
}

function workbenchSpec(html: string): unknown {
  const raw = decodeAttr(html, "data-spec");
  return raw === undefined ? undefined : JSON.parse(raw);
}

describe("prose core (GFM → HTML)", () => {
  it("renders headings with slug ids", async () => {
    const html = await renderLesson("# Hello World\n\n## Sub Section");
    expect(html).toMatch(/<h1[^>]*id="hello-world"/);
    expect(html).toMatch(/<h2[^>]*id="sub-section"/);
  });

  it("renders unordered + ordered lists", async () => {
    const html = await renderLesson("- a\n- b\n\n1. one\n2. two");
    expect(html).toContain("<ul>");
    expect(html).toContain("<ol>");
    expect(html).toContain("<li>a</li>");
  });

  it("renders GFM tables", async () => {
    const html = await renderLesson(["| A | B |", "| - | - |", "| 1 | 2 |"].join("\n"));
    expect(html).toContain("<table>");
    expect(html).toMatch(/<th[^>]*>A<\/th>/);
    expect(html).toMatch(/<td[^>]*>1<\/td>/);
  });

  it("renders links, GFM strikethrough, and autolinks", async () => {
    const html = await renderLesson("See [docs](https://example.com).\n\n~~old~~ https://a.com");
    expect(html).toContain('href="https://example.com"');
    expect(html).toContain(">docs</a>");
    expect(html).toContain("<del>old</del>");
    expect(html).toContain('href="https://a.com"');
  });

  it("renders blockquotes", async () => {
    const html = await renderLesson("> a quoted note");
    expect(html).toContain("<blockquote>");
    expect(html).toContain("a quoted note");
  });

  it("keeps inline code a plain chip (shiki bypasses inline nodes)", async () => {
    const html = await renderLesson("the `x = 1` value");
    expect(html).toContain("<code>x = 1</code>");
    expect(html).not.toMatch(/<code[^>]*data-language[^>]*>x = 1/);
  });
});

describe("fenced code (shiki highlighting)", () => {
  it("highlights a python fence with css-variable token colors", async () => {
    const html = await renderLesson("```python\nprint('hi')\n```");
    expect(html).toContain('data-language="python"');
    expect(html).toContain("--shiki-"); // createCssVariablesTheme emits var(--shiki-*) spans
  });

  it("renders an un-tagged fence as plaintext without throwing", async () => {
    const html = await renderLesson("```\nplain text\n```");
    expect(html).toContain("<pre");
    expect(html).toContain("plain text");
  });
});

// Every display-language fence is wrapped in ONE `.fence-group` card the client mounts a
// header bar into — language TABS when adjacent fences offer the same idea in another
// language, a lone ▶ pill otherwise. Unlike every other grouper, the fences keep their
// rendered output: the panes are still shiki figures, nested inside the wrapper.
function countOf(html: string, needle: RegExp): number {
  return (html.match(needle) ?? []).length;
}

const GROUPS = /class="fence-group"/g;
const FIGURES = /data-rehype-pretty-code-figure/g;

describe("plain fences → tab-group cards", () => {
  it("groups ADJACENT fences in different languages into ONE card, in order", async () => {
    const html = await renderLesson("```java\nint x = 1;\n```\n\n```python\nx = 1\n```");
    expect(countOf(html, GROUPS)).toBe(1);
    expect(html).toContain('data-langs="java,python"');
    expect(countOf(html, FIGURES)).toBe(2); // both panes survive as real shiki figures
    expect(html).toContain('data-language="java"');
    expect(html).toContain('data-language="python"');
    expect(html).toContain("--shiki-"); // …and both are still highlighted
  });

  it("emits the header-bar host FIRST and empty (children render in DOM order — the bar must precede the panes)", async () => {
    const html = await renderLesson("```java\nint x = 1;\n```");
    expect(html).toContain('<div class="fence-group" data-langs="java"><div class="fence-group__bar"></div><figure');
  });

  it("wraps a LONE tagged fence in the same card with one pane (the ▶ pill case)", async () => {
    const html = await renderLesson("Prose.\n\n```java\nint x = 1;\n```\n\nMore prose.");
    expect(countOf(html, GROUPS)).toBe(1);
    expect(html).toContain('data-langs="java"');
    expect(countOf(html, FIGURES)).toBe(1);
  });

  it("a paragraph between two fences breaks the group into two cards", async () => {
    const html = await renderLesson("```java\nint x = 1;\n```\n\nBetween.\n\n```python\nx = 1\n```");
    expect(countOf(html, GROUPS)).toBe(2);
    expect(html).toContain('data-langs="java"');
    expect(html).toContain('data-langs="python"');
  });

  it("a REPEATED language breaks the run — two fences can't both be the Java tab", async () => {
    const html = await renderLesson("```java\nint x = 1;\n```\n\n```java\nint y = 2;\n```");
    expect(countOf(html, GROUPS)).toBe(2);
    expect(html).not.toContain('data-langs="java,java"');
  });

  it("leaves an UNTAGGED fence alone — no card, still plain highlighted code", async () => {
    const html = await renderLesson("```\nHello, world!\n```");
    expect(html).not.toContain("fence-group");
    expect(html).toContain("<pre");
    expect(html).toContain("Hello, world!");
  });

  it("does not let a bare fence join the card of the tagged fence above it", async () => {
    // The corpus shape this guards: a `run` block's program output printed directly below it.
    const html = await renderLesson("```python\nprint(1)\n```\n\n```\n1\n```");
    expect(countOf(html, GROUPS)).toBe(1);
    expect(html).toContain('data-langs="python"');
  });

  it("a ```lang run fence stays a workbench and never joins a card", async () => {
    const html = await renderLesson("```python run\nprint(1)\n```\n\n```java\nint x = 1;\n```");
    expect(html).toContain('class="workbench"');
    expect(countOf(html, GROUPS)).toBe(1);
    expect(html).toContain('data-langs="java"');
    expect(workbenchVariants(html).map((v) => v.lang)).toEqual(["python"]);
  });

  it("a following ```mermaid fence keeps its own placeholder and breaks the run", async () => {
    const html = await renderLesson("```java\nint x = 1;\n```\n\n```mermaid\nflowchart LR\n  A --> B\n```");
    expect(html).toContain('class="mermaid-block"');
    expect(countOf(html, GROUPS)).toBe(1);
    expect(html).toContain('data-langs="java"');
  });

  it("reserved vocabularies stay OUTSIDE the card — an orphan ```testcases is bare code", async () => {
    const html = await renderLesson("```testcases\n{ nonsense }\n```");
    expect(html).not.toContain("fence-group");
    expect(html).toContain("<pre");
  });

  it("a ```viz fence with no widget= attribute is reserved too, so it stays bare", async () => {
    const html = await renderLesson("```viz\n{}\n```");
    expect(html).not.toContain("fence-group");
    expect(html).toContain("<pre");
  });
});

// A run-fence group is discovered into ONE interactive workbench placeholder
// (steps 11 · 24). The other reserved fences stay plain highlighted code until
// their own discovery hooks land (Phases 4–5).
const SPEC_JSON = `{
  "args": [{"id": "arr", "label": "arr", "type": "int[]"}],
  "cases": [{"args": {"arr": "[1, 2, 3]"}, "expected": "[3, 2, 1]"}]
}`;

describe("workbench fences → adaptive placeholder (steps 11 · 24)", () => {
  it("a single ```lang run fence becomes an empty workbench div with one variant", async () => {
    const html = await renderLesson("```python run\nprint('x')\n```");
    expect(html).toContain('class="workbench"');
    expect(workbenchVariants(html)).toEqual([{ lang: "python", source: "print('x')" }]);
    expect(workbenchSpec(html)).toBeUndefined();
    // it's a placeholder, NOT a highlighted <pre> for that block
    expect(html).not.toContain('data-language="python"');
  });

  it("captures a `viz=<structure>` hint from the fence meta onto the variant", async () => {
    const html = await renderLesson("```python run viz=array\nprint(sum([1, 2]))\n```");
    expect(workbenchVariants(html)).toEqual([{ lang: "python", source: "print(sum([1, 2]))", viz: "array" }]);
  });

  it("captures a `viz=<structure>:<root>` hint verbatim (root travels with the structure)", async () => {
    const html = await renderLesson("```python run viz=list:self.head\nx = 1\n```");
    expect(workbenchVariants(html)[0].viz).toBe("list:self.head");
  });

  it("a run fence with no viz meta carries no viz field", async () => {
    const html = await renderLesson("```python run\nprint('x')\n```");
    expect(workbenchVariants(html)[0].viz).toBeUndefined();
  });

  it("in an adjacent group, each variant carries its own viz (or none)", async () => {
    const md = ["```python run viz=array:nums", "print(1)", "```", "", "```java run", "class Solution {}", "```"].join(
      "\n",
    );
    const html = await renderLesson(md);
    expect(workbenchVariants(html)).toEqual([
      { lang: "python", source: "print(1)", viz: "array:nums" },
      { lang: "java", source: "class Solution {}" },
    ]);
  });

  it("does NOT treat a fence whose language merely starts with 'run' as runnable", async () => {
    // no bare `run` token in the meta → still a normal highlighted block
    const html = await renderLesson("```runtime-notes\nnot code to run\n```");
    expect(html).not.toContain("workbench");
    expect(html).toContain("<pre");
  });

  it("merges ADJACENT run fences into one workbench with ordered variants", async () => {
    const md = ["```python run", "print(1)", "```", "", "```java run", "class Solution {}", "```"].join("\n");
    const html = await renderLesson(md);
    expect(html.match(/class="workbench"/g)).toHaveLength(1);
    expect(workbenchVariants(html)).toEqual([
      { lang: "python", source: "print(1)" },
      { lang: "java", source: "class Solution {}" },
    ]);
  });

  it("a prose paragraph (or any other block) between run fences breaks the group", async () => {
    const md = ["```python run", "print(1)", "```", "", "Now in Java:", "", "```java run", "x", "```"].join("\n");
    const html = await renderLesson(md);
    expect(html.match(/class="workbench"/g)).toHaveLength(2);
  });

  it("parses a ```testcases fence after the group into data-spec and splices it out", async () => {
    const md = ["```python run", "print(1)", "```", "", "```testcases", SPEC_JSON, "```"].join("\n");
    const html = await renderLesson(md);
    expect(workbenchSpec(html)).toEqual({
      args: [{ id: "arr", label: "arr", type: "int[]" }],
      cases: [{ args: { arr: "[1, 2, 3]" }, expected: "[3, 2, 1]" }],
    });
    // spliced: the whole doc renders as just the placeholder — no <pre> for the testcases fence
    expect(html).not.toContain("<pre");
    expect(html).not.toContain("workbench-error");
  });

  it("an INVALID testcases fence earns a visible error card and stays rendered as code", async () => {
    const md = ["```python run", "print(1)", "```", "", "```testcases", "{ not json", "```"].join("\n");
    const html = await renderLesson(md);
    expect(html).toContain('class="workbench"'); // the block itself survives, spec-less
    expect(workbenchSpec(html)).toBeUndefined();
    expect(html).toContain("workbench-error");
    expect(html).toContain("Test cases ignored");
    expect(html).toContain("<pre"); // the raw fence still renders below the card
  });

  it("rejects a structurally-wrong spec (empty cases) with the error card", async () => {
    const md = ["```python run", "print(1)", "```", "", "```testcases", '{"args": [], "cases": []}', "```"].join("\n");
    const html = await renderLesson(md);
    expect(workbenchSpec(html)).toBeUndefined();
    expect(html).toContain("workbench-error");
  });
});

describe("solution fences → spoiler-safe placeholder", () => {
  it("a ```lang solution fence becomes an empty solution-block div with variants + metas", async () => {
    const html = await renderLesson("```python solution time=O(n) space=O(1)\ndef f(): pass\n```");
    expect(html).toContain('class="solution-block"');
    expect(workbenchVariants(html)).toEqual([{ lang: "python", source: "def f(): pass" }]);
    const metas = JSON.parse(decodeAttr(html, "data-metas")!);
    expect(metas).toEqual(["solution time=O(n) space=O(1)"]);
    // spoiler-safe: the code is NOT rendered as a highlighted <pre>
    expect(html).not.toContain('data-language="python"');
  });

  it("merges ADJACENT solution fences (python + java) into one block, metas index-aligned", async () => {
    const md = [
      "```python solution time=O(m) space=O(m)",
      "def f(): pass",
      "```",
      "",
      "```java solution",
      "class S {}",
      "```",
    ].join("\n");
    const html = await renderLesson(md);
    expect(html.match(/class="solution-block"/g)).toHaveLength(1);
    expect(workbenchVariants(html)).toEqual([
      { lang: "python", source: "def f(): pass" },
      { lang: "java", source: "class S {}" },
    ]);
    expect(JSON.parse(decodeAttr(html, "data-metas")!)).toEqual(["solution time=O(m) space=O(m)", "solution"]);
  });

  it("a run group does NOT swallow a following solution fence (separate blocks)", async () => {
    const md = ["```python run", "print(1)", "```", "", "```python solution", "answer", "```"].join("\n");
    const html = await renderLesson(md);
    expect(html.match(/class="workbench"/g)).toHaveLength(1);
    expect(html.match(/class="solution-block"/g)).toHaveLength(1);
  });
});

describe("practice-problem fences → one grouped placeholder", () => {
  const SPEC = JSON.stringify({
    args: [{ id: "n", label: "n", type: "int" }],
    cases: [{ args: { n: "3" } }],
  });
  const page = [
    "````problem",
    "Reverse the array **in place**.",
    "````",
    "```python run",
    "def reverse(a): ...",
    "```",
    "```java run",
    "class Main {}",
    "```",
    "```testcases",
    SPEC,
    "```",
  ].join("\n");

  it("groups problem + run variants + testcases into one .practice-problem placeholder", async () => {
    const html = await renderLesson(page);
    expect(html).toContain('class="practice-problem"');
    expect(decodeAttr(html, "data-problem")).toContain("Reverse the array");
    const variants = JSON.parse(decodeAttr(html, "data-variants")!) as { lang: string }[];
    expect(variants.map((v) => v.lang)).toEqual(["python", "java"]);
    expect(JSON.parse(decodeAttr(html, "data-spec")!)).toEqual(JSON.parse(SPEC));
    // the consumed fences do not ALSO render as workbench/highlighted blocks
    expect(html).not.toContain('class="workbench"');
  });

  it("a bare ```editorial fence lands as one untagged entry in data-editorials", async () => {
    const html = await renderLesson([page, "```editorial", "Use two pointers.", "```"].join("\n"));
    const editorials = JSON.parse(decodeAttr(html, "data-editorials")!) as { tag: string; md: string }[];
    expect(editorials).toEqual([{ tag: "", md: "Use two pointers." }]);
  });

  it("adjacent approach-tagged editorial fences become tagged entries, in order", async () => {
    const tagged = [
      page,
      "```editorial approach-brute-force-1",
      "Try every rotation.",
      "```",
      "```editorial approach-optimal-1",
      "Two pointers, O(n).",
      "```",
    ].join("\n");
    const html = await renderLesson(tagged);
    const editorials = JSON.parse(decodeAttr(html, "data-editorials")!) as { tag: string; md: string }[];
    expect(editorials).toEqual([
      { tag: "approach-brute-force-1", md: "Try every rotation." },
      { tag: "approach-optimal-1", md: "Two pointers, O(n)." },
    ]);
  });

  it("without a ```problem fence the same group stays a plain workbench (backward compatible)", async () => {
    const html = await renderLesson(["```python run", "print(1)", "```"].join("\n"));
    expect(html).toContain('class="workbench"');
    expect(html).not.toContain("practice-problem");
  });
});

describe("other reserved fences stay plain code (hooks reserved, not built)", () => {
  it("an ORPHAN ```testcases fence (no run group before it) stays a plain code block", async () => {
    const html = await renderLesson("```testcases\n" + SPEC_JSON + "\n```");
    expect(html).toContain("<pre");
    expect(html).not.toContain("data-variants");
    expect(html).not.toContain("workbench-error");
  });

});

describe("quiz fences → interactive card placeholder", () => {
  const QUIZ = '{"prompt": "Which graph is NOT two-colourable?", "options": ["A 4-cycle", "A triangle"], "answer": "A triangle"}';

  it("a valid ```quiz fence becomes an empty quiz-block div carrying the card JSON", async () => {
    const html = await renderLesson("```quiz\n" + QUIZ + "\n```");
    expect(html).toContain('class="quiz-block"');
    expect(JSON.parse(decodeAttr(html, "data-quiz")!)).toEqual({
      prompt: "Which graph is NOT two-colourable?",
      options: ["A 4-cycle", "A triangle"],
      answer: "A triangle",
    });
    expect(html).not.toContain("<pre"); // the fence is fully replaced
  });

  it("an answer that is not among the options earns the error card + the raw fence", async () => {
    const html = await renderLesson('```quiz\n{"prompt": "?", "options": ["a", "b"], "answer": "c"}\n```');
    expect(html).toContain("workbench-error");
    expect(html).toContain("Quiz ignored");
    expect(html).toContain("<pre"); // the raw fence stays visible for the author
    expect(html).not.toContain("quiz-block");
  });

  it("invalid JSON earns the error card as well", async () => {
    const html = await renderLesson("```quiz\n{ not json\n```");
    expect(html).toContain("Quiz ignored");
    expect(html).toContain("<pre");
  });
});

describe("mermaid fences → diagram placeholder", () => {
  it("a ```mermaid fence becomes an empty mermaid-block div carrying the URI-encoded source", async () => {
    const src = "flowchart LR\n  A --> B";
    const html = await renderLesson("```mermaid\n" + src + "\n```");
    expect(html).toContain('class="mermaid-block"');
    expect(decodeAttr(html, "data-source")).toBe(src);
    expect(html).not.toContain("<pre"); // the fence is fully replaced, not also highlighted
  });

  it("round-trips diagram syntax with arrows and quoted labels through the entity+URI layers", async () => {
    const src = 'graph TD\n  X["a & b"] -->|"yes"| Y';
    const html = await renderLesson("```mermaid\n" + src + "\n```");
    expect(decodeAttr(html, "data-source")).toBe(src);
  });

  it("does not turn a non-mermaid fence into a mermaid-block", async () => {
    const html = await renderLesson("```text\nflowchart LR\n  A --> B\n```");
    expect(html).not.toContain("mermaid-block");
    expect(html).toContain("<pre"); // stays a plain highlighted block
  });
});

describe("viz widget fences → declarative-widget placeholder", () => {
  const PAYLOAD = '{"steps":[{"nodes":[{"id":"0","label":"5","kind":"cell","slot":0}],"annotation":"start"}]}';

  it("a ```viz widget=array fence becomes a viz-widget div carrying the structure + payload", async () => {
    const html = await renderLesson("```viz widget=array\n" + PAYLOAD + "\n```");
    expect(html).toContain('class="viz-widget"');
    expect(html).toContain('data-widget="array"');
    expect(JSON.parse(decodeAttr(html, "data-payload")!)).toEqual(JSON.parse(PAYLOAD));
    expect(html).not.toContain("<pre");
  });

  it("round-trips a payload with quotes through the entity + URI layers", async () => {
    const payload = '{"steps":[{"nodes":[],"annotation":"it\'s \\"5\\""}]}';
    const html = await renderLesson("```viz widget=array\n" + payload + "\n```");
    expect(JSON.parse(decodeAttr(html, "data-payload")!)).toEqual(JSON.parse(payload));
  });

  it("invalid JSON earns an error card and keeps the raw fence", async () => {
    const html = await renderLesson("```viz widget=array\n{ not json\n```");
    expect(html).toContain("workbench-error");
    expect(html).toContain("Widget");
    expect(html).toContain("<pre");
    expect(html).not.toContain('class="viz-widget"');
  });

  it("a ```viz fence with no widget= attribute stays plain code", async () => {
    const html = await renderLesson("```viz\nsome text\n```");
    expect(html).not.toContain("viz-widget");
    expect(html).toContain("<pre");
  });
});

describe("simulator fences → iframe placeholder", () => {
  it("a ```simulator name= fence becomes an empty simulator-block div, no <pre", async () => {
    const html = await renderLesson("```simulator name=osi-encapsulation\n```");
    expect(html).toContain('class="simulator-block"');
    expect(html).toContain('data-name="osi-encapsulation"');
    expect(html).not.toContain("data-height"); // unauthored knobs stay absent
    expect(html).not.toContain("<pre");
  });

  it("height and a quoted title round-trip to attributes", async () => {
    const html = await renderLesson('```simulator name=osi-encapsulation height=560 title="OSI Encapsulation"\n```');
    expect(html).toContain('data-height="560"');
    expect(html).toContain('data-title="OSI Encapsulation"');
  });

  it("missing name= earns the error card and keeps the raw fence", async () => {
    const html = await renderLesson("```simulator\n```");
    expect(html).toContain("workbench-error");
    expect(html).toContain("Simulator ignored");
    expect(html).toContain("<pre");
    expect(html).not.toContain('class="simulator-block"');
  });

  it("a non-slug name earns the error card too", async () => {
    const html = await renderLesson("```simulator name=Not_A_Slug\n```");
    expect(html).toContain("Simulator ignored");
    expect(html).not.toContain('class="simulator-block"');
  });

  it("a malformed height earns the error card naming the simulator", async () => {
    const html = await renderLesson("```simulator name=osi-encapsulation height=tall\n```");
    expect(html).toContain("workbench-error");
    expect(html).toContain("height must be a whole number");
    expect(html).not.toContain('class="simulator-block"');
  });
});

describe("trusted raw HTML passthrough (no sanitizer — ADR-S015)", () => {
  it("passes a <details> editorial through unmodified", async () => {
    const md = ["<details>", "<summary>Editorial</summary>", "", "The walkthrough.", "", "</details>"].join("\n");
    const html = await renderLesson(md);
    expect(html).toContain("<details>");
    expect(html).toContain("<summary>Editorial</summary>");
    expect(html).toContain("The walkthrough.");
  });
});

// End-to-end: one realistic lesson exercising every feature in a single render,
// so interactions between them (a code fence after a table, raw HTML beside prose)
// are covered — not just each feature in isolation.
describe("end-to-end: a complete realistic lesson", () => {
  it("renders prose + table + a runnable fence + details into one coherent document", async () => {
    const lesson = [
      "# Measuring Cost",
      "",
      "Cost has two axes: **time** and *space*. See [Big-O](https://example.com/big-o).",
      "",
      "## Comparison",
      "",
      "| Structure | Lookup |",
      "| --------- | ------ |",
      "| Array     | O(1)   |",
      "| List      | O(n)   |",
      "",
      "Run it:",
      "",
      "```python run viz=array",
      "print(sum(range(10)))",
      "```",
      "",
      "> Amortised cost can differ.",
      "",
      "<details>",
      "<summary>Proof</summary>",
      "",
      "The amortised argument.",
      "",
      "</details>",
    ].join("\n");

    const html = await renderLesson(lesson);

    // headings + slugs, emphasis, links
    expect(html).toMatch(/<h1[^>]*id="measuring-cost"/);
    expect(html).toMatch(/<h2[^>]*id="comparison"/);
    expect(html).toContain("<strong>time</strong>");
    expect(html).toContain('href="https://example.com/big-o"');
    // GFM table
    expect(html).toContain("<table>");
    expect(html).toMatch(/<td[^>]*>O\(1\)<\/td>/);
    // the runnable fence → an interactive workbench placeholder (steps 11 · 24), not a highlighted <pre>
    expect(html).toContain('class="workbench"');
    expect(workbenchVariants(html)).toEqual([{ lang: "python", source: "print(sum(range(10)))", viz: "array" }]);
    // a blockquote and trusted raw <details> survive in the same document
    expect(html).toContain("<blockquote>");
    expect(html).toContain("<details>");
    expect(html).toContain("<summary>Proof</summary>");
  });
});

// Frame runs are the one vocabulary carried by images rather than a fence, so these cases run
// through the WHOLE pipeline: they prove the pre-pass survives remark-rehype and, above all,
// that an ordinary image is still an ordinary image — 1769 single diagrams depend on it.
describe("frame-image runs → one stepping placeholder", () => {
  const CAPTION = "Iterating through the sorted array";

  function frames(caption: string, total: number): string {
    return Array.from(
      { length: total },
      (_unused, at) => `![${caption} — frame ${at + 1} of ${total}](/media/sweep-${at + 1}.png)`,
    ).join("\n\n");
  }

  it("collapses a marked run into one placeholder, consuming the marker and every <img>", async () => {
    const html = await renderLesson(
      `Before.\n\n// Interactive Diagram (3 frames): ${CAPTION}\n\n${frames(CAPTION, 3)}\n\nAfter.`,
    );
    expect(html).toContain('class="frame-slideshow"');
    expect(JSON.parse(decodeAttr(html, "data-frames")!)).toEqual([
      "/media/sweep-1.png",
      "/media/sweep-2.png",
      "/media/sweep-3.png",
    ]);
    expect(decodeAttr(html, "data-caption")).toBe(CAPTION);
    expect(html).not.toContain("Interactive Diagram");
    expect(html).not.toContain("<img"); // the whole point: three pictures became one figure
  });

  it("groups an unmarked run too — the alt is the signal", async () => {
    const html = await renderLesson(frames(CAPTION, 5));
    expect(html).toContain('class="frame-slideshow"');
    expect(JSON.parse(decodeAttr(html, "data-frames")!)).toHaveLength(5);
  });

  it("leaves an ordinary image a plain <img>", async () => {
    const html = await renderLesson("![A picture of an array](/media/array.png)");
    expect(html).toContain('<img src="/media/array.png"');
    expect(html).not.toContain("frame-slideshow");
    expect(html).not.toContain("prose-figure");
  });

  it("captions a lone image under a `// Diagram:` marker", async () => {
    const html = await renderLesson("// Diagram: An interval on the x-axis\n\n![An interval on the x-axis](/media/i.png)");
    expect(html).toContain('class="prose-figure"');
    expect(html).toContain("<figcaption>An interval on the x-axis</figcaption>");
    expect(html).toContain('loading="lazy"');
    expect(html).not.toContain("// Diagram:");
  });

  it("round-trips a caption whose escaped brackets remark unescapes on both paths", async () => {
    // The marker text and the image alt take different routes out of remark; the run only groups
    // (and the caption only matches) because both land as `leftArr[i]`.
    const caption = String.raw`Compare leftArr\[i\] and rightArr\[j\]`;
    const plain = "Compare leftArr[i] and rightArr[j]";
    const html = await renderLesson(
      `// Interactive Diagram (2 frames): ${caption}\n\n` +
        `![${caption} — frame 1 of 2](/a.png)\n\n![${caption} — frame 2 of 2](/b.png)`,
    );
    expect(decodeAttr(html, "data-caption")).toBe(plain);
    expect(JSON.parse(decodeAttr(html, "data-frames")!)).toEqual(["/a.png", "/b.png"]);
  });

  it("escapes a caption carrying markup rather than injecting it", async () => {
    const html = await renderLesson('// Diagram: X <= size of the list\n\n![bound](/b.png)');
    expect(html).toContain("<figcaption>X &lt;= size of the list</figcaption>");
  });

  it("leaves frame syntax inside a fence as highlighted code", async () => {
    const html = await renderLesson("```text\n![X — frame 1 of 2](/a.png)\n```");
    expect(html).not.toContain("frame-slideshow");
    expect(html).toContain("<pre");
  });

  it("coexists with a d2 fence — both pre-passes run, in source order", async () => {
    const html = await renderLesson(`${frames("A", 2)}\n\n\`\`\`d2\nx -> y\n\`\`\`\n\n${frames("B", 2)}`);
    expect(html.match(/class="frame-slideshow"/g)).toHaveLength(2);
    expect(html).toContain('class="d2-block"');
    expect(html.indexOf("d2-block")).toBeGreaterThan(html.indexOf("frame-slideshow"));
  });
});

describe("highlightCode — the lazy-workbench placeholder (qna Q1/B)", () => {
  it("tokenizes a known language with the css-variables theme", async () => {
    const html = await (await import("./render")).highlightCode("print('hi')", "python");
    expect(html).toContain("<pre");
    expect(html).toContain("--shiki-");
  });

  it("falls back to plaintext for an unknown language instead of throwing", async () => {
    const html = await (await import("./render")).highlightCode("whatever", "not-a-language");
    expect(html).toContain("<pre");
  });
});
