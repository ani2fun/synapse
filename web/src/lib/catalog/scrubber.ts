// The reader menu's vertical scrubber — the pure half. No DOM, no window: every function takes
// the page geometry it needs, so the mapping between a position on the track and a scroll offset
// is testable on its own (`islands/reader/scrubber.ts` is the DOM half).
//
// THE MODEL: the track stands for the whole document, top to bottom. The thumb is the viewport —
// its top is how far down the reader has scrolled, its height how much of the document one screen
// shows. A tick marks a heading at the thumb position where that heading has ARRIVED, i.e. where a
// jump to it (`scrollToHeading`) would leave the thumb. So dragging the thumb onto a tick and
// jumping to that heading land on the same scroll offset.

/** The page geometry the scrubber maps between, in CSS pixels. */
export interface Geometry {
  /** The document's full scroll height. */
  doc: number;
  /** The viewport's height. */
  view: number;
}

/** A heading as measured on the page: its distance from the top of the document. */
export interface MeasuredHeading {
  id: string;
  text: string;
  level: number;
  top: number;
}

/** A heading placed on the track. */
export interface Tick {
  id: string;
  text: string;
  level: number;
  /** The scroll offset that lands this heading under the header. */
  y: number;
  /** The same place as a fraction of the track — where the thumb's top sits on arrival. */
  at: number;
}

/** The outline marks a heading current a little BEFORE it arrives — 40px of lead, which is the
 *  reader chrome's own "top <= 120" rule against an 80px landing offset. */
export const READING_LEAD = 40;

export function maxScroll(g: Geometry): number {
  return Math.max(0, g.doc - g.view);
}

export function clampScroll(y: number, g: Geometry): number {
  return Math.min(Math.max(y, 0), maxScroll(g));
}

/** Where the thumb sits, as fractions of the track. A page shorter than its viewport is all
 *  thumb. */
export function thumb(scrollY: number, g: Geometry): { top: number; height: number } {
  if (g.doc <= 0 || g.doc <= g.view) return { top: 0, height: 1 };
  return { top: clampScroll(scrollY, g) / g.doc, height: g.view / g.doc };
}

/** The scroll offset that puts the thumb's top at `top` (a track fraction). */
export function scrollForThumbTop(top: number, g: Geometry): number {
  return clampScroll(top * g.doc, g);
}

/** How far through the lesson, 0–100, for the slider's `aria-valuenow`. */
export function percent(scrollY: number, g: Geometry): number {
  const max = maxScroll(g);
  if (max === 0) return 0;
  return Math.round((clampScroll(scrollY, g) / max) * 100);
}

/** Place each heading on the track. `landing` is how far below the viewport top a jump leaves a
 *  heading (the header offset). A heading the page cannot scroll far enough to land still gets
 *  its tick, at the deepest offset the page reaches. */
export function ticksOf(headings: MeasuredHeading[], landing: number, g: Geometry): Tick[] {
  if (g.doc <= 0) return [];
  return headings.map((h) => {
    const y = clampScroll(h.top - landing, g);
    return { id: h.id, text: h.text, level: h.level, y, at: y / g.doc };
  });
}

/** The heading in force at a scroll offset: the last one the reader has reached, counting the
 *  reading lead. Null above the first heading — the lesson's opening prose belongs to none. */
export function sectionAt(scrollY: number, ticks: Tick[]): Tick | null {
  let found: Tick | null = null;
  for (const tick of ticks) {
    if (tick.y <= scrollY + READING_LEAD) found = tick;
    else break;
  }
  return found;
}

/** The next heading below (`dir` 1) or the previous one above (`dir` -1) a scroll offset. A tick
 *  within a pixel of where the reader already is counts as HERE, not as the next stop, or a jump
 *  would land and then refuse to move on. */
export function neighbour(scrollY: number, ticks: Tick[], dir: 1 | -1): Tick | null {
  if (dir === 1) return ticks.find((t) => t.y > scrollY + 1) ?? null;
  for (let i = ticks.length - 1; i >= 0; i--) {
    const tick = ticks[i];
    if (tick && tick.y < scrollY - 1) return tick;
  }
  return null;
}

/** How close (in track PIXELS) a dragged thumb must come to a section start before it snaps
 *  onto it. Pixels rather than a fraction of the lesson, so a long lesson and a short one snap
 *  the same amount under the finger. Wider for a finger than for a mouse: a fingertip covers
 *  ~40px of track, a mouse pointer one. */
export const SNAP_PX_COARSE = 12;
export const SNAP_PX_FINE = 5;

/**
 * Where a dragged thumb settles. `top` is the thumb top the pointer asked for (a track fraction),
 * `ticks` are the section starts, `trackPx` the track's height on screen, and `coarse` is true
 * for a finger (a touch pointer) and false for a mouse or pen.
 *
 * A light, live snap: within `SNAP_PX_COARSE` / `SNAP_PX_FINE` pixels of a tick, the thumb lands
 * on that tick's `at`; anywhere else it stays exactly where the pointer put it. Only the NEAREST
 * tick may claim it — clustered h3s can sit closer together than the zone, and the first one in
 * range would make the second unreachable.
 */
export function snapToTick(top: number, ticks: Tick[], trackPx: number, coarse: boolean): number {
  if (trackPx <= 0) return top;
  const zone = (coarse ? SNAP_PX_COARSE : SNAP_PX_FINE) / trackPx;
  let nearest: Tick | null = null;
  for (const tick of ticks) {
    if (nearest === null || Math.abs(tick.at - top) < Math.abs(nearest.at - top)) nearest = tick;
  }
  return nearest !== null && Math.abs(nearest.at - top) <= zone ? nearest.at : top;
}
