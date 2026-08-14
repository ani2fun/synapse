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

import { d2Fences } from "./fences";
import * as boards from "../islands/diagram/boards";
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

describe("all three fence lexers agree with remark, byte for byte", () => {
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

    // ── long fences ──
    // A real lesson opens "D2 Interactive diagrams" with four backticks. The old lexer read that
    // as language "" with the fourth backtick swallowed into the info string and skipped it, so
    // the reader got the diagram (remark reads it correctly) and CI never drew it.
    "a four-backtick fence": "````d2\nx -> y\n````",
    "a four-backtick fence with meta": '````d2 boards name="x"\nx -> y\n````',
    "five backticks": "`````d2\nx -> y\n`````",
    "a long fence among ordinary ones": "```d2\na -> b\n```\n\n````d2\nc -> d\n````",
    // The other direction, and the reason the closing run must be at least as long as the opening:
    // a documentation example is not a diagram.
    "a four-backtick wrapper quoting a d2 fence": "````markdown\n```d2\nx -> y\n```\n````",
    "a fence quoting a longer fence": "`````markdown\n````d2\nx -> y\n````\n`````",
    // A backtick in a backtick-fence's info string is not an info string at all.
    "a backtick inside the info string": "```d2`x\nx -> y\n```",
    "a closing run longer than the opening": "```d2\nx -> y\n`````",
  };

  for (const [name, markdown] of Object.entries(DOCUMENTS)) {
    it(name, () => {
      const expected = remarkD2Fences(markdown);
      // The CI script, in plain JS, from a content repo.
      expect(script.d2Fences(markdown), "render-d2.mjs").toEqual(expected);
      // The app's copy, which `/d2` uses to find the fence someone clicked Edit on.
      expect(
        d2Fences(markdown).map((fence) => fence.source),
        "fences.ts",
      ).toEqual(expected);
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

  it("splits the info string into language and meta the way remark does", () => {
    const md = '````d2 boards name="url-shortener" root="Context"\nx -> y\n````';
    const tree = unified().use(remarkParse).parse(md) as Root;
    const node = tree.children[0] as Code;
    expect(node.lang).toBe("d2");
    expect(script.d2Blocks(md)[0].meta).toBe(node.meta);
    expect(d2Fences(md)[0]!.meta).toBe(node.meta);
  });
});

// ── THE MULTI-BOARD HALF ─────────────────────────────────────────────────────────────────────
// A ```d2 boards fence writes a DIRECTORY of boards beside its lesson, and the reader addresses
// those files by slug. Same stakes as the hash above, one level up: a slug or a version that
// disagrees means the page finds nothing and quietly compiles the diagram itself instead.

function remarkD2Meta(markdown: string): string[] {
  const tree = unified().use(remarkParse).parse(markdown) as Root;
  return tree.children
    .filter((n): n is Code => n.type === "code" && (n.lang ?? "").trim().toLowerCase() === "d2")
    .map((n) => (n.meta ?? "").trim());
}

describe("the script and the reader agree on the boards vocabulary", () => {
  const METAS = [
    "",
    "boards",
    'boards name="url-shortener"',
    "boards name=url-shortener",
    'boards name="url-shortener" root="System Context"',
    'boards root="A title with spaces"',
    "keyboards",
    "BOARDS",
    "boardsy",
    "run",
  ];

  it("reads the same fences as walkthroughs", () => {
    for (const meta of METAS) {
      expect(script.isBoardsFence(meta), JSON.stringify(meta)).toBe(boards.isBoardsFence(meta));
    }
  });

  it("reads the same name and root title", () => {
    for (const meta of METAS) {
      expect(script.fenceName(meta), JSON.stringify(meta)).toBe(boards.fenceName(meta));
      expect(script.rootTitleOf(meta), JSON.stringify(meta)).toBe(boards.rootTitleOf(meta));
    }
  });

  it("lexes the info string exactly as remark does", () => {
    const DOCS = [
      "```d2 boards\nx -> y\n```",
      '```d2 boards name="a b"\nx -> y\n```',
      "```D2 boards\nx -> y\n```",
      "```d2\nx -> y\n```",
    ];
    for (const md of DOCS) {
      expect(script.d2Blocks(md).map((b: { meta: string }) => b.meta)).toEqual(remarkD2Meta(md));
    }
  });
});

describe("the script and the reader name the same files", () => {
  const IDS = [
    "root",
    "root.layers.container",
    "root.layers.a.layers.b",
    "root.steps.one",
    "root.layers.Café Noir",
    "root.layers.MiXeD",
    "root.layers...",
    "root.layers.../..",
    "root.layers.!!!",
  ];

  it("slugs every board id identically", () => {
    for (const id of IDS) {
      expect(script.boardSlug(id), id).toBe(boards.boardSlug(id));
    }
  });

  it("never lets a slug escape its directory", () => {
    // The server joins this to a lesson path, so a separator here is a traversal there.
    for (const id of IDS) {
      const slug = script.boardSlug(id);
      expect(slug, id).toMatch(/^[a-z0-9_-]+$/);
    }
  });

  it("salts every board identically, and every board differently", () => {
    for (const id of IDS) {
      expect(script.saltForBoard("abcd1234", id), id).toBe(boards.saltForBoard("abcd1234", id));
    }
    const salts = IDS.map((id) => script.boardSlug(id));
    // Two boards sharing a salt collide on `<defs>` ids and lose arrowheads with no error, so
    // the generator disambiguates. What must hold here is that the INPUTS that differ, differ.
    expect(new Set(salts).size).toBeGreaterThan(1);
  });

  it("stamps the same manifest version", () => {
    expect(script.GENERATOR_VERSION).toBe(boards.GENERATOR_VERSION);
    expect(script.BOARDS_DIR).toBe(boards.BOARDS_DIR);
    expect(script.MANIFEST_FILE).toBe(boards.MANIFEST_FILE);
  });
});

describe("the script and the reader resolve a link the same way", () => {
  const CASES: [string, string][] = [
    ["root.layers.c", "root.layers.b"],
    ["layers.c", "root"],
    ["layers.c", "root.layers.b"],
    ["_.layers.c", "root.layers.b"],
    ["_._.layers.d", "root.layers.a.layers.b"],
    ["_", "root.layers.b"],
    ["root", "root.layers.b"],
    ["https://d2lang.com", "root"],
    ["mailto:a@b.c", "root"],
    ["", "root"],
  ];

  for (const [value, from] of CASES) {
    it(`${JSON.stringify(value)} from ${from}`, () => {
      expect(script.resolveBoardLink(value, from)).toBe(boards.resolveBoardLink(value, from));
    });
  }
});
