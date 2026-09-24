/**
 * The lab shell's column splitter — the draggable seam between `.lab-pane--l` and `.lab-pane--r`
 * that `/d2`, `/mermaid` and `/viz` all share, alongside `labshell.css`.
 *
 * The width is a percentage of the panes box, clamped to the same travel the problem page uses,
 * and persisted on RELEASE under the caller's key — never at pointer rate. Each lab keeps its own
 * key and its own default, because a diagram editor and a visualiser want different first splits.
 */
import type { RefObject } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { MAX_LEFT_PCT, MIN_LEFT_PCT } from "../../lib/catalog/pane";
import { get as storageGet, set as storageSet } from "../../lib/storage";

/** A stored or dragged width, kept inside the splitter's travel. Anything unreadable — absent,
 *  empty, not a number — is the caller's default. */
export function clampLeftPct(value: number, fallback: number): number {
  if (!Number.isFinite(value) || value <= 0) return fallback;
  return Math.min(Math.max(value, MIN_LEFT_PCT), MAX_LEFT_PCT);
}

export interface LabSplitter {
  /** The left pane's width, in percent. */
  leftPct: number;
  /** Goes on `.lab-panes` — the box a drag is measured against. */
  panes: RefObject<HTMLDivElement>;
  /** Goes on the seam's `pointerdown`. */
  startDrag: (event: PointerEvent) => void;
}

export function useLabSplitter(storageKey: string, defaultPct: number): LabSplitter {
  const [leftPct, setLeftPct] = useState(() => clampLeftPct(Number(storageGet(storageKey)), defaultPct));
  const panes = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  /** The width as of the last move, for the release handler, which is installed once. */
  const latest = useRef(leftPct);
  latest.current = leftPct;

  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      const box = panes.current?.getBoundingClientRect();
      if (!dragging.current || box == null || box.width <= 0) return;
      setLeftPct(clampLeftPct(((event.clientX - box.left) / box.width) * 100, defaultPct));
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
  }, [storageKey, defaultPct]);

  const startDrag = (event: PointerEvent) => {
    event.preventDefault();
    dragging.current = true;
    document.body.style.cursor = "col-resize";
  };

  return { leftPct, panes, startDrag };
}

/** The seam itself. `label` names what dragging resizes, for a screen reader. */
export function LabSplit({ label, onPointerDown }: { label: string; onPointerDown: (event: PointerEvent) => void }) {
  return (
    <div
      class="lab-split"
      role="separator"
      aria-orientation="vertical"
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
