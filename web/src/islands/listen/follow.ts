// ──────────────────────────────────────────────────────────────────
// FOLLOW ALONG — highlight what is being said, and keep it on screen
// ──────────────────────────────────────────────────────────────────
// The transcript view a video player has: the unit being spoken is highlighted in the prose and
// the page scrolls to keep it in view. The highlight is a CSS Custom Highlight — a range painted
// by `::highlight(listen-now)` with no element inserted — so the lesson's DOM is never edited and
// selection, copy and the other islands see exactly the page they always did. Where the API is
// missing, the unit's block gets a class instead.
//
// Following yields to the reader, the way a video transcript does: a manual scroll STOPS the page
// chasing the voice, so someone reading ahead or back at a diagram is never pulled away. Whenever
// the sentence being spoken is off screen, `onAway(true)` lets the bar offer "Jump to speech", and
// `jump()` takes the reader there and turns following back on.

import * as log from "../../lib/log";
import type { ProseBlock } from "./extract";
import { rangeOf } from "./extract";
import type { UnitSpan } from "./units";

const HIGHLIGHT = "listen-now";
const BLOCK_CLASS = "listen-now-block";

interface HighlightRegistry {
  set: (name: string, value: unknown) => void;
  delete: (name: string) => void;
}

/** The Custom Highlight registry and constructor, where the browser has them. */
function highlightApi(): { registry: HighlightRegistry; make: (r: Range) => unknown } | null {
  const css = (globalThis as { CSS?: { highlights?: HighlightRegistry } }).CSS;
  const Ctor = (globalThis as { Highlight?: new (...ranges: Range[]) => unknown }).Highlight;
  if (!css?.highlights || !Ctor) return null;
  return { registry: css.highlights, make: (range) => new Ctor(range) };
}

export interface Follower {
  /** Show unit `index` as the one being said; `null` clears the highlight. */
  show: (index: number | null) => void;
  /** Scroll to what is being said and resume following. */
  jump: () => void;
  /** Clear the highlight and stop listening to the page's scrolling. */
  dispose: () => void;
}

/** Whether `rect` sits in the part of the viewport the header and the bar leave free. */
function onScreen(rect: DOMRect): boolean {
  return rect.bottom > 64 && rect.top < window.innerHeight - 96;
}

export function follow(blocks: ProseBlock[], spans: UnitSpan[], onAway: (away: boolean) => void): Follower {
  const api = highlightApi();
  let marked: Element | null = null;
  let current: number | null = null;
  /** Whether the page keeps the voice on screen; a manual scroll turns it off, `jump()` back on. */
  let following = true;
  let programmatic = false;
  const onScroll = (): void => {
    if (!programmatic && following) {
      following = false;
      log.debug("listen: the reader scrolled — following paused");
    }
  };
  const onKey = (e: KeyboardEvent): void => {
    if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End", " "].includes(e.key)) onScroll();
  };
  window.addEventListener("wheel", onScroll, { passive: true });
  window.addEventListener("touchmove", onScroll, { passive: true });
  window.addEventListener("keydown", onKey);

  let currentRange: Range | null = null;
  let currentBlock: Element | null = null;
  /** What is being said, as something with a box — the sentence, or failing that its block. */
  const target = (): DOMRect | null => (currentRange ?? currentBlock)?.getBoundingClientRect() ?? null;
  let away = false;
  const report = (): void => {
    const rect = target();
    const next = rect !== null && !onScreen(rect);
    if (next !== away) {
      away = next;
      onAway(away);
    }
  };
  let frame = 0;
  const onPageScroll = (): void => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(report);
  };
  // Captured on the document: the reader scrolls inside its own container, and an element's scroll
  // event does not bubble — a window listener would never hear it.
  document.addEventListener("scroll", onPageScroll, { capture: true, passive: true });
  const scrollToCurrent = (): void => {
    if (!currentBlock) return;
    programmatic = true;
    currentBlock.scrollIntoView({ block: "center", behavior: "smooth" });
    window.setTimeout(() => (programmatic = false), 800);
  };

  const clear = (): void => {
    api?.registry.delete(HIGHLIGHT);
    marked?.classList.remove(BLOCK_CLASS);
    marked = null;
    currentRange = null;
    currentBlock = null;
  };

  return {
    dispose() {
      clear();
      cancelAnimationFrame(frame);
      window.removeEventListener("wheel", onScroll);
      window.removeEventListener("touchmove", onScroll);
      window.removeEventListener("keydown", onKey);
      document.removeEventListener("scroll", onPageScroll, { capture: true });
    },
    jump() {
      following = true;
      scrollToCurrent();
      log.debug("listen: jumped to the voice");
    },
    show(index) {
      if (index === current) return;
      current = index;
      clear();
      if (index === null) {
        report();
        return;
      }
      const span = spans[index];
      const block = span ? blocks[span.block] : undefined;
      if (!span || !block) return;
      const range = rangeOf(block, span.start, span.end);
      currentRange = range;
      currentBlock = block.element;
      if (api && range) {
        api.registry.set(HIGHLIGHT, api.make(range));
      } else {
        marked = block.element;
        marked.classList.add(BLOCK_CLASS);
      }
      log.debug(`listen: following unit ${index + 1}/${spans.length}`);
      const rect = target();
      if (rect && !onScreen(rect) && following) scrollToCurrent();
      // After a smooth scroll the scroll listener reports again; this covers the cases with none.
      report();
    },
  };
}
