// Pipeline spec for the markdown reader. Pure node — no DOM, no Scala.js.
// Ports Cortex's render.test.ts fixtures. Asserts the current scope: the GFM
// core renders as safe HTML, fenced code is shiki-highlighted, and a run-fence
// group (one or more adjacent ```lang run fences, + an optional ```testcases
// JSON fence) becomes ONE `workbench` placeholder the client mounts an editor
// into (steps 11 · 24). A ```mermaid fence becomes a `.mermaid-block` diagram
// placeholder the client renders as SVG (step 24). The remaining reserved
// fences (d2 / viz widget / orphan testcases) still render as PLAIN
// highlighted code — their discovery hooks are later Phase-4 work (ADR-S015).
import { describe, expect, it, vi } from "vitest";

import { renderLesson } from "./render";

// Mock the multi-MB d2 WASM (step 25): the real thing is verified in-browser. The stub echoes the source
// (so grouping/counts are assertable) and throws on "BROKEN" (so the error path is testable). `compileCalls`
// lets us prove d2 is never imported/invoked on a diagram-free document.
const d2Spy = vi.hoisted(() => ({ compileCalls: 0 }));
vi.mock("@terrastruct/d2", () => ({
  D2: class {
    async compile(src: string) {
      d2Spy.compileCalls += 1;
      if (src.includes("BROKEN")) throw new Error("parse error near BROKEN");
      return { diagram: { src } };
    }
    async render(d: { src: string }, opts: { salt: string }) {
      return `<svg class="d2" data-salt="${opts.salt}">${d.src}</svg>`;
    }
  },
}));

// Pull a data attribute back out of a workbench placeholder, the way the client does:
// getAttribute() (which decodes HTML entities — node here has no DOM, so mirror it) → decodeURIComponent.
function decodeAttr(html: string, name: string): string | undefined {
  const encoded = html.match(new RegExp(`${name}="([^"]*)"`))?.[1];
  if (encoded === undefined) return undefined;
  const attr = encoded
    .replace(/&#x27;/g, "'")
    .replace(/&#39;/g, "'")
    .replace(/&quot;/g, '"')
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&"); // last — un-escapes any remaining &-sequences without double-decoding
  return decodeURIComponent(attr);
}

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

  it("captures a `viz=<structure>` hint from the fence meta onto the variant (step 30)", async () => {
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

describe("solution fences → spoiler-safe placeholder (step 16)", () => {
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

describe("practice-problem fences → one grouped placeholder (step 30)", () => {
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

  it("adjacent approach-tagged editorial fences become tagged entries, in order (step 30)", async () => {
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

describe("quiz fences → interactive card placeholder (step 16)", () => {
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

describe("mermaid fences → diagram placeholder (step 24)", () => {
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

describe("d2 fences → parse-time SVG placeholders (step 25)", () => {
  it("a lone ```d2 fence becomes a d2-block carrying the rendered SVG", async () => {
    const html = await renderLesson("```d2\nx -> y\n```");
    expect(html).toContain('class="d2-block"');
    expect(decodeAttr(html, "data-svg")).toContain("<svg"); // the stubbed SVG rode along
    expect(decodeAttr(html, "data-svg")).toContain("x -> y"); // ...echoing the source
    expect(html).not.toContain("<pre"); // the fence is replaced, not also highlighted
    expect(html).not.toContain("d2-slides");
  });

  it("consecutive ```d2 fences group into ONE d2-slideshow with each rendered SVG", async () => {
    const html = await renderLesson("```d2\na -> b\n```\n\n```d2\nc -> d\n```");
    expect(html).toContain('class="d2-slideshow"');
    expect(html).not.toContain("d2-block");
    const slides = JSON.parse(decodeAttr(html, "data-slides")!) as string[];
    expect(slides).toHaveLength(2);
    expect(slides[0]).toContain("a -> b");
    expect(slides[1]).toContain("c -> d");
  });

  it("a paragraph between two d2 fences breaks the group into two d2-blocks", async () => {
    const html = await renderLesson("```d2\na -> b\n```\n\nBetween.\n\n```d2\nc -> d\n```");
    expect(html).not.toContain("d2-slides");
    expect(html.match(/class="d2-block"/g) ?? []).toHaveLength(2);
  });

  it("a broken d2 fence earns a visible error card and keeps the raw fence", async () => {
    const html = await renderLesson("```d2\nBROKEN garbage\n```");
    expect(html).toContain("diagram-error");
    expect(html).toContain("D2 diagram failed");
    expect(html).toContain("<pre"); // the raw fence stays for the author
    expect(html).not.toContain("d2-block");
  });

  it("never imports/invokes d2 on a diagram-free document", async () => {
    const before = d2Spy.compileCalls;
    await renderLesson("# Just prose\n\n```python\nprint(1)\n```\n\n```mermaid\nflowchart LR\n A-->B\n```");
    expect(d2Spy.compileCalls).toBe(before);
  });
});

describe("viz widget fences → declarative-widget placeholder (step 26)", () => {
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
