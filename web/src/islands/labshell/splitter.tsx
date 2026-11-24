/**
 * The lab shell's column splitter — the draggable seam between `.lab-pane--l` and `.lab-pane--r`
 * that `/d2`, `/mermaid` and `/viz` all share, alongside `labshell.css`.
 *
 * The width is a percentage of the panes box, clamped to the same travel the problem page uses,
 * and persisted on RELEASE under the caller's key — never at pointer rate. Each lab keeps its own
 * key and its own default, because a diagram editor and a visualiser want different first splits.
 *
 * `axis: "y"` turns the same seam on its side — the code lab's Input/Output rows — and measures
 * the TOP pane's height instead, over a wider travel: two text panes stacked can each shrink
 * further than an editor beside a canvas can.
 */
import type { RefObject } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { MAX_LEFT_PCT, MIN_LEFT_PCT } from "../../lib/catalog/pane";
import { get as storageGet, set as storageSet } from "../../lib/storage";

/** A stored or dragged width, kept inside the splitter's travel. Anything unreadable — absent,
 *  empty, not a number — is the caller's default. */
export function clampLeftPct(value: number, fallback: number, axis: SplitAxis = "x"): number {
  if (!Number.isFinite(value) || value <= 0) return fallback;
  const [min, max] = axis === "x" ? [MIN_LEFT_PCT, MAX_LEFT_PCT] : [MIN_TOP_PCT, MAX_TOP_PCT];
  return Math.min(Math.max(value, min), max);
}

/** Which way the seam runs: `x` splits columns (the default), `y` splits rows. */
export type SplitAxis = "x" | "y";

/** The row seam's travel, in percent of the box's height. */
export const MIN_TOP_PCT = 15;
export const MAX_TOP_PCT = 85;

export interface LabSplitter {
  /** The left pane's width (or, on the `y` axis, the top pane's height), in percent. */
  leftPct: number;
  /** Goes on `.lab-panes` — the box a drag is measured against. */
  panes: RefObject<HTMLDivElement | null>;
  /** Goes on the seam's `pointerdown`. */
  startDrag: (event: PointerEvent) => void;
}

export function useLabSplitter(storageKey: string, defaultPct: number, axis: SplitAxis = "x"): LabSplitter {
  const [leftPct, setLeftPct] = useState(() =>
    clampLeftPct(Number(storageGet(storageKey)), defaultPct, axis),
  );
  const panes = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  /** The width as of the last move, for the release handler, which is installed once. */
  const latest = useRef(leftPct);
  latest.current = leftPct;

  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      const box = panes.current?.getBoundingClientRect();
      if (!dragging.current || box == null) return;
      const [offset, extent] =
        axis === "x" ? [event.clientX - box.left, box.width] : [event.clientY - box.top, box.height];
      if (extent <= 0) return;
      setLeftPct(clampLeftPct((offset / extent) * 100, defaultPct, axis));
    };
    // Fires for every pointerup on the window, so it is gated on `dragging`.
    const onUp = () => {
      if (!dragging.current) return;
      dragging.current = false;
      document.body.style.cursor = "";
      storageSet(storageKey, latest.current.toFixed(2));
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [storageKey, defaultPct, axis]);

  const startDrag = (event: PointerEvent) => {
    event.preventDefault();
    dragging.current = true;
    document.body.style.cursor = axis === "x" ? "col-resize" : "row-resize";
  };

  return { leftPct, panes, startDrag };
}

/** The seam itself. `label` names what dragging resizes, for a screen reader. A `y` seam is a
 *  horizontal bar between rows, which is what `aria-orientation` describes: the bar, not the drag. */
export function LabSplit({
  label,
  onPointerDown,
  axis = "x",
}: {
  label: string;
  onPointerDown: (event: PointerEvent) => void;
  axis?: SplitAxis;
}) {
  return (
    <div
      class={axis === "x" ? "lab-split" : "lab-split lab-split--row"}
      role="separator"
      aria-orientation={axis === "x" ? "vertical" : "horizontal"}
      aria-label={label}
      onPointerDown={onPointerDown}
    >
      <span class="lab-split__grip">
        <i></i>
        <i></i>
        <i></i>
      </span>
    </div>
  );
}
