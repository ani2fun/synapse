/**
 * Simulator embeds: a `.simulator-block` marker (the ```simulator fence) becomes a same-origin
 * iframe over `/simulators/<name>/` — a self-contained static bundle a content repo ships under
 * `_simulators/<name>/`. A HEAD probe of the bundle's index.html gates the mount, so a missing
 * bundle earns the loud `.diagram-error` card (ADR-S026), never a blank frame. Raw authored
 * `<iframe src="/simulators/…">` gets the Enlarge chrome (the same wrap pattern).
 *
 * A lesson can also keep its widget BESIDE it (ADR-RS012): ```simulator src=_assets/_simulators/…
 * is resolved against the reader URL (`/synapse/<slug path>`) to `/content-assets/<slug path>/…`.
 * On a public book that URL is framed as is. On a PRIVATE book the files are gated like its prose
 * and an iframe cannot send the bearer, so the page and the scripts and stylesheets it references
 * are fetched here with the bearer (lib/privateMedia) and framed as one self-contained `srcdoc`.
 * A `srcdoc` has no query string, so the `src` query reaches the page as
 * `window.synapseSimulator.params`, defined before any of its own scripts.
 *
 * The `sandbox` attribute is belt-and-braces, not a trust boundary: the bundle is same-origin
 * first-party content (ADR-S015), so `allow-same-origin` + `allow-scripts` only blocks popups,
 * top navigation and form posts. The real gate is that content repos are first-party.
 */
import { render, h } from "preact";
import { useEffect, useState } from "preact/hooks";

import * as log from "../../lib/log";
import { privateMediaActive, resolveMedia } from "../../lib/privateMedia";
import { contentAssetUrl, selfContained } from "../../lib/simulatorDoc";

const DEFAULT_HEIGHT = 480;
const SANDBOX = "allow-scripts allow-same-origin";

// ─────────────────────────────────────────────────────────────────────────────
// DISCOVERY
// ─────────────────────────────────────────────────────────────────────────────

export function hydrateSimulators(root: ParentNode): number {
  let count = 0;
  for (const host of Array.from(root.querySelectorAll<HTMLElement>("div.simulator-block"))) {
    const name = host.getAttribute("data-name");
    const assetSrc = host.getAttribute("data-src");
    if (!name && !assetSrc) continue;
    const height = Number(host.getAttribute("data-height")) || DEFAULT_HEIGHT;
    const title = host.getAttribute("data-title") ?? `${name ?? "lesson"} simulator`;
    host.replaceChildren();
    if (name) {
      const src = `/simulators/${name}/`;
      render(h(SimulatorCard, { src, probe: `${src}index.html`, height, title, missing: `_simulators/${name}/index.html in a mounted content repository` }), host);
    } else {
      const src = contentAssetUrl(assetSrc!, location.pathname);
      if (src == null) {
        host.innerHTML = `<div class="diagram-error">This simulator lives beside its lesson (<code>${assetSrc}</code>) and shows on the lesson's own page.</div>`;
      } else {
        const probe = src.split("?")[0]!;
        render(h(SimulatorCard, { src, probe, height, title, missing: `${assetSrc} beside this lesson` }), host);
      }
    }
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
    wrap.appendChild(frame); // re-parenting reloads the iframe — accepted; the load listener rewires
    const host = document.createElement("div");
    wrap.appendChild(host);
    render(h(ZoomAffordance, { src, title: frame.title || "Simulator" }), host);
    count += 1;
  }
  return count;
}

// ─────────────────────────────────────────────────────────────────────────────
// LESSON-LOCAL WIDGETS (ADR-RS012)
// ─────────────────────────────────────────────────────────────────────────────

/** A gated file's text, fetched with the reader's bearer; throws when it was refused. */
async function privateText(url: string): Promise<string> {
  const blob = await resolveMedia(url);
  if (blob === url) throw new Error(`refused: ${url}`);
  return (await fetch(blob)).text();
}

// ─────────────────────────────────────────────────────────────────────────────
// THE INLINE EMBED: existence probe → iframe (or the loud missing card)
// ─────────────────────────────────────────────────────────────────────────────

interface CardProps {
  /** What the iframe loads: `/simulators/<name>/`, or a resolved `/content-assets/…` URL. */
  src: string;
  /** The file a HEAD probe asks for, to tell a missing bundle from a slow one. */
  probe: string;
  height: number;
  title: string;
  /** What the missing card says was expected. */
  missing: string;
}

function SimulatorCard({ src, probe, height, title, missing }: CardProps) {
  const [state, setState] = useState<"probing" | "ok" | "missing">("probing");
  // A private book's lesson-local widget, fetched with the bearer and framed as one document.
  const [doc, setDoc] = useState<string | null>(null);
  const gated = privateMediaActive() && src.startsWith("/content-assets/");

  // HEAD is answered body-free by the same route that will serve the iframe, so this settles
  // fast and warms the entry point's cache minute. An onload sniff can't tell a same-origin
  // 404 body from a slow bundle; the probe can. A gated widget has no probe to make: an
  // unauthenticated HEAD is refused by design, so fetching it with the bearer IS the probe.
  useEffect(() => {
    let cancelled = false;
    const settle = (ok: boolean) => {
      if (!cancelled) setState(ok ? "ok" : "missing");
    };
    if (gated) {
      selfContained(src, location.origin, privateText)
        .then((html) => {
          if (!cancelled) setDoc(html);
          settle(true);
        })
        .catch(() => settle(false));
    } else {
      fetch(probe, { method: "HEAD" })
        .then((res) => settle(res.ok))
        .catch(() => settle(false));
    }
    return () => {
      cancelled = true;
    };
  }, [src, probe, gated]);

  useEffect(() => {
    if (state === "missing") {
      log.warn(`simulator ${src} is not served — is its repository mounted?`);
    }
  }, [state, src]);

  if (state === "missing") {
    // Neutral wording on purpose: right after boot a satellite's first sync can lag a reload,
    // so "not served" may be transient rather than an authoring mistake.
    return (
      <div class="diagram-error">
        This simulator is not being served — expected <code>{missing}</code>.
      </div>
    );
  }
  return (
    <div class="sim-embed not-prose">
      {state === "ok" && (
        <iframe
          {...(doc == null ? { src } : { srcdoc: doc })}
          title={title}
          loading="lazy"
          sandbox={SANDBOX}
          style={{ height: `${height}px` }}
        ></iframe>
      )}
      {state === "ok" && <ZoomAffordance src={src} doc={doc} title={title} />}
    </div>
  );
}

function ZoomAffordance({ src, doc = null, title }: { src: string; doc?: string | null; title: string }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button class="sim-embed__zoom modal-btn" aria-label="Enlarge simulator" onClick={() => setOpen(true)}>
        ⤢ Enlarge
      </button>
      {open && <SimZoom src={src} doc={doc} title={title} onClose={() => setOpen(false)} />}
    </>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE FULLSCREEN ZOOM
// A NEW iframe with the same src fills the modal — moving an iframe reloads it anyway, so a
// fresh instance (state reset included) is the honest trade.
// ─────────────────────────────────────────────────────────────────────────────

function SimZoom({
  src,
  doc = null,
  title,
  onClose,
}: {
  src: string;
  doc?: string | null;
  title: string;
  onClose: () => void;
}) {
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
          <iframe
            class="diagram-zoom__iframe"
            {...(doc == null ? { src } : { srcdoc: doc })}
            title={title}
            sandbox={SANDBOX}
          ></iframe>
        </div>
      </div>
    </div>
  );
}
