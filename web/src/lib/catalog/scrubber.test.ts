// The scrubber's mapping between a track position and a scroll offset, pinned on one geometry: a
// 4000px document read through a 1000px viewport, so the page scrolls 3000px.

import { describe, expect, it } from "vitest";
import {
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
  it("leavesAPositionFarFromEveryTickWhereThePointerPutIt", () => {
    const ticks = ticksOf(HEADINGS, LANDING, PAGE);
    expect(snapToTick(0.5, ticks, 600)).toBe(0.5);
  });

  it.todo("pullsAPositionNearATickOntoIt");
});
