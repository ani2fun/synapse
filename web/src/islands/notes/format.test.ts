import { describe, expect, it } from "vitest";

import { applyFormat, spliceOf } from "./format";
import type { Edit, FormatAction } from "./format";

/** Write a buffer with the selection marked `«` … `»` (or a lone `|` caret), apply, and render
 *  the result the same way — so each case reads as before → after. */
function run(marked: string, action: FormatAction): string {
  const caret = marked.indexOf("|");
  let text: string;
  let start: number;
  let end: number;
  if (caret >= 0) {
    text = marked.slice(0, caret) + marked.slice(caret + 1);
    start = end = caret;
  } else {
    start = marked.indexOf("«");
    end = marked.indexOf("»") - 1;
    text = marked.slice(0, start) + marked.slice(start + 1, end + 1) + marked.slice(end + 2);
  }
  return show(applyFormat(text, start, end, action));
}

function show({ text, start, end }: Edit): string {
  if (start === end) return `${text.slice(0, start)}|${text.slice(start)}`;
  return `${text.slice(0, start)}«${text.slice(start, end)}»${text.slice(end)}`;
}

describe("inline wraps", () => {
  it("wraps a selection and keeps it selected", () => {
    expect(run("a «word» b", "bold")).toBe("a **«word»** b");
    expect(run("a «word» b", "italic")).toBe("a _«word»_ b");
    expect(run("a «word» b", "strike")).toBe("a ~~«word»~~ b");
    expect(run("a «word» b", "code")).toBe("a `«word»` b");
  });

  it("plants a selected placeholder at a caret", () => {
    expect(run("a | b", "bold")).toBe("a **«bold text»** b");
  });

  it("toggles off when the selection is already wrapped", () => {
    expect(run("a **«word»** b", "bold")).toBe("a «word» b");
    expect(run("a «**word**» b", "bold")).toBe("a «word» b");
  });

  it("does not mistake bold's stars for italic", () => {
    expect(run("**«word»**", "italic")).toBe("**_«word»_**");
  });
});

describe("line prefixes", () => {
  it("bullets the caret's line and parks the caret at its end", () => {
    expect(run("one\ntw|o\nthree", "bullet")).toBe("one\n- two|\nthree");
  });

  it("numbers every selected line in order, skipping blank lines", () => {
    expect(run("«a\n\nb\nc»", "numbered")).toBe("«1. a\n\n2. b\n3. c»");
  });

  it("toggles off when every line already carries the marker", () => {
    expect(run("«- a\n- b»", "bullet")).toBe("«a\nb»");
    expect(run("«> a\n> b»", "quote")).toBe("«a\nb»");
  });

  it("converts one list kind to another rather than stacking markers", () => {
    expect(run("«1. a\n2. b»", "bullet")).toBe("«- a\n- b»");
    expect(run("«- a»", "task")).toBe("«- [ ] a»");
  });

  it("does not read a checklist item as a bullet", () => {
    expect(run("«- [ ] a»", "bullet")).toBe("«- a»");
  });

  it("does not claim the line a selection merely ends at", () => {
    expect(run("«a\n»b", "quote")).toBe("«> a»\nb");
  });

  it("starts a fresh item on an empty line", () => {
    expect(run("|", "task")).toBe("- [ ] |");
  });

  it("replaces a heading level instead of nesting one", () => {
    expect(run("### Ti|tle", "heading")).toBe("## Title|");
  });
});

describe("blocks and links", () => {
  it("fences a selection on lines of its own", () => {
    expect(run("see «x = 1» here", "codeblock")).toBe("see \n```\n«x = 1»\n```\n here");
  });

  it("opens an empty fence at a caret with the caret inside", () => {
    expect(run("|", "codeblock")).toBe("```\n|\n```");
  });

  it("links a label and selects the url slot", () => {
    expect(run("«docs»", "link")).toBe("[docs](«https://»)");
  });

  it("links a selected url and selects the label slot", () => {
    expect(run("«https://x.dev»", "link")).toBe("[«link text»](https://x.dev)");
  });

  it("images carry a bang", () => {
    expect(run("|", "image")).toBe("![alt text](«https://»)");
  });
});

describe("spliceOf", () => {
  it("finds the smallest changed region", () => {
    expect(spliceOf("a word b", "a word!! b")).toEqual({ from: 6, to: 6, insert: "!!" });
    // Two separated insertions are one splice spanning both — the selection between them rides along.
    expect(spliceOf("a word b", "a **word** b")).toEqual({ from: 2, to: 6, insert: "**word**" });
  });

  it("reports a pure deletion as an empty insert", () => {
    expect(spliceOf("a **word** b", "a word** b")).toEqual({ from: 2, to: 4, insert: "" });
  });

  it("applies back to the target", () => {
    const before = "- a\n- b";
    const after = "a\nb";
    const { from, to, insert } = spliceOf(before, after);
    expect(before.slice(0, from) + insert + before.slice(to)).toBe(after);
  });
});
