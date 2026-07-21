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

/** The two shapes stylesheet damage takes; both make the browser drop rules. */
type Damage = { kind: "parse-error" | "file-scope-declaration"; where: string };

function inspect(css: string, name: string): Damage[] {
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
