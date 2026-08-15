// ──────────────────────────────────────────────────────────────────
// RE-INDENTING A MERMAID DIAGRAM
// ──────────────────────────────────────────────────────────────────
// d2's tidy counts braces anywhere on a line, and running that over mermaid would corrupt it:
// `edge{"Cached?"}` is a DECISION NODE, not a block, and a brace-counting pass reads it as one
// level in and never comes back out.
//
// So this indents by BLOCK instead, and the two tests it does use are line-anchored — a line that
// *ends* with `{` opens one, a line that *starts* with `}` closes one. A decision node satisfies
// neither, which is what makes it safe. Everything else is mermaid's own vocabulary: `subgraph`,
// `loop`, `alt` and friends open, `end` closes, `else` and `and` sit between.
//
// Frontmatter is copied through untouched — it is YAML, where indentation is meaning.

/** Blocks that `end` closes. */
const OPENS = /^(subgraph|loop|alt|opt|par|critical|rect|box)\b/;
/** The lines that live one level out from the block they punctuate. */
const MIDDLE = /^(else|and)\b/;
const CLOSES = /^end\b/;

/** The first word decides how everything under it parses, and sits at column zero. */
const HEADER =
  /^(flowchart|graph|sequenceDiagram|classDiagram(-v2)?|stateDiagram(-v2)?|erDiagram|journey|gantt|pie|quadrantChart|requirementDiagram|gitGraph|mindmap|timeline|zenuml|sankey-beta|xychart-beta|block-beta|packet-beta|architecture-beta|kanban|radar|treemap|C4\w*)\b/;

/**
 * `source` re-indented two spaces a level, the house style.
 *
 * Never reflows and never reorders — it only ever replaces the leading whitespace of a line, so
 * the worst it can do to a diagram it does not understand is leave it as it was.
 */
export function tidyMermaid(source: string): string {
  const lines = source.split("\n");
  const out: string[] = [];
  let at = 0;

  // Frontmatter is YAML: its indentation carries meaning, so it is copied through verbatim.
  if (lines[0]?.trim() === "---") {
    const close = lines.findIndex((line, i) => i >= 1 && line.trim() === "---");
    if (close !== -1) {
      out.push(...lines.slice(0, close + 1));
      at = close + 1;
    }
  }

  let depth = 0;
  let started = false; // whether the diagram's own header has been passed
  for (let i = at; i < lines.length; i += 1) {
    const text = lines[i]!.trim();
    if (text === "") {
      out.push("");
      continue;
    }
    // Comments ride at the depth of whatever they annotate.
    if (text.startsWith("%%")) {
      out.push("  ".repeat(depth) + text);
      continue;
    }
    if (!started && HEADER.test(text)) {
      out.push(text); // column zero
      started = true;
      depth = 1;
      continue;
    }
    if (CLOSES.test(text) || text.startsWith("}")) depth = Math.max(started ? 1 : 0, depth - 1);
    const indent = MIDDLE.test(text) ? Math.max(started ? 1 : 0, depth - 1) : depth;
    out.push("  ".repeat(indent) + text);
    if (OPENS.test(text) || text.endsWith("{")) depth += 1;
  }
  return out.join("\n");
}
