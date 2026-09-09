import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

import postcss from "postcss";
import { describe, expect, it } from "vitest";

// ─────────────────────────────────────────────────────────────────────────────
// STYLESHEET SANITY (step 40)
// A stylesheet that fails to parse does NOT fail loudly — the browser's error
// recovery silently discards the damaged region AND the rule that follows it,
// so real styling disappears with nothing in the console. Step 25 left the
// declaration bodies of the deleted `.header__search*` rules orphaned at file
// scope; recovery swallowed the next rule, `.cmdk-scrim`, and the ⌘K palette
// lost `position: fixed` — it rendered in normal flow at the page's bottom-left
// and stayed that way for fifteen steps. This suite makes that class of damage
// fail in CI instead of in the reader's browser.
// ─────────────────────────────────────────────────────────────────────────────

const STYLES_DIR = fileURLToPath(new URL(".", import.meta.url));

/** The three shapes stylesheet damage takes; all of them make the browser drop rules. */
type Damage = {
  kind: "parse-error" | "file-scope-declaration" | "unterminated-comment";
  where: string;
};

/**
 * A comment that never closed, so the NEXT comment's `*` + `/` closes it — and every rule in
 * between is commented out. The browser says nothing, because a comment is not an error, and
 * postcss says nothing either: the comment it parses is perfectly well formed, it is simply far
 * longer than its author meant.
 *
 * `labshell.css` shipped exactly this for two commits. An extraction left a comment's first line
 * behind without its close; `.lab-acts`, `.lab-seg` and `.lab-primary` fell inside it and died on
 * all three lab pages, so every primary button rendered as a bare UA `<button>`. Nothing in the
 * console, nothing in a build, and the page still looked broadly right.
 *
 * The tell is a whole RULE inside comment text — a selector, a brace and a declaration. Prose
 * about CSS quotes selectors and properties all the time; it does not write out rule bodies.
 * Deliberately commented-out CSS trips this too, which is the right answer: dead rules kept "just
 * in case" are what this file exists to keep out of the tree.
 */
const RULE_IN_PROSE = /^[ \t]*[.#][\w-][^{}\n]*\{[^}\n]*[\w-]+\s*:\s*[^}\n]+;/m;

function swallowedRules(css: string): Damage[] {
  const damage: Damage[] = [];
  let at = 0;
  for (;;) {
    const open = css.indexOf("/*", at);
    if (open < 0) return damage;
    const close = css.indexOf("*/", open + 2);
    const line = css.slice(0, open).split("\n").length;
    if (close < 0) {
      damage.push({ kind: "unterminated-comment", where: `line ${line}: runs to the end of the file` });
      return damage;
    }
    const found = RULE_IN_PROSE.exec(css.slice(open + 2, close));
    if (found) {
      damage.push({
        kind: "unterminated-comment",
        where: `line ${line}: this comment swallows a rule — ${found[0].trim().slice(0, 60)}…`,
      });
    }
    at = close + 2;
  }
}

function inspect(css: string, name: string): Damage[] {
  const swallowed = swallowedRules(css);
  if (swallowed.length > 0) return swallowed;
  let root: postcss.Root;
  try {
    root = postcss.parse(css, { from: name });
  } catch (error) {
    // A stray `}` (or an unclosed block) — the shape the real bug took.
    return [{ kind: "parse-error", where: (error as Error).message }];
  }
  // Declarations stranded outside any rule — the shape it takes when the braces
  // happen to balance. postcss keeps them; a browser discards them and the
  // following rule with them.
  return root.nodes
    .filter((node): node is postcss.Declaration => node.type === "decl")
    .map((decl) => ({
      kind: "file-scope-declaration" as const,
      where: `line ${decl.source?.start?.line}: ${decl.prop}: ${decl.value}`,
    }));
}

describe("stylesheets", () => {
  const sheets = readdirSync(STYLES_DIR).filter((f) => f.endsWith(".css"));

  it("ships at least the sheets we know about", () => {
    // Guards the guard: an empty glob would make every check below vacuous.
    expect(sheets.length).toBeGreaterThanOrEqual(15);
  });

  it.each(sheets)("%s parses with no rules silently dropped", (sheet) => {
    const path = join(STYLES_DIR, sheet);
    const damage = inspect(readFileSync(path, "utf8"), path);
    expect(damage, `${sheet}: ${JSON.stringify(damage, null, 2)}`).toEqual([]);
  });

  // ── the detector itself, against both shapes of the real bug ──────────────

  it("catches a stray closing brace (the ⌘K palette's actual damage)", () => {
    const damage = inspect(
      `.cmdk__x { color: red; }\n  margin-left: 1rem; cursor: pointer; }\n.cmdk-scrim { position: fixed; }`,
      "fixture.css",
    );
    expect(damage.map((d) => d.kind)).toEqual(["parse-error"]);
  });

  it("catches declarations orphaned at file scope when braces balance", () => {
    const damage = inspect(`.a { color: red; }\n  margin-left: 1rem;\n.b { color: blue; }`, "fixture.css");
    expect(damage).toEqual([
      { kind: "file-scope-declaration", where: "line 2: margin-left: 1rem" },
    ]);
  });
});
