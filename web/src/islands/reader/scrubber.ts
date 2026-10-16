import * as log from "../../lib/log";
// The reader menu's vertical scrubber — the DOM half. A tall track down the right edge that
// stands for the whole lesson: the thumb is the screen the reader is looking at, and a tick marks
// each section start. Dragging (or tapping) moves through the page; the arrow keys step from
// section to section. It exists only while the reader menu is open, so it costs nothing while the
// reader reads — its scroll listener and its resize observer go when it does. The mapping itself is `lib/catalog/scrubber.ts`.

import {
  neighbour,
  percent,
  scrollForThumbTop,
  sectionAt,
  snapToTick,
  thumb,
  ticksOf,
  clampScroll,
  maxScroll,
  type Geometry,
  type Tick,
} from "../../lib/catalog/scrubber";
import { HEADER_OFFSET, documentTop, type Heading } from "./headings";

/** The thumb never draws shorter than this, or a long lesson's thumb is a sliver no finger finds. */
const MIN_THUMB_PX = 28;
/** PageUp/PageDown move this share of the whole lesson. */
const PAGE_STEP = 0.1;
/** How long the section bubble stays after a keyboard step. */
const BUBBLE_MS = 1200;

export interface Scrubber {
  el: HTMLElement;
  destroy: () => void;
}

/** Scroll at once. The page sets `scroll-behavior: smooth` on <html>, which a bare
 *  `scrollTo(x, y)` honours — so every pointer move started an animation and the page trailed
 *  the finger instead of following it. */
function scrollNow(top: number): void {
  window.scrollTo({ top, behavior: "instant" });
}

function geometry(): Geometry {
  return { doc: document.documentElement.scrollHeight, view: window.innerHeight };
}

/** Mount the scrubber into `host` for these headings. */
export function mountScrubber(host: HTMLElement, headings: Heading[]): Scrubber {
  const el = document.createElement("div");
  el.className = "reader-scrub";
  el.tabIndex = 0;
  el.setAttribute("role", "slider");
  el.setAttribute("aria-orientation", "vertical");
  el.setAttribute("aria-label", "Position in this lesson");
  el.setAttribute("aria-valuemin", "0");
  el.setAttribute("aria-valuemax", "100");

  const track = document.createElement("div");
  track.className = "reader-scrub__track";
  const thumbEl = document.createElement("div");
  thumbEl.className = "reader-scrub__thumb";
  const bubble = document.createElement("div");
  bubble.className = "reader-scrub__bubble";
  bubble.hidden = true;
  el.append(track, bubble);

  let ticks: Tick[] = [];
  let tickEls: HTMLElement[] = [];

  /** Measure the headings against the page as it is NOW — the reading preferences reflow it. */
  const measure = (): void => {
    const g = geometry();
    const measured = headings.flatMap((h) => {
      const top = documentTop(h.id);
      return top === null ? [] : [{ ...h, top }];
    });
    ticks = ticksOf(measured, HEADER_OFFSET, g);
    for (const t of tickEls) t.remove();
    tickEls = ticks.map((tick) => {
      const mark = document.createElement("span");
      mark.className = tick.level >= 3 ? "reader-scrub__tick reader-scrub__tick--l3" : "reader-scrub__tick";
      mark.style.top = `${(tick.at * 100).toFixed(3)}%`;
      return mark;
    });
    track.replaceChildren(...tickEls, thumbEl);
    log.debug(`scrubber: ${ticks.length} section tick(s) on a ${g.doc}px lesson`);
  };

  /** The thumb in track pixels, with its drawn minimum height kept inside the track. */
  const thumbPx = (trackPx: number): { top: number; height: number } => {
    const t = thumb(window.scrollY, geometry());
    const height = Math.min(trackPx, Math.max(t.height * trackPx, MIN_THUMB_PX));
    const top = Math.min(t.top * trackPx, trackPx - height);
    return { top, height };
  };

  const showBubble = (text: string | null, topPx: number): void => {
    bubble.hidden = text === null;
    if (text === null) return;
    bubble.textContent = text;
    bubble.style.top = `${topPx}px`;
  };

  let dragging = false;
  let bubbleTimer: ReturnType<typeof setTimeout> | null = null;

  const paint = (): void => {
    const trackPx = track.clientHeight;
    const { top, height } = thumbPx(trackPx);
    thumbEl.style.transform = `translateY(${top}px)`;
    thumbEl.style.height = `${height}px`;
    const section = sectionAt(window.scrollY, ticks);
    el.setAttribute("aria-valuenow", String(percent(window.scrollY, geometry())));
    el.setAttribute("aria-valuetext", section ? section.text : "Start of the lesson");
    if (dragging || bubbleTimer !== null) showBubble(section ? section.text : "Start of the lesson", top + height / 2);
  };

  let frame = 0;
  const onScroll = (): void => {
    if (frame !== 0) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      paint();
    });
  };
  // Re-measure whenever the PAGE changes height, not only the window: the settings view beside
  // this scrubber reflows the whole lesson (size, leading, width), and an opened `<details>` or a
  // late image does the same. One frame per burst of changes.
  let measureFrame = 0;
  const onResize = (): void => {
    if (measureFrame !== 0) return;
    measureFrame = requestAnimationFrame(() => {
      measureFrame = 0;
      measure();
      paint();
    });
  };
  const resizes = new ResizeObserver(onResize);

  // ── pointer: grab the thumb where it was touched, or centre it under a tap on the track ──
  let grab = 0;
  /** `coarse` — a finger rather than a mouse or pen, which snaps from further away. */
  const moveTo = (clientY: number, coarse: boolean): void => {
    const rect = track.getBoundingClientRect();
    if (rect.height <= 0) return;
    const wanted = (clientY - rect.top - grab) / rect.height;
    const top = snapToTick(Math.min(Math.max(wanted, 0), 1), ticks, rect.height, coarse);
    scrollNow(scrollForThumbTop(top, geometry()));
    paint();
  };
  el.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    el.focus({ preventScroll: true });
    const rect = track.getBoundingClientRect();
    const { top, height } = thumbPx(rect.height);
    const at = event.clientY - rect.top;
    grab = at >= top && at <= top + height ? at - top : height / 2;
    dragging = true;
    el.classList.add("reader-scrub--dragging");
    el.setPointerCapture(event.pointerId);
    moveTo(event.clientY, event.pointerType === "touch");
    log.debug(`scrubber: drag start (${event.pointerType})`);
  });
  el.addEventListener("pointermove", (event) => {
    if (dragging) moveTo(event.clientY, event.pointerType === "touch");
  });
  const endDrag = (): void => {
    if (!dragging) return;
    dragging = false;
    el.classList.remove("reader-scrub--dragging");
    showBubble(null, 0);
    log.debug(`scrubber: drag end at ${percent(window.scrollY, geometry())}%`);
  };
  el.addEventListener("pointerup", endDrag);
  el.addEventListener("pointercancel", endDrag);

  // ── keyboard: section to section, a tenth at a time, or end to end ──
  const jump = (y: number): void => {
    scrollNow(clampScroll(y, geometry()));
    if (bubbleTimer !== null) clearTimeout(bubbleTimer);
    bubbleTimer = setTimeout(() => {
      bubbleTimer = null;
      showBubble(null, 0);
    }, BUBBLE_MS);
    paint();
  };
  el.addEventListener("keydown", (event) => {
    const y = window.scrollY;
    const g = geometry();
    let next: number | null = null;
    switch (event.key) {
      case "ArrowDown":
      case "ArrowRight":
        next = neighbour(y, ticks, 1)?.y ?? maxScroll(g);
        break;
      case "ArrowUp":
      case "ArrowLeft":
        next = neighbour(y, ticks, -1)?.y ?? 0;
        break;
      case "PageDown":
        next = y + maxScroll(g) * PAGE_STEP;
        break;
      case "PageUp":
        next = y - maxScroll(g) * PAGE_STEP;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = maxScroll(g);
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
    jump(next);
  });

  host.append(el);
  measure();
  paint();
  window.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("resize", onResize);
  resizes.observe(document.body);

  const destroy = (): void => {
    window.removeEventListener("scroll", onScroll);
    window.removeEventListener("resize", onResize);
    resizes.disconnect();
    if (frame !== 0) cancelAnimationFrame(frame);
    if (measureFrame !== 0) cancelAnimationFrame(measureFrame);
    if (bubbleTimer !== null) clearTimeout(bubbleTimer);
    el.remove();
  };
  return { el, destroy };
}
