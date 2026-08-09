// The ahead-of-time renderer (`dev-tools/render-d2.mjs`) and the reader's own renderer must agree
// EXACTLY. The script names a file after `fnv1a(source)` and draws it with a salt and options of
// its own; the page looks that file up by the same hash and inlines it. If either half drifts,
// every lookup misses — and a miss is silent by design, so the only symptom is that diagrams get
// slow again on a page that still renders perfectly. That is what this file is here to prevent.
//
// The script cannot import the app's copies (it runs in a content repo's CI, from a plain Node
// checkout with no TypeScript), so it carries its own. These assertions are the seam.
import type { Code, Root } from "mdast";
import remarkParse from "remark-parse";
import { unified } from "unified";
import { describe, expect, it } from "vitest";

import { d2Salt, d2RenderOptions, LAYOUT } from "../islands/diagram/d2";
import { fnv1a } from "../hash";
// eslint-disable-next-line import/no-unresolved
import * as script from "../../../../dev-tools/render-d2.mjs";

const CORPUS = [
  "x -> y",
  "hello -> world",
  "",
  "a",
  "x: {style.shadow: true}\ny\nx -> y",
  "unicode: café → naïve ☕",
  "  leading and trailing whitespace matters  ",
  "a really quite long one\n".repeat(40),
];

describe("the ahead-of-time renderer agrees with the reader's", () => {
  it("hashes every source identically", () => {
    for (const source of CORPUS) {
      expect(script.fnv1a(source), `hash of ${JSON.stringify(source.slice(0, 24))}`).toBe(
        fnv1a(source),
      );
    }
  });

  it("names a file after the salt the reader will look up", () => {
    for (const source of CORPUS) {
      // `d2Salt` with a fresh tally is a diagram's FIRST occurrence — what the script draws.
      expect(script.saltFor(source)).toBe(d2Salt(source, new Map()));
    }
  });

  it("uses the same layout engine", () => {
    expect(script.LAYOUT).toBe(LAYOUT);
  });

  it("uses the same render options", () => {
    expect(script.renderOptions("d2-abc12345")).toEqual(d2RenderOptions("d2-abc12345"));
  });
});

// The script reads markdown with a regex; the reader reads it with remark. The string they end up
// with is the CACHE KEY, so "close enough" is a silent miss — a trailing newline is all it takes.
// These compare against remark itself rather than against a hand-written expectation, because a
// hand-written one just encodes whichever of the two happened to be written first.
function remarkD2Fences(markdown: string): string[] {
  const tree = unified().use(remarkParse).parse(markdown) as Root;
  return tree.children
    .filter((n): n is Code => n.type === "code" && (n.lang ?? "").trim().toLowerCase() === "d2")
    .map((n) => n.value);
}

describe("the script's fence lexer agrees with remark, byte for byte", () => {
  const DOCUMENTS: Record<string, string> = {
    "a lone fence": "```d2\nx -> y\n```",
    "several, in order": "```d2\na -> b\n```\n\ntext\n\n```d2\nc -> d\n```",
    "adjacent fences (a slideshow)": "```d2\na -> b\n```\n```d2\nc -> d\n```",
    "an upper-case tag": "```D2\nx -> y\n```",
    "a multi-line body": "```d2\nvars: {\n  d2-config: {\n    theme-id: 0\n  }\n}\nx -> y\n```",
    "indented content": "```d2\nx: {\n  shape: circle\n}\n```",
    "a body with a blank line": "```d2\na -> b\n\nc -> d\n```",
    "other languages alongside": "```bash\nx -> y\n```\n\n```d2\nx -> y\n```\n\n```mermaid\ngraph TD; A-->B;\n```",
    "no d2 at all": "```bash\nls\n```\n\nprose",
  };

  for (const [name, markdown] of Object.entries(DOCUMENTS)) {
    it(name, () => {
      expect(script.d2Fences(markdown)).toEqual(remarkD2Fences(markdown));
    });
  }

  it("agrees on the real lesson the tour is built from", () => {
    // The shape that actually ships: every diagram printed twice, once as ```bash and once live.
    const md = ["### Heading", "", "```bash", "x -> y", "```", "", "```d2", "x -> y", "```", ""].join(
      "\n",
    );
    expect(script.d2Fences(md)).toEqual(remarkD2Fences(md));
    expect(script.d2Fences(md)).toHaveLength(1);
  });
});
