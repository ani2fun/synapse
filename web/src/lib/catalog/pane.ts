// The problem page's splitter width and the label matcher the editorial shares (oracle:
// client/src/catalog/logic/pane.rs — its `mod tests` is ported verbatim in `pane.test.ts`).
//
// Step 47 also carried the ACTIVE TAB and the active editorial section across problem pages;
// step 65 removed both (a new problem opening on someone else's Editorial-and-Solution is a
// spoiler you never asked for). What stayed is the splitter width, because dragging a pane is a
// layout act rather than a place in the material.
//
// The tab vocabulary itself is NOT here: A07 renders exactly three tabs (Description · Editorial ·
// Submissions), the Coach tab arrives with the tutoring island (A09), and the editorial STEPPER
// that consumes `sectionIndex` is A08's `editorial.ts`. `normalizeLabel`/`sectionIndex` live in
// this module because the oracle's `pane.rs` owns them — "the label matcher the editorial shares"
// — so they ride along with the width helpers and their parity tests rather than being invented
// again in A08.

// ─────────────────────────────────────────────────────────────────────────────
// THE SPLITTER WIDTH — the only thing the problem page still remembers
// ─────────────────────────────────────────────────────────────────────────────

/** The splitter's travel, matching the drag clamp in the island. */
export const MIN_LEFT_PCT = 28.0;
export const MAX_LEFT_PCT = 64.0;
export const DEFAULT_LEFT_PCT = 46.0;

/**
 * Parse a stored splitter width. Anything unreadable — including a step-47 `tab|pct|section`
 * record, which is why an existing reader's width resets exactly once — degrades to the default.
 * (oracle: `parse_left_pct`)
 */
export function parseLeftPct(stored: string | null): number {
  if (stored === null) return DEFAULT_LEFT_PCT;
  // A bare width parses; a legacy `editorial|52.50|Solution` record does not (Number("editorial…")
  // is NaN), and neither does an empty string (`Number("")` is 0, but `stored.trim() === ""`
  // catches it first — the oracle's `str::parse::<f64>` rejects the empty string outright).
  if (stored.trim() === "") return DEFAULT_LEFT_PCT;
  const pct = Number(stored);
  if (!Number.isFinite(pct)) return DEFAULT_LEFT_PCT;
  return Math.min(Math.max(pct, MIN_LEFT_PCT), MAX_LEFT_PCT);
}

/**
 * Two decimals, matching the precision the island actually renders — a raw drag lands on
 * `55.67703952901598`, and there is no reason to keep sixteen digits of it. (oracle:
 * `serialize_left_pct`)
 */
export function serializeLeftPct(leftPct: number): string {
  return leftPct.toFixed(2);
}

// ─────────────────────────────────────────────────────────────────────────────
// SECTION MATCHING — by normalised label, never by index (A08 consumes this)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Lowercase, trimmed, inner whitespace collapsed — so `"Complexity  Analysis"` from one
 * problem's heading matches `"Complexity Analysis"` from another's. (oracle: `normalize_label`)
 */
export function normalizeLabel(label: string): string {
  return label.trim().split(/\s+/).join(" ").toLowerCase();
}

/**
 * Which section to reveal, given this editorial's labels and the remembered one. No match —
 * including a blank preference or an editorial that has no sections — reveals the first. (oracle:
 * `section_index`)
 */
export function sectionIndex(labels: string[], preferred: string): number {
  const wanted = normalizeLabel(preferred);
  if (wanted === "") return 0;
  const at = labels.findIndex((label) => normalizeLabel(label) === wanted);
  return at === -1 ? 0 : at;
}
