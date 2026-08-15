// Tidy rewrites the author's own source, so the bar is that it only ever changes leading
// whitespace — and above all that it does not mistake a decision node for a block.
import { describe, expect, it } from "vitest";

import { tidyMermaid } from "./mermaidTidy";

/** The property that matters most: nothing but indentation may change. */
const stripped = (source: string) =>
  source
    .split("\n")
    .map((line) => line.trim())
    .join("\n");

describe("re-indenting a mermaid diagram", () => {
  it("puts the body one level under the header", () => {
    expect(tidyMermaid("flowchart LR\na --> b\nb --> c\n")).toBe(
      "flowchart LR\n  a --> b\n  b --> c\n",
    );
  });

  it("leaves a decision node alone", () => {
    // The reason d2's brace-counting tidy cannot be reused: `{` here opens a diamond, not a block,
    // and counting it would indent the whole rest of the diagram one level deeper for good.
    const source = 'flowchart LR\n  reader --> edge{"Cached?"}\n  edge -->|hit| cdn\n';
    expect(tidyMermaid(source)).toBe(source);
  });

  it("indents a subgraph and closes it on end", () => {
    expect(tidyMermaid("flowchart LR\nsubgraph edge\na --> b\nend\nc --> d\n")).toBe(
      "flowchart LR\n  subgraph edge\n    a --> b\n  end\n  c --> d\n",
    );
  });

  it("nests subgraphs", () => {
    const out = tidyMermaid("flowchart LR\nsubgraph a\nsubgraph b\nx --> y\nend\nend\n");
    expect(out).toBe("flowchart LR\n  subgraph a\n    subgraph b\n      x --> y\n    end\n  end\n");
  });

  it("sets alt/else/end one level out from their bodies", () => {
    const out = tidyMermaid(
      "sequenceDiagram\nA->>B: ask\nalt found\nB-->>A: yes\nelse missing\nB-->>A: no\nend\n",
    );
    expect(out).toBe(
      [
        "sequenceDiagram",
        "  A->>B: ask",
        "  alt found",
        "    B-->>A: yes",
        "  else missing",
        "    B-->>A: no",
        "  end",
        "",
      ].join("\n"),
    );
  });

  it("indents a classDiagram's brace block", () => {
    // A line that ENDS with `{` opens a block; a decision node does not.
    expect(tidyMermaid("classDiagram\nclass Animal {\n+int age\n}\n")).toBe(
      "classDiagram\n  class Animal {\n    +int age\n  }\n",
    );
  });

  it("copies frontmatter through untouched — it is YAML", () => {
    const source = "---\nconfig:\n  theme: forest\n---\nflowchart LR\na --> b\n";
    expect(tidyMermaid(source)).toBe("---\nconfig:\n  theme: forest\n---\nflowchart LR\n  a --> b\n");
  });

  it("keeps comments at the depth of what they annotate", () => {
    expect(tidyMermaid("%% a note\nflowchart LR\n%% about this\na --> b\n")).toBe(
      "%% a note\nflowchart LR\n  %% about this\n  a --> b\n",
    );
  });

  it("is idempotent", () => {
    const source = 'flowchart LR\nsubgraph edge\nreader --> gate{"Cached?"}\nend\n';
    const once = tidyMermaid(source);
    expect(tidyMermaid(once)).toBe(once);
  });

  it("changes nothing but leading whitespace", () => {
    const source = [
      "---",
      "title: A diagram",
      "---",
      "%% note",
      "flowchart LR",
      '        reader --> gate{"Cached?"}',
      "subgraph inner",
      "   a --> b",
      "end",
      "",
    ].join("\n");
    expect(stripped(tidyMermaid(source))).toBe(stripped(source));
  });

  it("leaves a source it does not recognise as it was", () => {
    // No header it knows, so nothing is claimed to be a body — the worst case is a no-op.
    const source = "notADiagram\nsome --> thing\n";
    expect(tidyMermaid(source)).toBe(source);
  });

  it("never indents below zero on a stray end", () => {
    expect(tidyMermaid("flowchart LR\nend\nend\na --> b\n")).toBe(
      "flowchart LR\n  end\n  end\n  a --> b\n",
    );
  });
});
