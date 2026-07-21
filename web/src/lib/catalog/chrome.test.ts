// `spreadFractions` de-overlap: three assertions covering the forward push, the backward clamp,
// and the empty-input case.

import { describe, expect, it } from "vitest";
import { spreadFractions } from "./chrome";

describe("spreadFractions", () => {
  it("deOverlapsAndClampsFractions", () => {
    const out = spreadFractions([0.1, 0.11, 0.12]);
    expect(out[1] - out[0]).toBeGreaterThanOrEqual(0.05 - 1e-9);
    expect(out[2] - out[1]).toBeGreaterThanOrEqual(0.05 - 1e-9);

    const edges = spreadFractions([0.0, 1.0]);
    expect(edges[0]).toBeGreaterThanOrEqual(0.05 - 1e-9);
    expect(edges[1]).toBeLessThanOrEqual(0.95 + 1e-9);

    expect(spreadFractions([])).toEqual([]);
  });

  it("keepsSingletonInsideTheGuardBand", () => {
    // n = 1 → gap = min(0.05, 1/2) = 0.05; a mid-page heading is untouched, an edge one clamps.
    expect(spreadFractions([0.5])).toEqual([0.5]);
    expect(spreadFractions([0.0])[0]).toBeCloseTo(0.05, 9);
    expect(spreadFractions([1.0])[0]).toBeCloseTo(0.95, 9);
  });
});
