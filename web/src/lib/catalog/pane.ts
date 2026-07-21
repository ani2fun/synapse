// The problem page's splitter width and the label matcher the editorial shares.
//
// A legacy format also carried the ACTIVE TAB and the active editorial section across problem
// pages, packed as `tab|pct|section`; that's since been removed (a new problem opening on
// someone else's Editorial-and-Solution is a spoiler you never asked for). What stayed is the
// splitter width, because dragging a pane is a layout act rather than a place in the material.
//
// The tab vocabulary itself is NOT here: the problem page renders exactly three tabs
// (Description · Editorial · Submissions), the Coach tab arrives with the tutoring island, and
// the editorial STEPPER that consumes `sectionIndex` lives in `editorial.ts`. `normalizeLabel`/
// `sectionIndex` live in this module because it owns them — "the label matcher the editorial
// shares" — so they ride along with the width helpers and their tests rather than being invented
// again elsewhere.

// ─────────────────────────────────────────────────────────────────────────────
// THE SPLITTER WIDTH — the only thing the problem page still remembers
// ─────────────────────────────────────────────────────────────────────────────

/** The splitter's travel, matching the drag clamp in the island. */
export const MIN_LEFT_PCT = 28.0;
export const MAX_LEFT_PCT = 64.0;
export const DEFAULT_LEFT_PCT = 46.0;

/**
 * Parse a stored splitter width. Anything unreadable — including a legacy `tab|pct|section`
 * record, which is why an existing reader's width resets exactly once — degrades to the default.
 */
export function parseLeftPct(stored: string | null): number {
  if (stored === null) return DEFAULT_LEFT_PCT;
  // A bare width parses; a legacy `editorial|52.50|Solution` record does not (Number("editorial…")
  // is NaN), and neither does an empty string — `Number("")` alone would have silently accepted
  // an empty string as 0, but `stored.trim() === ""` catches it first.
  if (stored.trim() === "") return DEFAULT_LEFT_PCT;
  const pct = Number(stored);
  if (!Number.isFinite(pct)) return DEFAULT_LEFT_PCT;
  return Math.min(Math.max(pct, MIN_LEFT_PCT), MAX_LEFT_PCT);
}

/**
 * Two decimals, matching the precision the island actually renders — a raw drag lands on
 * `55.67703952901598`, and there is no reason to keep sixteen digits of it.
 */
export function serializeLeftPct(leftPct: number): string {
  return leftPct.toFixed(2);
}

// ─────────────────────────────────────────────────────────────────────────────
// SECTION MATCHING — by normalised label, never by index (editorial.ts consumes this)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Lowercase, trimmed, inner whitespace collapsed — so `"Complexity  Analysis"` from one
 * problem's heading matches `"Complexity Analysis"` from another's.
 */
export function normalizeLabel(label: string): string {
  return label.trim().split(/\s+/).join(" ").toLowerCase();
}

/**
 * Which section to reveal, given this editorial's labels and the remembered one. No match —
 * including a blank preference or an editorial that has no sections — reveals the first.
 */
export function sectionIndex(labels: string[], preferred: string): number {
  const wanted = normalizeLabel(preferred);
  if (wanted === "") return 0;
  const at = labels.findIndex((label) => normalizeLabel(label) === wanted);
  return at === -1 ? 0 : at;
}
