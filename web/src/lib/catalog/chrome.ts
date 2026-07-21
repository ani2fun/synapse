// The reader chrome's pure logic. No DOM, no `localStorage` — the island in `islands/chrome.ts`
// harvests headings and drives scroll; this module only does the math, pinned natively so
// vitest can test it directly.

// ─────────────────────────────────────────────────────────────────────────────
// MINIMAP SPREAD — de-overlap heading fractions: min gap 0.05 (capped 1/(n+1));
// forward pass pushes apart, backward clamps.
// ─────────────────────────────────────────────────────────────────────────────

/** De-overlap a sorted-or-unsorted list of document fractions so no two ticks sit closer than a
 *  minimum gap, and every tick stays inside `[gap, 1 - gap]` (forward push / backward clamp /
 *  final clamp passes). */
export function spreadFractions(fractions: readonly number[]): number[] {
  const n = fractions.length;
  if (n === 0) return [];
  const gap = Math.min(0.05, 1.0 / (n + 1));
  const out = [...fractions].sort((a, b) => a - b);
  for (let i = 1; i < n; i += 1) {
    if (out[i] < out[i - 1] + gap) out[i] = out[i - 1] + gap;
  }
  for (let i = n - 1; i >= 0; i -= 1) {
    const above = n - 1 - i;
    const ceiling = 1.0 - gap - above * gap;
    if (out[i] > ceiling) out[i] = ceiling;
    if (i > 0 && out[i] < out[i - 1] + gap) out[i - 1] = out[i] - gap;
  }
  for (let i = 0; i < n; i += 1) {
    out[i] = Math.min(Math.max(out[i], gap), 1.0 - gap);
  }
  return out;
}
