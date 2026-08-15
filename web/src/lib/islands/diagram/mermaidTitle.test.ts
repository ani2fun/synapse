// Renaming a diagram rewrites the author's own source, so the bar is that it changes the title
// and NOTHING else — `config:` lives in the same block, and mermaid reads both out of it.
import { describe, expect, it } from "vitest";

import { titleOf, withTitle } from "./mermaidTitle";

const TITLED = ["---", "title: The request path", "---", "flowchart LR", "  a --> b", ""].join("\n");
const BARE = ["flowchart LR", "  a --> b", ""].join("\n");

describe("reading a diagram's title", () => {
  it("reads it out of the leading frontmatter block", () => {
    expect(titleOf(TITLED)).toBe("The request path");
  });

  it("has none when there is no block", () => {
    expect(titleOf(BARE)).toBeNull();
  });

  it("unquotes a value that had to be quoted", () => {
    expect(titleOf('---\ntitle: "Reads: the path"\n---\nflowchart LR\n')).toBe("Reads: the path");
    expect(titleOf("---\ntitle: 'it''s here'\n---\nflowchart LR\n")).toBe("it's here");
  });

  it("ignores a `---` that is not the very first line", () => {
    // mermaid's own regex is anchored at the start; further down it is a horizontal rule or the
    // diagram's own syntax, and reading a title out of it would rename the wrong thing.
    expect(titleOf("flowchart LR\n---\ntitle: not frontmatter\n---\n")).toBeNull();
  });

  it("ignores an unterminated block, as mermaid does", () => {
    expect(titleOf("---\ntitle: dangling\nflowchart LR\n")).toBeNull();
  });

  it("ignores an indented `title:` belonging to something else", () => {
    // Under `config:` this is a setting, not the diagram's name.
    const source = "---\nconfig:\n  title: a setting\n---\nflowchart LR\n";
    expect(titleOf(source)).toBeNull();
  });
});

describe("renaming a diagram", () => {
  it("adds a block to a source that had none", () => {
    expect(withTitle(BARE, "Fresh")).toBe("---\ntitle: Fresh\n---\nflowchart LR\n  a --> b\n");
  });

  it("replaces the title in place and leaves the diagram alone", () => {
    const out = withTitle(TITLED, "Another path");
    expect(titleOf(out)).toBe("Another path");
    expect(out).toContain("flowchart LR\n  a --> b");
  });

  it("keeps everything else in the block", () => {
    const source = "---\nconfig:\n  theme: forest\ntitle: Old\n---\nflowchart LR\n";
    const out = withTitle(source, "New");
    expect(out).toContain("config:\n  theme: forest");
    expect(titleOf(out)).toBe("New");
  });

  it("quotes a value YAML would read back as something else", () => {
    expect(withTitle(BARE, "Reads: the path")).toContain('title: "Reads: the path"');
    expect(titleOf(withTitle(BARE, "Reads: the path"))).toBe("Reads: the path");
  });

  it("round-trips titles that need quoting", () => {
    for (const title of ["plain", "with: colon", "#hash", "- dash", "quote\" here", "{brace}"]) {
      expect(titleOf(withTitle(BARE, title))).toBe(title);
    }
  });

  it("drops the block entirely when the last key is cleared", () => {
    // An untitled diagram should be spelled the way an author would spell it, not left as a husk.
    expect(withTitle(TITLED, "")).toBe(BARE);
  });

  it("keeps the block when clearing the title leaves other keys", () => {
    const source = "---\nconfig:\n  theme: forest\ntitle: Old\n---\nflowchart LR\n";
    const out = withTitle(source, "");
    expect(titleOf(out)).toBeNull();
    expect(out).toContain("config:\n  theme: forest");
  });

  it("is a no-op when clearing a title that was never there", () => {
    expect(withTitle(BARE, "")).toBe(BARE);
  });

  it("trims, so a stray space does not become a quoted title", () => {
    expect(withTitle(BARE, "  Padded  ")).toContain("title: Padded");
  });
});
