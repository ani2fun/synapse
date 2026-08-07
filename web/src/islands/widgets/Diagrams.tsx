/**
 * Authored-diagram hydration. A `.mermaid-block` / `.d2-block` / `.d2-slideshow` placeholder
 * carries its RAW SOURCE and renders through `lib/islands/diagram/` on the CLIENT; a `.d2-block`
 * marked `data-prerendered` instead arrives from the server with its figure already in place and
 * is only adopted here. Client-side d2 compiles on approach and one at a time — the renderer
 * holds a single worker — so the figure a reader is looking at is never queued behind the rest
 * of the document. Every rendered figure gets the Enlarge affordance → the near-fullscreen zoom
 * overlay (wheel zoom · drag pan · − ⟲ + controls). House rule: the diagram chrome — Enlarge on
 * the card AND Close in the overlay — sits top-LEFT (LikeC4 owns top-right, see C4Embed.tsx).
 *
 * `.frame-slideshow` is the one family that renders no source: it carries the URLs of a run of
 * authored stills (lib/markdown/frameRun.ts) and steps through them one <img> at a time.
 */
import { render, h, type ComponentChildren } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

// Statically imported: this module is small and holds the salt + queue only. The multi-MB WASM
// it renders through stays behind a dynamic `import()` inside it, so nothing heavy lands here.
import { d2Salt, renderD2Source } from "../../lib/islands/diagram/d2";
import * as log from "../../lib/log";
import { watchNear } from "../workbench/lazy";

/**
 * Rendered d2 SVGs, keyed by salt (a fingerprint of the source — see `d2Salt`).
 *
 * The editorial pane and the authoring preview both re-render their markdown and re-hydrate over
 * it, repeatedly, without the source having changed; without this every tab switch and every
 * debounced keystroke would pay a fresh compile. In memory only — a page navigation is meant to
 * clear it, and the document itself bounds how much can accumulate.
 */
const svgCache = new Map<string, string>();

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

/** A JSON string array off a data attribute — a d2 run's sources, a frame run's URLs. */
function decodedStringArray(element: Element, name: string): string[] | null {
  const raw = decodedAttr(element, name);
  if (raw == null) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    const ok =
      Array.isArray(parsed) && parsed.length > 0 && parsed.every((item) => typeof item === "string");
    return ok ? (parsed as string[]) : null;
  } catch {
    return null;
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// DISCOVERY — every placeholder carries its raw source; the card renders it lazily.
// ─────────────────────────────────────────────────────────────────────────────

export function hydrateDiagrams(root: ParentNode): number {
  let count = 0;
  // Salts are unique per DOCUMENT, and this pass is the only place that sees the whole document,
  // so the tally lives here. A re-hydration over unchanged markup mints the same salts again,
  // which is exactly what lets `svgCache` hit on the second pass.
  const seen = new Map<string, number>();
  for (const element of root.querySelectorAll("div.mermaid-block")) {
    const source = decodedAttr(element, "data-source");
    if (source == null) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(MermaidCard, { source }), host);
    count += 1;
  }
  for (const element of root.querySelectorAll("div.d2-block")) {
    const host = element as HTMLElement;
    // A server-rendered block already holds its figure. Lift the SVG out BEFORE clearing the
    // host — `replaceChildren` would otherwise discard the very thing SSR did the work for —
    // and hand it straight to the card, which then only wires up Enlarge.
    const prerendered =
      host.dataset.prerendered != null
        ? (host.querySelector(".diagram__figure")?.innerHTML ?? null)
        : null;
    const source = decodedAttr(element, "data-source");
    if (prerendered == null && source == null) continue;
    host.replaceChildren();
    // Pre-rendered figures never compile, so they need no salt and must not consume one.
    const salt = prerendered != null ? "" : d2Salt(source!, seen);
    render(h(D2Card, { source: source ?? "", salt, host, prerendered }), host);
    count += 1;
  }
  for (const element of root.querySelectorAll("div.d2-slideshow")) {
    const slides = decodedStringArray(element, "data-slides");
    if (!slides) continue;
    const host = element as HTMLElement;
    // The server draws the FIRST slide, since that is the one the transport paints at mount.
    // Lifting it into the cache before clearing the host is the whole handover: the component's
    // ordinary cache lookup then finds it and never reaches for the engine.
    const first =
      host.dataset.prerendered != null
        ? (host.querySelector(".diagram__figure")?.innerHTML ?? null)
        : null;
    host.replaceChildren();
    const salts = slides.map((slide) => d2Salt(slide, seen));
    if (first != null) svgCache.set(salts[0]!, first);
    render(h(D2Slideshow, { slides, salts }), host);
    count += 1;
  }
  for (const element of root.querySelectorAll("div.frame-slideshow")) {
    const frames = decodedStringArray(element, "data-frames");
    const caption = decodedAttr(element, "data-caption");
    if (!frames || caption == null) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(FrameSlideshow, { frames, caption }), host);
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

/**
 * A single ```d2 fence. Three ways its figure arrives, cheapest first: already server-rendered
 * (`prerendered` — the card only wires up Enlarge), already compiled this session (`svgCache`),
 * or compiled here on approach.
 */
function D2Card({
  source,
  salt,
  host,
  prerendered,
}: {
  source: string;
  salt: string;
  host: HTMLElement;
  prerendered: string | null;
}) {
  const figureRef = useRef<HTMLDivElement>(null);
  const [svgHtml, setSvgHtml] = useState<string | null>(prerendered ?? svgCache.get(salt) ?? null);
  const [failed, setFailed] = useState<string | null>(null);
  const [near, setNear] = useState(false);
  const ran = useRef(false);

  // Paint whatever is known — at mount for a server-rendered or cached figure, later for a
  // freshly compiled one.
  useEffect(() => {
    if (svgHtml != null && figureRef.current) figureRef.current.innerHTML = svgHtml;
  }, [svgHtml]);

  // Nothing compiles until the block nears the viewport. One worker serves every diagram on the
  // page in turn, so a reader's first figure must not queue behind the last one in the document.
  // A figure that arrived with the HTML never arms this at all.
  useEffect(() => {
    if (svgHtml != null) return;
    const watch = watchNear(host, (isNear) => {
      if (isNear) setNear(true);
    });
    return () => watch?.disconnect();
  }, []);

  useEffect(() => {
    if (!near || ran.current || svgHtml != null) return;
    ran.current = true;
    void (async () => {
      try {
        const svg = await renderD2Source(source, salt);
        svgCache.set(salt, svg);
        setSvgHtml(svg);
      } catch (error) {
        setFailed(errorMessage(error));
      }
    })();
  }, [near]);

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

/** A run of adjacent d2 fences: one figure + the step transport (‹ i / n ›). A slide compiles the
 *  first time its step is shown and lands in `svgCache`, so stepping back is instant — and so is
 *  re-opening the same slideshow after the pane around it re-renders. */
function D2Slideshow({ slides, salts }: { slides: string[]; salts: string[] }) {
  const count = slides.length;
  const [idx, setIdx] = useState(0);
  const [svgHtml, setSvgHtml] = useState<string | null>(null);
  const [bump, setBump] = useState(0);
  const figureRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const i = Math.min(idx, count - 1);
    const node = figureRef.current;
    if (!node) return;
    const cached = svgCache.get(salts[i]!);
    if (cached != null) {
      node.innerHTML = cached;
      setSvgHtml(cached);
      return;
    }
    void (async () => {
      try {
        const svg = await renderD2Source(slides[i]!, salts[i]!);
        svgCache.set(salts[i]!, svg);
        setBump((b) => b + 1); // re-run this effect to paint the freshly-cached slide
      } catch {
        // A malformed slide fails quietly here — the slideshow simply keeps showing whatever it
        // last had; a lone bad fence would otherwise be authored as `.d2-block`.
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idx, bump]);

  const step = (delta: number) => setIdx((i) => Math.min(Math.max(i + delta, 0), count - 1));

  // Keydown is bound to the CARD, never to the document: `MarkdownPane` re-renders editorial
  // markdown and re-hydrates diagrams without ever unmounting the old hosts, so a document-level
  // listener would survive every tab switch.
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "ArrowLeft") step(-1);
    else if (event.key === "ArrowRight") step(1);
    else return;
    event.preventDefault();
  };

  return (
    <div
      class="diagram diagram--slides not-prose"
      role="group"
      aria-roledescription="step-through diagram"
      aria-label={`Diagram, ${count} steps`}
      tabIndex={0}
      onKeyDown={onKeyDown}
    >
      <ZoomAffordance svgHtml={svgHtml} />
      <div class="diagram__figure" ref={figureRef}></div>
      <div class="transport">
        <button
          class="transport__btn"
          aria-label="Previous slide"
          title="Previous slide"
          disabled={idx === 0}
          onClick={() => step(-1)}
        >
          ‹
        </button>
        {/* The counter is the live region here, unlike the frame slideshow — that one announces
            through its <img> alt, and this figure is an injected SVG whose text would be read
            wholesale. Hiding this span would leave a stepping reader with no position at all. */}
        <span class="transport__label" aria-live="polite">{`${idx + 1} / ${count}`}</span>
        <button
          class="transport__btn"
          aria-label="Next slide"
          title="Next slide"
          disabled={idx === count - 1}
          onClick={() => step(1)}
        >
          ›
        </button>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// FRAME SLIDESHOWS — an animation authored as consecutive images
// A `.frame-slideshow` placeholder carries one run's frame URLs (lib/markdown/frameRun.ts). ONE
// <img> shows one frame and the neighbours are warmed a step ahead. Holding the loaded elements
// the way D2Slideshow memoises its SVG strings is the trap here, not the optimisation: a 95-frame
// run at 1450×1024 is ~9.5 MB of PNG and ~565 MB of decoded bitmap. The browser's HTTP cache is
// the store (/media ships max-age=3600); this component keeps only which indices it has asked for.
// ─────────────────────────────────────────────────────────────────────────────

const FRAME_STEP_MS = 500;
const PRELOAD_AT_REST = 1; // neighbours warmed before the reader touches anything
const PRELOAD_STEPPING = 2; // …and after they do

function FrameSlideshow({ frames, caption }: { frames: string[]; caption: string }) {
  const total = frames.length;
  // Two indices, and the split is what removes both the flash and the height jump: `shown` is
  // bound to the <img> and is only ever an index that has decoded, while `wanted` is what the
  // reader just asked for and drives the label and the scrubber. The picture holds the last good
  // frame (dimmed) instead of blanking while the next one arrives.
  const [shown, setShown] = useState(0);
  const [wanted, setWanted] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [ready, setReady] = useState(false);
  const requested = useRef(new Set<number>([0]));
  const stepped = useRef(false);
  const imgRef = useRef<HTMLImageElement>(null);

  const altFor = (index: number) => `${caption} — frame ${index + 1} of ${total}`;

  const clamp = (index: number) => Math.min(Math.max(index, 0), total - 1);

  const goTo = (index: number) => {
    stepped.current = true;
    setWanted(clamp(index));
  };

  // Stepping reads the CURRENT index through the updater rather than the one this render closed
  // over, so two clicks inside one frame advance twice instead of landing on the same frame.
  const step = (delta: number) => {
    setPlaying(false);
    stepped.current = true;
    setWanted((current) => clamp(current + delta));
  };

  // Commit only once the next frame can paint. The signal is `load`, NOT `decode()`: on a
  // detached Image the decode promise can simply never settle, which strands the transport on
  // one frame forever. `error` settles too — a broken URL must cost one dud frame, not the widget.
  useEffect(() => {
    if (wanted === shown) return;
    let live = true;
    const settle = () => {
      if (live) setShown(wanted);
    };
    const probe = new Image();
    probe.onload = settle;
    probe.onerror = settle;
    probe.src = frames[wanted]!;
    requested.current.add(wanted);
    if (probe.complete) settle(); // already cached — the event may have fired before we listened
    return () => {
      live = false;
    };
  }, [wanted, shown, frames]);

  // A frame served straight from cache can finish before the listener is attached, and then the
  // `load` that gates Enlarge and the preloads never arrives. Checking `complete` each render is
  // the cheap way to notice a first paint whose event we missed.
  useEffect(() => {
    const image = imgRef.current;
    if (!ready && image?.complete && image.naturalWidth > 0) setReady(true);
  });

  // Warm the neighbours, but never before the frame the reader is actually looking at has landed —
  // `hydrateDiagrams` runs over the whole document at DOMContentLoaded, and a lesson carrying ten
  // runs would otherwise open ten connections for pictures nobody has scrolled to.
  useEffect(() => {
    if (!ready) return;
    const reach = stepped.current ? PRELOAD_STEPPING : PRELOAD_AT_REST;
    for (let offset = 1; offset <= reach; offset += 1) {
      for (const index of [shown + offset, shown - offset]) {
        if (index < 0 || index >= total || requested.current.has(index)) continue;
        requested.current.add(index);
        new Image().src = frames[index]!; // fire and forget: the HTTP cache is the store
      }
    }
  }, [shown, ready, total, frames]);

  // Autoplay advances off `shown`, not `wanted`, so a slow network slows the animation down
  // instead of skipping frames the reader never sees.
  useEffect(() => {
    if (!playing) return;
    if (shown >= total - 1) {
      setPlaying(false);
      return;
    }
    const timer = setTimeout(() => goTo(shown + 1), FRAME_STEP_MS);
    return () => clearTimeout(timer);
  }, [playing, shown, total]);

  useEffect(() => {
    log.debug(`frame slideshow “${caption}” → ${shown + 1}/${total}`);
  }, [shown]);

  const togglePlay = () => {
    if (!playing && shown >= total - 1) goTo(0); // replay from the top rather than sitting at the end
    setPlaying((on) => !on);
  };

  // Keydown is bound to the CARD, never to the document: `MarkdownPane` re-renders editorial
  // markdown and re-hydrates diagrams without ever unmounting the old hosts, so a document-level
  // listener would survive every tab switch.
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "ArrowLeft") step(-1);
    else if (event.key === "ArrowRight") step(1);
    else return;
    event.preventDefault();
  };

  return (
    <div
      class="diagram diagram--slides diagram--frames not-prose"
      role="group"
      aria-roledescription="frame-by-frame diagram"
      aria-label={caption}
      tabIndex={0}
      onKeyDown={onKeyDown}
    >
      {ready && (
        <ZoomAffordance>
          <img src={frames[shown]} alt={altFor(shown)} />
        </ZoomAffordance>
      )}
      {/* The live region announces each frame through the img's alt, which carries the position. */}
      <div class="diagram__figure" aria-live="polite" aria-atomic="true">
        <img
          ref={imgRef}
          class={wanted === shown ? "frames__img" : "frames__img frames__img--pending"}
          src={frames[shown]}
          alt={altFor(shown)}
          loading="lazy"
          decoding="async"
          onLoad={() => setReady(true)}
        />
      </div>
      <p class="diagram__caption">{caption}</p>
      <div class="transport">
        <button
          class="transport__btn"
          aria-label="Previous frame"
          title="Previous frame"
          disabled={wanted === 0}
          onClick={() => step(-1)}
        >
          ‹
        </button>
        <button
          class={playing ? "transport__btn transport__btn--play" : "transport__btn"}
          aria-label={playing ? "Pause" : "Play"}
          title={playing ? "Pause" : "Play"}
          onClick={togglePlay}
        >
          {playing ? "⏸" : "▶"}
        </button>
        <input
          class="transport__scrubber"
          type="range"
          min={0}
          max={total - 1}
          value={wanted}
          aria-label={`Frame ${wanted + 1} of ${total}`}
          onInput={(event) => {
            setPlaying(false);
            goTo(Number((event.currentTarget as HTMLInputElement).value));
          }}
        />
        <button
          class="transport__btn"
          aria-label="Next frame"
          title="Next frame"
          disabled={wanted === total - 1}
          onClick={() => step(1)}
        >
          ›
        </button>
        <span class="transport__label" aria-hidden="true">{`${wanted + 1} / ${total}`}</span>
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

/**
 * The Enlarge pill and the overlay behind it. A renderer passes its SVG as a string; a figure
 * built from real elements (the frame slideshow) passes them as children instead. Either way the
 * pill only exists once there is something to enlarge.
 */
function ZoomAffordance({ svgHtml, children }: { svgHtml?: string | null; children?: ComponentChildren }) {
  const [open, setOpen] = useState(false);
  if (svgHtml == null && children == null) return null;
  return (
    <>
      <button class="diagram__zoom modal-btn" aria-label="Enlarge diagram" onClick={() => setOpen(true)}>
        {ICON_MAXIMIZE}
        <span>Enlarge</span>
      </button>
      {open && (
        <ZoomOverlay svg={svgHtml ?? undefined} onClose={() => setOpen(false)}>
          {children}
        </ZoomOverlay>
      )}
    </>
  );
}

function ZoomOverlay({
  svg,
  children,
  onClose,
}: {
  svg?: string;
  children?: ComponentChildren;
  onClose: () => void;
}) {
  const [scale, setScale] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const grip = useRef<{ x: number; y: number } | null>(null);

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
  // The pan/zoom target. An SVG string lands on this element itself rather than a wrapper, so
  // `.diagram-zoom__figure svg` keeps binding and the flex centring is unchanged.
  const transform = `transform: translate(${pan.x.toFixed(1)}px, ${pan.y.toFixed(1)}px) scale(${scale.toFixed(3)})`;

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
            {svg != null ? (
              <div
                class="diagram-zoom__figure"
                style={transform}
                dangerouslySetInnerHTML={{ __html: svg }}
              ></div>
            ) : (
              <div class="diagram-zoom__figure" style={transform}>
                {children}
              </div>
            )}
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
