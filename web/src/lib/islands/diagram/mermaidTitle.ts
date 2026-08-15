// ──────────────────────────────────────────────────────────────────
// A MERMAID DIAGRAM'S TITLE
// ──────────────────────────────────────────────────────────────────
// A ```mermaid fence carries no info string, so — unlike a d2 walkthrough, whose name lives on the
// opening backticks — a mermaid diagram's title lives INSIDE the source, in a leading YAML
// frontmatter block that `preprocessDiagram` pulls off before the lexer sees anything:
//
//   ---
//   title: The request path
//   ---
//   flowchart LR
//     …
//
// That makes the title editable the same way d2's is, and it is why the editor's heading writes
// back into the buffer rather than into the fence.
//
// The block is real YAML to mermaid (it parses `config` out of the same place), so a value that
// would not survive a YAML round trip is quoted rather than written raw.

/** The leading `---` block, if the source opens with one: the lines it spans, and its body. */
function frontmatter(source: string): { from: number; to: number; lines: string[] } | null {
  const lines = source.split("\n");
  // Anchored at the very first line, exactly as mermaid's own regex is — a `---` further down is
  // a horizontal rule or a diagram's own syntax, not frontmatter.
  if (lines[0]?.trim() !== "---") return null;
  const close = lines.findIndex((line, i) => i >= 1 && line.trim() === "---");
  if (close === -1) return null; // unterminated: mermaid does not treat it as frontmatter either
  return { from: 0, to: close, lines: lines.slice(1, close) };
}

const TITLE_LINE = /^(\s*)title\s*:\s*(.*)$/;

/** Strip one layer of YAML quoting from a scalar. */
function unquote(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length >= 2 && /^(".*"|'.*')$/s.test(trimmed)) {
    const inner = trimmed.slice(1, -1);
    return trimmed.startsWith('"') ? inner.replace(/\\(["\\])/g, "$1") : inner.replace(/''/g, "'");
  }
  return trimmed;
}

/**
 * A value YAML will read back as the string it was given.
 *
 * Plain scalars are left plain — that is what an author would have typed — but anything YAML
 * would reinterpret (a colon, a leading indicator character, surrounding space, emptiness) is
 * double-quoted, which is also JSON and so needs no separate escaping rule.
 */
function yamlScalar(value: string): string {
  const safe = value === value.trim() && value !== "" && !/[:#]/.test(value) && !/^[-?*&!|>'"%@`[\]{},]/.test(value);
  return safe ? value : JSON.stringify(value);
}

/** The diagram's title, or null when it has none. */
export function titleOf(source: string): string | null {
  const block = frontmatter(source);
  if (block == null) return null;
  for (const line of block.lines) {
    const found = TITLE_LINE.exec(line);
    // Top-level keys only: an indented `title:` belongs to whatever nests above it (`config:`),
    // and rewriting that one would move a setting rather than rename the diagram.
    if (found != null && found[1] === "") {
      const value = unquote(found[2]!);
      return value === "" ? null : value;
    }
  }
  return null;
}

/**
 * The source with its title set to `title` — or removed, when `title` is blank.
 *
 * Everything else in the block is preserved in place, because `config:` lives there too and a
 * rename must not disturb it. Removing the last key removes the block along with it, so an
 * untitled diagram is spelled the way an author would spell it rather than as an empty husk.
 */
export function withTitle(source: string, title: string): string {
  const wanted = title.trim();
  const block = frontmatter(source);

  if (block == null) {
    if (wanted === "") return source;
    return `---\ntitle: ${yamlScalar(wanted)}\n---\n${source}`;
  }

  const lines = source.split("\n");
  const body = block.lines;
  const at = body.findIndex((line) => {
    const found = TITLE_LINE.exec(line);
    return found != null && found[1] === "";
  });

  let next: string[];
  if (wanted === "") {
    if (at === -1) return source;
    next = [...body.slice(0, at), ...body.slice(at + 1)];
    // Nothing left worth a block — drop the fence too, and the blank lines it was padding.
    if (next.every((line) => line.trim() === "")) {
      const rest = lines.slice(block.to + 1);
      while (rest.length > 0 && rest[0]!.trim() === "") rest.shift();
      return rest.join("\n");
    }
  } else {
    const line = `title: ${yamlScalar(wanted)}`;
    next = at === -1 ? [line, ...body] : [...body.slice(0, at), line, ...body.slice(at + 1)];
  }
  return [...lines.slice(0, block.from), "---", ...next, "---", ...lines.slice(block.to + 1)].join("\n");
}
