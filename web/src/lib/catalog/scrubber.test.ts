// The scrubber's mapping between a track position and a scroll offset, pinned on one geometry: a
// 4000px document read through a 1000px viewport, so the page scrolls 3000px.

import { describe, expect, it } from "vitest";
import {
  SNAP_PX_COARSE,
  SNAP_PX_FINE,
  maxScroll,
  neighbour,
  percent,
  scrollForThumbTop,
  sectionAt,
  snapToTick,
  thumb,
  ticksOf,
  type Geometry,
  type MeasuredHeading,
  type Tick,
} from "./scrubber";

const PAGE: Geometry = { doc: 4000, view: 1000 };
const LANDING = 80;

function heading(id: string, top: number, level = 2): MeasuredHeading {
  return { id, text: id, level, top };
}

const HEADINGS = [heading("intro", 600), heading("detail", 1400, 3), heading("wrap-up", 2600)];

describe("geometry", () => {
  it("theThumbIsTheViewportAndClampsAtBothEnds", () => {
    expect(thumb(0, PAGE)).toEqual({ top: 0, height: 0.25 });
    expect(thumb(1500, PAGE)).toEqual({ top: 0.375, height: 0.25 });
    expect(thumb(-50, PAGE).top).toBe(0);
    expect(thumb(9999, PAGE).top).toBe(0.75);
  });

  it("aPageShorterThanItsViewportIsAllThumb", () => {
    expect(thumb(0, { doc: 600, view: 1000 })).toEqual({ top: 0, height: 1 });
    expect(percent(0, { doc: 600, view: 1000 })).toBe(0);
  });

  it("aThumbTopMapsBackToTheScrollItCameFrom", () => {
    const y = 1234;
    expect(scrollForThumbTop(thumb(y, PAGE).top, PAGE)).toBeCloseTo(y);
    expect(scrollForThumbTop(1, PAGE)).toBe(maxScroll(PAGE));
    expect(scrollForThumbTop(-1, PAGE)).toBe(0);
  });

  it("percentRunsFromZeroAtTheTopToAHundredAtTheBottom", () => {
    expect(percent(0, PAGE)).toBe(0);
    expect(percent(1500, PAGE)).toBe(50);
    expect(percent(3000, PAGE)).toBe(100);
  });
});

describe("ticks", () => {
  it("aTickSitsWhereAJumpToItsHeadingLeavesTheThumb", () => {
    const [intro] = ticksOf(HEADINGS, LANDING, PAGE);
    expect(intro?.y).toBe(520);
    expect(intro?.at).toBe(520 / 4000);
    expect(thumb(intro?.y ?? 0, PAGE).top).toBe(intro?.at);
  });

  it("aHeadingThePageCannotScrollToLandsAtTheDeepestOffset", () => {
    const [deep] = ticksOf([heading("footer", 3900)], LANDING, PAGE);
    expect(deep?.y).toBe(maxScroll(PAGE));
  });

  it("theSectionInForceCountsTheReadingLeadAndNothingOwnsTheOpening", () => {
    const ticks = ticksOf(HEADINGS, LANDING, PAGE);
    expect(sectionAt(0, ticks)).toBeNull();
    expect(sectionAt(480, ticks)?.id).toBe("intro");
    expect(sectionAt(479, ticks)).toBeNull();
    expect(sectionAt(2000, ticks)?.id).toBe("detail");
    expect(sectionAt(3000, ticks)?.id).toBe("wrap-up");
  });

  it("stepsMoveOnFromTheHeadingAlreadyLandedOn", () => {
    const ticks = ticksOf(HEADINGS, LANDING, PAGE);
    expect(neighbour(0, ticks, 1)?.id).toBe("intro");
    expect(neighbour(520, ticks, 1)?.id).toBe("detail");
    expect(neighbour(520, ticks, -1)).toBeNull();
    expect(neighbour(1320, ticks, -1)?.id).toBe("intro");
    expect(neighbour(3000, ticks, 1)).toBeNull();
  });
});

describe("snapToTick", () => {
  // A 600px track over the 4000px page: `intro` sits at 520/4000 = 0.13, i.e. 78px down the track.
  const TRACK = 600;
  const ticks = ticksOf(HEADINGS, LANDING, PAGE);
  const intro = ticks[0]?.at ?? NaN;
  /** A track position `px` pixels below (positive) or above (negative) the intro tick. */
  const near = (px: number): number => intro + px / TRACK;

  it("leavesAPositionFarFromEveryTickWhereThePointerPutIt", () => {
    expect(snapToTick(0.5, ticks, TRACK, true)).toBe(0.5);
    expect(snapToTick(0.5, ticks, TRACK, false)).toBe(0.5);
  });

  it("aFingerSnapsFromFurtherAwayThanAMouse", () => {
    expect(snapToTick(near(SNAP_PX_FINE + 3), ticks, TRACK, true)).toBe(intro);
    expect(snapToTick(near(SNAP_PX_FINE + 3), ticks, TRACK, false)).toBe(near(SNAP_PX_FINE + 3));
  });

  it("snapsFromEitherSideAndStopsAtTheEdgeOfTheZone", () => {
    expect(snapToTick(near(-(SNAP_PX_COARSE - 1)), ticks, TRACK, true)).toBe(intro);
    expect(snapToTick(near(SNAP_PX_COARSE + 1), ticks, TRACK, true)).toBe(near(SNAP_PX_COARSE + 1));
    expect(snapToTick(near(SNAP_PX_FINE - 1), ticks, TRACK, false)).toBe(intro);
  });

  it("theNearestTickWinsWhenTwoAreInRange", () => {
    // Two section starts 10px apart on the track — both inside a finger's zone from between them.
    const close: Tick[] = [
      { id: "a", text: "a", level: 3, y: 0, at: 0.3 },
      { id: "b", text: "b", level: 3, y: 0, at: 0.3 + 10 / TRACK },
    ];
    expect(snapToTick(0.3 + 7 / TRACK, close, TRACK, true)).toBe(close[1]?.at);
    expect(snapToTick(0.3 + 3 / TRACK, close, TRACK, true)).toBe(0.3);
  });

  it("aPageWithNoSectionsNeverSnaps", () => {
    expect(snapToTick(0.42, [], TRACK, true)).toBe(0.42);
  });
});
