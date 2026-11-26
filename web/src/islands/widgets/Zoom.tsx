/**
 * The Enlarge affordance and the near-fullscreen overlay behind it — shared by every authored
 * figure: mermaid, d2, a d2 walkthrough, the frame slideshow.
 *
 * House rule: diagram chrome never sits ON the figure. The card carries a header bar above the
 * paper — a kind label on the left, Edit and Enlarge on the right, the shape of a code card's tab
 * bar — and the overlay carries the same bar above its viewport: label left, the viewer's own
 * `chrome` in the middle, zoom controls and Close on the right. A floating pill hides whatever the
 * figure draws in that corner, and a figure's corner is where titles live.
 *
 * The overlay is a real dialog: it takes focus, traps Tab, and gives focus back on close. That
 * matters more than it used to — it now holds breadcrumbs, a menu and navigation buttons, so a
 * keyboard reader who tabs out of it is stranded behind a scrim they cannot see past.
 */
import { type ComponentChildren, h } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

export const ICON_MAXIMIZE = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M15 3h6v6"></path>
    <path d="M9 21H3v-6"></path>
    <path d="m21 3-7 7"></path>
    <path d="m3 21 7-7"></path>
  </svg>
);

const ICON_CLOSE = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M18 6 6 18"></path>
    <path d="m6 6 12 12"></path>
  </svg>
);

const FOCUSABLE =
  'a[href], button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])';

export interface ZoomProps {
  /** What the figure is — `D2`, `Mermaid`, `Walkthrough` — the bar's left-hand label. */
  kind: string;
  /** A second pill beside Enlarge — the d2 figures' "Edit". Rendered in the bar's action group. */
  edit?: ComponentChildren;
  /** A renderer's SVG, as a string. */
  svgHtml?: string | null;
  /** A figure built from real elements instead (the frame slideshow). */
  children?: ComponentChildren;
  /** A viewer's own controls, rendered top-centre inside the overlay. */
  chrome?: ComponentChildren;
  /** Clicks inside the enlarged figure — how a walkthrough stays navigable once enlarged. */
  onFigureClick?: (event: MouseEvent) => void;
  /** Announced on the overlay, so it is not just "dialog". */
  label?: string;
}

/**
 * The card's header bar and the overlay behind its Enlarge button. The bar renders from the first
 * paint, so the figure never shifts down when it arrives; Enlarge stays disabled until there is
 * something to enlarge.
 */
export function ZoomAffordance({ kind, svgHtml, children, chrome, edit, onFigureClick, label }: ZoomProps) {
  const [open, setOpen] = useState(false);
  const ready = svgHtml != null || children != null;
  return (
    <>
      <div class="diagram__bar">
        <span class="diagram__label">{kind}</span>
        <div class="diagram__actions">
          {edit}
          <button
            class="diagram__zoom modal-btn"
            aria-label="Enlarge diagram"
            disabled={!ready}
            onClick={() => setOpen(true)}
          >
            {ICON_MAXIMIZE}
            <span>Enlarge</span>
          </button>
        </div>
      </div>
      {open && ready && (
        <ZoomOverlay
          svg={svgHtml ?? undefined}
          chrome={chrome}
          onFigureClick={onFigureClick}
          label={label ?? kind}
          onClose={() => setOpen(false)}
        >
          {children}
        </ZoomOverlay>
      )}
    </>
  );
}

/**
 * What a figure shows while its renderer is still working.
 *
 * An empty card is indistinguishable from a broken one, and the wait here is long enough to need
 * saying: a figure nobody drew ahead of time compiles in the browser, behind a ~5.9 MB gz engine
 * that boots once per page. Silence for that long reads as failure.
 *
 * It overlays `.diagram` rather than sitting inside `.diagram__figure`, because a renderer writes
 * that box's `innerHTML` imperatively and would tear this out from under Preact. Fixed colours,
 * like everything else on the card, which is fixed-light on both themes.
 */
export function DiagramPending({ label = "Drawing diagram" }: { label?: string }) {
  return (
    <div class="diagram__pending" role="status" aria-live="polite">
      <span class="diagram__pending-dot" aria-hidden="true"></span>
      <span>{`${label}…`}</span>
    </div>
  );
}

export function ZoomOverlay({
  svg,
  children,
  chrome,
  onFigureClick,
  label,
  onClose,
}: {
  svg?: string;
  children?: ComponentChildren;
  chrome?: ComponentChildren;
  onFigureClick?: (event: MouseEvent) => void;
  label?: string;
  onClose: () => void;
}) {
  const [scale, setScale] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const grip = useRef<{ x: number; y: number } | null>(null);
  const panel = useRef<HTMLDivElement>(null);

  // Take focus, keep it, and hand it back. Without the restore, closing the overlay drops a
  // keyboard reader at the top of the document rather than at the figure they opened.
  useEffect(() => {
    const returnTo = document.activeElement as HTMLElement | null;
    panel.current?.focus();
    return () => returnTo?.focus?.();
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
        return;
      }
      if (event.key !== "Tab" || panel.current == null) return;
      const stops = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
        (node) => node.offsetParent !== null,
      );
      if (stops.length === 0) return;
      const first = stops[0]!;
      const last = stops[stops.length - 1]!;
      const at = document.activeElement;
      // Wrap at both ends, and treat the panel itself as "before the first stop".
      if (event.shiftKey && (at === first || at === panel.current)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && at === last) {
        event.preventDefault();
        first.focus();
      }
    };
    const onMove = (event: PointerEvent) => {
      const last = grip.current;
      if (!last) return;
      const dx = event.clientX - last.x;
      const dy = event.clientY - last.y;
      setPan((p) => ({ x: p.x + dx, y: p.y + dy }));
      grip.current = { x: event.clientX, y: event.clientY };
    };
    const onUp = () => {
      grip.current = null;
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [onClose]);

  // A new figure is a new subject: keeping the old pan would leave a reader looking at empty
  // canvas after drilling into a board laid out somewhere else entirely.
  useEffect(() => {
    setScale(1);
    setPan({ x: 0, y: 0 });
  }, [svg]);

  const zoomBy = (factor: number) => setScale((s) => Math.min(Math.max(s * factor, 0.25), 4));
  // The pan/zoom target. An SVG string lands on this element itself rather than a wrapper, so
  // `.diagram-zoom__figure svg` keeps binding and the flex centring is unchanged.
  const transform = `transform: translate(${pan.x.toFixed(1)}px, ${pan.y.toFixed(1)}px) scale(${scale.toFixed(3)})`;

  return (
    <div class="diagram-zoom-scrim" onClick={onClose}>
      <div
        class="diagram-zoom diagram-zoom--paper"
        role="dialog"
        aria-modal="true"
        aria-label={label ?? "Enlarged diagram"}
        tabIndex={-1}
        ref={panel}
        onClick={(event) => event.stopPropagation()}
      >
        <div class="diagram-zoom__bar">
          <span class="diagram__label">{label ?? "Diagram"}</span>
          <div class="diagram-zoom__chrome">{chrome}</div>
          <div class="diagram-zoom__controls">
            <button class="diagram-zoom__ctl" aria-label="Zoom out" onClick={() => zoomBy(1 / 1.25)}>
              −
            </button>
            <span class="diagram-zoom__level">{`${Math.round(scale * 100)}%`}</span>
            <button class="diagram-zoom__ctl" aria-label="Zoom in" onClick={() => zoomBy(1.25)}>
              +
            </button>
            <button
              class="diagram-zoom__ctl"
              aria-label="Reset zoom"
              onClick={() => {
                setScale(1);
                setPan({ x: 0, y: 0 });
              }}
            >
              ⟲
            </button>
            <ZoomClose onClose={onClose} />
          </div>
        </div>
        <div class="diagram-zoom__zoomable">
          <div
            class="diagram-zoom__viewport"
            onPointerDown={(event) => {
              event.preventDefault();
              grip.current = { x: event.clientX, y: event.clientY };
            }}
            onWheel={(event) => {
              event.preventDefault();
              zoomBy(event.deltaY < 0 ? 1.12 : 1 / 1.12);
            }}
          >
            {svg != null ? (
              <div
                class="diagram-zoom__figure"
                style={transform}
                onClick={onFigureClick as never}
                dangerouslySetInnerHTML={{ __html: svg }}
              ></div>
            ) : (
              <div class="diagram-zoom__figure" style={transform} onClick={onFigureClick as never}>
                {children}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

/** The overlay's Close pill — shared with the simulator's overlay so both bars end the same way. */
export function ZoomClose({ onClose }: { onClose: () => void }) {
  return (
    <button class="diagram-zoom__close modal-btn" aria-label="Close" onClick={onClose}>
      {ICON_CLOSE}
      <span>Close</span>
    </button>
  );
}
