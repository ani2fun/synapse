/**
 * Simulator embeds: a `.simulator-block` marker (the ```simulator fence) becomes a same-origin
 * iframe over `/simulators/<name>/` — a self-contained static bundle a content repo ships under
 * `_simulators/<name>/`. A HEAD probe of the bundle's index.html gates the mount, so a missing
 * bundle earns the loud `.diagram-error` card (ADR-S026), never a blank frame. Raw authored
 * `<iframe src="/simulators/…">` gets the same Enlarge chrome (the LikeC4 wrap pattern).
 *
 * The `sandbox` attribute is belt-and-braces, not a trust boundary: the bundle is same-origin
 * first-party content (ADR-S015), so `allow-same-origin` + `allow-scripts` only blocks popups,
 * top navigation and form posts. The real gate is that content repos are first-party.
 */
import { render, h } from "preact";
import { useEffect, useState } from "preact/hooks";

import * as log from "../../lib/log";

const DEFAULT_HEIGHT = 480;
const SANDBOX = "allow-scripts allow-same-origin";

// ─────────────────────────────────────────────────────────────────────────────
// DISCOVERY
// ─────────────────────────────────────────────────────────────────────────────

export function hydrateSimulators(root: ParentNode): number {
  let count = 0;
  for (const host of Array.from(root.querySelectorAll<HTMLElement>("div.simulator-block"))) {
    const name = host.getAttribute("data-name");
    if (!name) continue;
    const height = Number(host.getAttribute("data-height")) || DEFAULT_HEIGHT;
    const title = host.getAttribute("data-title") ?? `${name} simulator`;
    host.replaceChildren();
    render(h(SimulatorCard, { name, height, title }), host);
    count += 1;
  }
  // Raw authored iframes keep their own attributes and gain only the Enlarge chrome. The
  // `.sim-embed` ancestor check keeps a re-run (and the marker path's own iframe) unwrapped.
  for (const frame of Array.from(root.querySelectorAll<HTMLIFrameElement>("iframe[src^='/simulators/']"))) {
    const parent = frame.parentElement;
    const src = frame.getAttribute("src");
    if (!parent || src == null || frame.closest(".sim-embed")) continue;
    const wrap = document.createElement("div");
    wrap.className = "sim-embed not-prose";
    parent.insertBefore(wrap, frame);
    wrap.appendChild(frame); // re-parenting reloads the iframe — accepted, same as the c4 wrap
    const host = document.createElement("div");
    wrap.appendChild(host);
    render(h(ZoomAffordance, { src, title: frame.title || "Simulator" }), host);
    count += 1;
  }
  return count;
}

// ─────────────────────────────────────────────────────────────────────────────
// THE INLINE EMBED: existence probe → iframe (or the loud missing card)
// ─────────────────────────────────────────────────────────────────────────────

function SimulatorCard({ name, height, title }: { name: string; height: number; title: string }) {
  const [state, setState] = useState<"probing" | "ok" | "missing">("probing");
  const src = `/simulators/${name}/`;

  // HEAD is answered body-free by the same route that will serve the iframe, so this settles
  // fast and warms the entry point's cache minute. An onload sniff can't tell a same-origin
  // 404 body from a slow bundle; the probe can.
  useEffect(() => {
    let cancelled = false;
    fetch(`${src}index.html`, { method: "HEAD" })
      .then((res) => {
        if (!cancelled) setState(res.ok ? "ok" : "missing");
      })
      .catch(() => {
        if (!cancelled) setState("missing");
      });
    return () => {
      cancelled = true;
    };
  }, [src]);

  useEffect(() => {
    if (state === "missing") {
      log.warn(`simulator “${name}” is not served — is its repository mounted?`);
    }
  }, [state, name]);

  if (state === "missing") {
    // Neutral wording on purpose: right after boot a satellite's first sync can lag a reload,
    // so "not served" may be transient rather than an authoring mistake.
    return (
      <div class="diagram-error">
        Simulator “{name}” is not being served — expected <code>_simulators/{name}/index.html</code> in a
        mounted content repository.
      </div>
    );
  }
  return (
    <div class="sim-embed not-prose">
      {state === "ok" && (
        <iframe
          src={src}
          title={title}
          loading="lazy"
          sandbox={SANDBOX}
          style={{ height: `${height}px` }}
        ></iframe>
      )}
      {state === "ok" && <ZoomAffordance src={src} title={title} />}
    </div>
  );
}

function ZoomAffordance({ src, title }: { src: string; title: string }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button class="sim-embed__zoom modal-btn" aria-label="Enlarge simulator" onClick={() => setOpen(true)}>
        ⤢ Enlarge
      </button>
      {open && <SimZoom src={src} title={title} onClose={() => setOpen(false)} />}
    </>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE FULLSCREEN ZOOM
// A NEW iframe with the same src fills the modal — moving an iframe reloads it anyway, so a
// fresh instance (state reset included) is the honest trade, same as the C4 zoom.
// ─────────────────────────────────────────────────────────────────────────────

function SimZoom({ src, title, onClose }: { src: string; title: string; onClose: () => void }) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div class="diagram-zoom-scrim" onClick={onClose}>
      <div class="diagram-zoom diagram-zoom--fill" onClick={(event) => event.stopPropagation()}>
        <button class="diagram-zoom__close modal-btn" aria-label="Close" onClick={onClose}>
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M18 6 6 18M6 6l12 12"></path>
          </svg>
          Close
        </button>
        <div class="diagram-zoom__live">
          <iframe class="diagram-zoom__iframe" src={src} title={title} sandbox={SANDBOX}></iframe>
        </div>
      </div>
    </div>
  );
}
