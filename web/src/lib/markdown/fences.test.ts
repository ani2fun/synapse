// Putting a diagram back where it came from. The lexer half is pinned against remark in
// `renderD2Script.test.ts`; this covers the splice, which is the half that can quietly damage
// someone's lesson — replacing the wrong fence, or losing the frontmatter the server insists on.
import { describe, expect, it } from "vitest";

import { FenceMoved, replaceFence, splitHead } from "../../islands/d2lab/AddToLesson";
import { d2Fences, fences } from "./fences";

const LESSON = [
  "---",
  "title: A lesson",
  "---",
  "",
  "Prose.",
  "",
  "```d2",
  "first -> one",
  "```",
  "",
  "More prose.",
  "",
  "```d2",
  "run -> a",
  "```",
  "```d2",
  "run -> b",
  "```",
  "",
  "````d2 boards name=\"w\"",
  "walk -> through",
  "````",
  "",
  "Closing prose.",
  "",
].join("\n");

const NEW = "```d2\nreplaced -> me\n```";

describe("splitting a lesson from its frontmatter", () => {
  it("rejoins into exactly the file it came from", () => {
    // `head + body` is how every proposal is rebuilt, so any newline this loses becomes an
    // unrelated whitespace change in someone's pull request.
    for (const file of [LESSON, "no frontmatter\n\n```d2\nx -> y\n```\n", "---\nonly: head\n---\n", ""]) {
      const [head, body] = splitHead(file);
      expect(head + body).toBe(file);
    }
  });
});

describe("finding fences", () => {
  it("reads every d2 fence in order, long ones included", () => {
    expect(d2Fences(LESSON).map((f) => f.source)).toEqual([
      "first -> one",
      "run -> a",
      "run -> b",
      "walk -> through",
    ]);
  });

  it("carries offsets that cut the whole block out", () => {
    const [first] = d2Fences(LESSON);
    expect(LESSON.slice(first!.start, first!.end)).toBe("```d2\nfirst -> one\n```");
  });

  it("keeps a four-backtick block's own runs in its slice", () => {
    const walk = d2Fences(LESSON)[3]!;
    expect(LESSON.slice(walk.start, walk.end)).toBe('````d2 boards name="w"\nwalk -> through\n````');
    expect(walk.meta).toBe('boards name="w"');
  });

  it("ignores a fence quoted inside a longer one", () => {
    expect(fences("````markdown\n```d2\nx -> y\n```\n````").map((f) => f.lang)).toEqual(["markdown"]);
  });
});

describe("replacing a diagram in its lesson", () => {
  const bodyOf = (file: string) => splitHead(file)[1];

  it("replaces the one named and leaves the rest alone", () => {
    const out = replaceFence(LESSON, 0, 1, NEW, "first -> one");
    expect(d2Fences(out).map((f) => f.source)).toEqual([
      "replaced -> me",
      "run -> a",
      "run -> b",
      "walk -> through",
    ]);
    expect(out).toContain("More prose.");
    expect(out).toContain("Closing prose.");
  });

  it("keeps the frontmatter byte-for-byte", () => {
    // The server rejects a proposal that lost its frontmatter, so this is not cosmetic.
    const out = replaceFence(LESSON, 0, 1, NEW, "first -> one");
    expect(splitHead(out)[0]).toBe(splitHead(LESSON)[0]);
    expect(out.startsWith("---\ntitle: A lesson\n---")).toBe(true);
  });

  it("replaces a whole run as one figure", () => {
    // Two adjacent fences are one card, so editing it is one replacement, not two.
    const out = replaceFence(LESSON, 1, 2, NEW, "run -> a");
    expect(d2Fences(out).map((f) => f.source)).toEqual([
      "first -> one",
      "replaced -> me",
      "walk -> through",
    ]);
  });

  it("replaces a long fence including its own backtick runs", () => {
    const out = replaceFence(LESSON, 3, 1, NEW, "walk -> through");
    expect(out).not.toContain("````");
    expect(d2Fences(out).map((f) => f.source)).toEqual([
      "first -> one",
      "run -> a",
      "run -> b",
      "replaced -> me",
    ]);
  });

  it("refuses when the diagram at that position is no longer the one that was opened", () => {
    // The guard that matters: without it this would overwrite somebody else's diagram with yours.
    expect(() => replaceFence(LESSON, 0, 1, NEW, "something -> else")).toThrow(FenceMoved);
  });

  it("refuses when the lesson no longer has a diagram there", () => {
    expect(() => replaceFence(LESSON, 9, 1, NEW, "first -> one")).toThrow(FenceMoved);
  });

  it("is exact — nothing outside the fence moves", () => {
    const before = bodyOf(LESSON);
    const after = bodyOf(replaceFence(LESSON, 0, 1, NEW, "first -> one"));
    const cut = (text: string) => text.replace(/```d2[\s\S]*?```/, "");
    expect(cut(after)).toBe(cut(before));
  });
});

describe("editing a diagram does not change what KIND of diagram it is", () => {
  // The bug this pins: the editor built its outgoing fence as `d2 boards name=… root=…`
  // unconditionally, so proposing an update to a plain ```d2 figure rewrote it as a one-board
  // walkthrough — moving its artifact out of the shared pool into a `_d2/` sidecar and changing
  // how it renders, none of which anyone asked for by clicking Edit.
  const outgoing = (opened: string, source: string) => {
    const boards = /(?:^|\s)boards(?:$|\s)/.test(opened);
    const meta = boards ? 'boards name="w" root="Context"' : "";
    return `\`\`\`d2${meta === "" ? "" : ` ${meta}`}\n${source}\n\`\`\`\n`;
  };

  it("leaves a plain fence plain", () => {
    const out = replaceFence(LESSON, 0, 1, outgoing("", "first -> one"), "first -> one");
    const replaced = d2Fences(out)[0]!;
    expect(replaced.meta).toBe("");
    // Scoped to the fence that was replaced — the lesson's OWN walkthrough further down still
    // carries `boards`, and asserting over the whole file would pass for the wrong reason.
    expect(out.slice(replaced.start, replaced.end)).toBe("```d2\nfirst -> one\n```");
  });

  it("keeps a walkthrough a walkthrough", () => {
    const out = replaceFence(
      LESSON,
      3,
      1,
      outgoing('boards name="w"', "walk -> through"),
      "walk -> through",
    );
    expect(d2Fences(out)[3]!.meta).toContain("boards");
  });

  it("round-trips a fence byte-for-byte when nothing was edited", () => {
    // The strongest form: opening a diagram and proposing it unchanged must be a no-op diff.
    const before = d2Fences(LESSON)[0]!;
    const out = replaceFence(LESSON, 0, 1, "```d2\nfirst -> one\n```\n", before.source);
    expect(out).toBe(LESSON);
  });
});
