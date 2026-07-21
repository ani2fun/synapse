/**
 * The embedded practice-problem decode, with APPROACH TABS. Pure: the URI-decoded attribute
 * strings a `.practice-problem` placeholder carries in, a `PracticeSpec` out; the workbench half
 * reuses the SAME `parseVariants`/`TestSpec` decode the plain workbench placeholders use. A
 * malformed practice problem (no variants, blank statement) decodes to `null` and silently
 * disappears — it never crashes the reader.
 *
 * `solutionComplexities` lives here rather than in `blocks.ts` because the editorial model
 * (`catalog/editorial.ts`) reads a solution fence's `time=/space=` claims through it, so both
 * consumers share this one parser.
 */

import { parseVariants } from "./blocks";
import type { Variant } from "./blocks";
import type { TestSpec } from "./judge";

/** One editorial approach: the tab label ("Brute Force" · "Optimal" · "Editorial") + its markdown. */
export interface Approach {
  label: string;
  md: string;
}

/** One authored practice problem: the statement, the starter workbench, the editorials. */
export interface PracticeSpec {
  problemMd: string;
  variants: Variant[];
  spec: TestSpec | null;
  editorials: Approach[];
}

/** The wire shape of one `data-editorials` entry. */
interface EditorialWire {
  tag?: string;
  md?: string;
}

/**
 * Decoded attribute payloads → the spec. `null` when the statement is blank or no variant
 * survives (`parseVariants` already drops blank-language entries and returns `null` for empty).
 */
export function decodePractice(
  problemMd: string,
  variantsJson: string,
  specJson: string | null,
  editorialsJson: string | null,
): PracticeSpec | null {
  const problem = problemMd.trim();
  if (problem === "") return null;
  const variants = parseVariants(variantsJson);
  if (!variants || variants.length === 0) return null;

  let spec: TestSpec | null = null;
  if (specJson != null) {
    try {
      spec = JSON.parse(specJson) as TestSpec;
    } catch {
      spec = null;
    }
  }

  let wire: EditorialWire[] = [];
  if (editorialsJson != null) {
    try {
      const parsed: unknown = JSON.parse(editorialsJson);
      if (Array.isArray(parsed)) wire = parsed as EditorialWire[];
    } catch {
      wire = [];
    }
  }

  return { problemMd: problem, variants, spec, editorials: labelApproaches(wire) };
}

/**
 * `approach-brute-force-1` → "Brute Force" (numbered only when the same kind repeats); a bare or
 * unrecognised tag → "Editorial"; blank-body entries drop. Order is authoring order.
 */
function labelApproaches(wire: EditorialWire[]): Approach[] {
  const kinds = wire.map((entry) => approachKind(entry.tag ?? ""));
  const counters = new Map<string, number>();
  const out: Approach[] = [];
  wire.forEach((entry, index) => {
    const md = (entry.md ?? "").trim();
    if (md === "") return;
    const kind = kinds[index]!;
    const repeats = kinds.filter((k) => k === kind).length > 1;
    let label: string;
    if (repeats) {
      const n = (counters.get(kind) ?? 0) + 1;
      counters.set(kind, n);
      label = `${kind} ${n}`;
    } else {
      label = kind;
    }
    out.push({ label, md });
  });
  return out;
}

/** The human kind behind a tag: strip `approach-`, strip a trailing `-<n>`, title-case. */
function approachKind(tag: string): string {
  const prefix = "approach-";
  if (!tag.startsWith(prefix)) return "Editorial";
  let kind = tag.slice(prefix.length);
  const lastDash = kind.lastIndexOf("-");
  if (lastDash >= 0) {
    const suffix = kind.slice(lastDash + 1);
    // `rsplit_once('-')` with an all-digits (vacuously true for empty) tail is dropped.
    if ([...suffix].every((c) => c >= "0" && c <= "9")) kind = kind.slice(0, lastDash);
  }
  const words = kind
    .split("-")
    .filter((w) => w !== "")
    .map((w) => w[0]!.toUpperCase() + w.slice(1));
  return words.length === 0 ? "Editorial" : words.join(" ");
}

/**
 * A solution fence's meta carries `time=O(…) space=O(…)` claims, extracted here. A value may
 * contain spaces (`time=O(log N)`, `time=O(min(N1, N2))`) — following tokens are pulled in until
 * its parentheses balance, so whitespace inside the O-group never truncates it.
 */
export function solutionComplexities(meta: string): [string, string][] {
  const depthDelta = (s: string): number => {
    let depth = 0;
    for (const c of s) {
      if (c === "(") depth += 1;
      else if (c === ")") depth -= 1;
    }
    return depth;
  };
  const out: [string, string][] = [];
  const tokens = meta.split(/\s+/).filter((t) => t !== "");
  let idx = 0;
  while (idx < tokens.length) {
    const token = tokens[idx]!;
    idx += 1;
    const eq = token.indexOf("=");
    if (eq < 0) continue;
    const name = token.slice(0, eq);
    if (name !== "time" && name !== "space") continue;
    let value = token.slice(eq + 1);
    let depth = depthDelta(value);
    while (depth > 0) {
      if (idx >= tokens.length) break;
      const next = tokens[idx]!;
      idx += 1;
      value += " " + next;
      depth += depthDelta(next);
    }
    out.push([name, value]);
  }
  return out;
}
