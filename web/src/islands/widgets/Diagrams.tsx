/**
 * Authored-diagram hydration (port of client/src/catalog/view/diagrams.rs — oracle:
 * `DiagramBlocks` + `MermaidView`/`D2View` + `DiagramZoom`): `.mermaid-block` AND
 * `.d2-block`/`.d2-slideshow` placeholders carry their RAW SOURCE and render through the lazy
 * `@diagram` island (now `lib/islands/diagram/`, A09) on the CLIENT at mount — each diagram
 * renders in its own task (concurrent), so a lesson's prose never waits on a sequential layout.
 * Every rendered figure gets the Enlarge affordance → the near-fullscreen zoom overlay (wheel
 * zoom · drag pan · − ⟲ + controls). House rule: the diagram chrome — Enlarge on the card AND
 * Close in the overlay — sits top-LEFT (LikeC4 owns top-right, see C4Embed.tsx).
 */
import { render, h } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

function decodedAttr(element: Element, name: string): string | null {
  const raw = element.getAttribute(name);
  if (raw == null) return null;
  try {
    return decodeURIComponent(raw);
  } catch {
    return null;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

// ─────────────────────────────────────────────────────────────────────────────
// DISCOVERY — every placeholder carries its raw source; the card renders it lazily.
// ─────────────────────────────────────────────────────────────────────────────

export function hydrateDiagrams(root: ParentNode): number {
  let count = 0;
  for (const element of root.querySelectorAll("div.mermaid-block")) {
    const source = decodedAttr(element, "data-source");
    if (source == null) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(MermaidCard, { source }), host);
    count += 1;
  }
  for (const element of root.querySelectorAll("div.d2-block")) {
    const source = decodedAttr(element, "data-source");
    if (source == null) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(D2Card, { source }), host);
    count += 1;
  }
  for (const element of root.querySelectorAll("div.d2-slideshow")) {
    const raw = decodedAttr(element, "data-slides");
    let slides: string[] | null = null;
    if (raw != null) {
      try {
        const parsed: unknown = JSON.parse(raw);
        if (Array.isArray(parsed) && parsed.length > 0 && parsed.every((s) => typeof s === "string")) {
          slides = parsed as string[];
        }
      } catch {
        slides = null;
      }
    }
    if (!slides) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(D2Slideshow, { slides }), host);
    count += 1;
  }
  return count;
}

// ─────────────────────────────────────────────────────────────────────────────
// CARDS
// Every diagram sits on a FIXED-LIGHT card (the authored palettes assume light), with the
// Enlarge pill revealed once the figure has rendered.
// ─────────────────────────────────────────────────────────────────────────────

/** A ```mermaid fence: source → SVG via the lazy island; a malformed diagram becomes the loud
 *  error card with the raw source to fix — never a blank figure. */
function MermaidCard({ source }: { source: string }) {
  const figureRef = useRef<HTMLDivElement>(null);
  const [svgHtml, setSvgHtml] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const ran = useRef(false);

  useEffect(() => {
    if (ran.current) return;
    ran.current = true;
    const node = figureRef.current;
    if (!node) return;
    void (async () => {
      try {
        const { renderMermaidInto } = await import("../../lib/islands/diagram/mermaid");
        await renderMermaidInto(node, source);
        setSvgHtml(node.innerHTML);
      } catch (error) {
        setFailed(errorMessage(error));
      }
    })();
  }, []);

  return (
    <>
      {failed != null && (
        <div class="diagram-error">
          {`Mermaid diagram failed — ${failed}.`}
          <details>
            <summary>diagram source</summary>
            <pre>{source}</pre>
          </details>
        </div>
      )}
      <div class={failed != null ? "diagram not-prose hidden" : "diagram not-prose"}>
        <ZoomAffordance svgHtml={svgHtml} />
        <div class="diagram__figure" ref={figureRef}></div>
      </div>
    </>
  );
}

/** A single ```d2 fence: raw source → SVG via the lazy `@diagram` island, rendered on the CLIENT
 *  at mount (each diagram its own task — concurrent, and off the parse-time path, so the
 *  multi-MB d2 WASM never blocks prose). Mirrors `MermaidCard`. */
function D2Card({ source }: { source: string }) {
  const figureRef = useRef<HTMLDivElement>(null);
  const [svgHtml, setSvgHtml] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const ran = useRef(false);

  useEffect(() => {
    if (ran.current) return;
    ran.current = true;
    void (async () => {
      try {
        const { renderD2Source } = await import("../../lib/islands/diagram/d2");
        const svg = await renderD2Source(source);
        if (figureRef.current) figureRef.current.innerHTML = svg;
        setSvgHtml(svg);
      } catch (error) {
        setFailed(errorMessage(error));
      }
    })();
  }, []);

  return (
    <>
      {failed != null && (
        <div class="diagram-error">
          {`D2 diagram failed — ${failed}.`}
          <details>
            <summary>diagram source</summary>
            <pre>{source}</pre>
          </details>
        </div>
      )}
      <div class={failed != null ? "diagram not-prose hidden" : "diagram not-prose"}>
        <ZoomAffordance svgHtml={svgHtml} />
        <div class="diagram__figure" ref={figureRef}></div>
      </div>
    </>
  );
}

/** A run of adjacent d2 fences: one figure + the step transport (‹ i / n ›). Each slide's SVG
 *  renders from source via the lazy island the first time its step is shown, then is memoized
 *  per index so stepping back is instant. */
function D2Slideshow({ slides }: { slides: string[] }) {
  const count = slides.length;
  const [idx, setIdx] = useState(0);
  const [svgHtml, setSvgHtml] = useState<string | null>(null);
  const [bump, setBump] = useState(0);
  const figureRef = useRef<HTMLDivElement>(null);
  const rendered = useRef<(string | null)[]>(new Array(count).fill(null) as (string | null)[]);

  useEffect(() => {
    const i = Math.min(idx, count - 1);
    const node = figureRef.current;
    if (!node) return;
    const cached = rendered.current[i];
    if (cached != null) {
      node.innerHTML = cached;
      setSvgHtml(cached);
      return;
    }
    void (async () => {
      try {
        const { renderD2Source } = await import("../../lib/islands/diagram/d2");
        const svg = await renderD2Source(slides[i]!);
        rendered.current[i] = svg;
        setBump((b) => b + 1); // re-run this effect to paint the freshly-cached slide
      } catch {
        // A malformed slide fails quietly here (oracle parity) — the slideshow simply keeps
        // showing whatever it last had; a lone bad fence would otherwise be authored as `.d2-block`.
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idx, bump]);

  return (
    <div class="diagram diagram--slides not-prose">
      <ZoomAffordance svgHtml={svgHtml} />
      <div class="diagram__figure" ref={figureRef}></div>
      <div class="transport">
        <button class="transport__btn" title="Previous" onClick={() => setIdx((i) => Math.max(i - 1, 0))}>
          ‹
        </button>
        <span class="transport__label">{`${idx + 1} / ${count}`}</span>
        <button class="transport__btn" title="Next" onClick={() => setIdx((i) => Math.min(i + 1, count - 1))}>
          ›
        </button>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE ZOOM OVERLAY
// Near-fullscreen light card over a scrim; wheel zoom, drag pan, − ⟲ + controls. Enlarge (card)
// and Close (overlay) both live top-LEFT — the house corner (LikeC4 owns top-right).
// ─────────────────────────────────────────────────────────────────────────────

const ICON_MAXIMIZE = (
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

function ZoomAffordance({ svgHtml }: { svgHtml: string | null }) {
  const [open, setOpen] = useState(false);
  if (svgHtml == null) return null;
  return (
    <>
      <button class="diagram__zoom modal-btn" aria-label="Enlarge diagram" onClick={() => setOpen(true)}>
        {ICON_MAXIMIZE}
        <span>Enlarge</span>
      </button>
      {open && <ZoomOverlay svg={svgHtml} onClose={() => setOpen(false)} />}
    </>
  );
}

function ZoomOverlay({ svg, onClose }: { svg: string; onClose: () => void }) {
  const [scale, setScale] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const grip = useRef<{ x: number; y: number } | null>(null);
  const figureRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (figureRef.current) figureRef.current.innerHTML = svg;
  }, [svg]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
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

  const zoomBy = (factor: number) => setScale((s) => Math.min(Math.max(s * factor, 0.25), 4));

  return (
    <div class="diagram-zoom-scrim" onClick={onClose}>
      <div class="diagram-zoom diagram-zoom--paper" onClick={(event) => event.stopPropagation()}>
        <button class="diagram-zoom__close modal-btn" aria-label="Close" onClick={onClose}>
          {ICON_CLOSE}
          <span>Close</span>
        </button>
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
            <div
              class="diagram-zoom__figure"
              style={`transform: translate(${pan.x.toFixed(1)}px, ${pan.y.toFixed(1)}px) scale(${scale.toFixed(3)})`}
              ref={figureRef}
            ></div>
          </div>
        </div>
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
        </div>
      </div>
    </div>
  );
}
