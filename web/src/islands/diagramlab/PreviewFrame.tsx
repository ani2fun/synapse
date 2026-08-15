/**
 * The right pane's chrome: what the diagram is doing, how big to draw it, and where the error is.
 *
 * Everything here is true of any diagram language — a compile state, a zoom, an Enlarge, and a
 * problem with a line to jump to. The figure itself is the caller's, passed as children, because
 * that is the only part a walkthrough and a flowchart disagree about.
 *
 * The compile state is a single pill that changes colour rather than three different layouts, so
 * the toolbar never reflows between "drawing" and "done".
 */
import { type ComponentChildren, h } from "preact";
import { useState } from "preact/hooks";

import { Icon } from "./icons";
import { ZoomOverlay } from "../widgets/Zoom";

/** What a renderer objected to, in the only two terms this pane needs. Both `d2Errors` and
 *  `mermaidErrors` produce something assignable to it. */
export interface DiagramProblem {
  /** 1-based, matching the editor's gutter. Null when the message carried no position. */
  line: number | null;
  message: string;
}

const ZOOM_MIN = 0.5;
const ZOOM_MAX = 2;
const ZOOM_STEP = 0.1;

export function PreviewFrame({
  busy,
  problem,
  ready,
  label,
  children,
  overlaySvg,
  overlayChrome,
  onFigureClick,
  onKeyDown,
  onGoToLine,
}: {
  busy: boolean;
  problem: DiagramProblem | null;
  /** Whether there is a figure at all yet — what gates Enlarge and replaces the canvas with a
   *  placeholder line. */
  ready: boolean;
  /** Announced on the overlay, so it is not just "dialog". */
  label: string;
  children?: ComponentChildren;
  /** The figure as a string, for the overlay. A caller with no SVG to hand passes nothing and
   *  simply gets no Enlarge. */
  overlaySvg?: string | null;
  /** A viewer's own controls, re-skinned inside Enlarge — d2's board bar. */
  overlayChrome?: ComponentChildren;
  onFigureClick?: (event: MouseEvent) => void;
  /** Bound to the canvas, which is the focusable element here — a walk's arrow keys have to work
   *  anywhere in the pane, not only while the board bar holds focus. */
  onKeyDown?: (event: KeyboardEvent) => void;
  onGoToLine: (line: number) => void;
}) {
  const [zoom, setZoom] = useState(1);
  const [enlarged, setEnlarged] = useState(false);
  const round = (value: number) => Number(value.toFixed(2));

  return (
    <>
      <div class="pane-hd">
        <span class="pane-hd__eyebrow">Preview</span>
        <StatusPill busy={busy} problem={problem} />
        <span class="pane-hd__sp"></span>
        <div class="zoom">
          <button
            class="zoom__b"
            aria-label="Zoom out"
            onClick={() => setZoom((z) => Math.max(ZOOM_MIN, round(z - ZOOM_STEP)))}
          >
            <Icon name="minus" size={14} />
          </button>
          <button class="zoom__lvl" title="Reset zoom" onClick={() => setZoom(1)}>
            {`${Math.round(zoom * 100)}%`}
          </button>
          <button
            class="zoom__b"
            aria-label="Zoom in"
            onClick={() => setZoom((z) => Math.min(ZOOM_MAX, round(z + ZOOM_STEP)))}
          >
            <Icon name="plus" size={14} />
          </button>
        </div>
        <button
          class="pane-hd__btn"
          aria-label="Enlarge diagram"
          title="Enlarge"
          disabled={!ready || overlaySvg == null}
          onClick={() => setEnlarged(true)}
        >
          <Icon name="expand" size={15} />
        </button>
      </div>
      {ready ? (
        <div class="pv" tabIndex={0} onKeyDown={onKeyDown}>
          <div class="pv__paper" style={{ transform: `scale(${zoom})` }}>
            {children}
          </div>
          {overlayChrome}
          {/* The reader's own overlay — so enlarging here is the affordance a lesson has, not a
              second implementation of it. */}
          {enlarged && overlaySvg != null && (
            <ZoomOverlay
              svg={overlaySvg}
              chrome={overlayChrome}
              onFigureClick={onFigureClick}
              label={label}
              onClose={() => setEnlarged(false)}
            />
          )}
        </div>
      ) : (
        <div class="pv">
          <p class="pv__empty">{problem == null ? "Drawing…" : "Nothing drawn yet."}</p>
        </div>
      )}
      {problem != null && (
        <div class="ed-err">
          <Icon name="alert" size={15} />
          <span class="ed-err__msg">{problem.message}</span>
          <span class="pane-hd__sp"></span>
          {problem.line != null && (
            <button class="ed-err__go" onClick={() => onGoToLine(problem.line!)}>
              {`Go to line ${problem.line}`}
            </button>
          )}
        </div>
      )}
    </>
  );
}

function StatusPill({ busy, problem }: { busy: boolean; problem: DiagramProblem | null }) {
  const tone = problem != null ? "bad" : busy ? "busy" : "ok";
  const text =
    problem != null
      ? problem.line != null
        ? `Line ${problem.line} · ${problem.message}`
        : problem.message
      : busy
        ? "Drawing…"
        : "Up to date";
  return (
    <span class={`st st--${tone}`} aria-live="polite" title={text}>
      <i class="st__dot"></i>
      {text}
    </span>
  );
}
