/**
 * The LikeC4 lesson embed chrome (port of client/src/catalog/view/c4.rs — oracle: `C4Blocks` +
 * `DiagramZoom.openIframe`, commit d8b969a): every authored `<iframe src="/c4/…">` is wrapped so
 * an Enlarge button (top-LEFT — LikeC4 owns top-right) floats over it; Enlarge opens the
 * near-fullscreen iframe zoom with parity chrome — − / + buttons driving SYNTHETIC ctrl+wheel
 * pinches at the viewer's `.react-flow__pane`, a live % read from the viewport transform, and
 * the gesture hint. While a `.likec4-overlay[open]` dialog is up inside the iframe, OUR chrome
 * steps aside (its ✕ · Share · Export render exactly where ours sits). Everything relies on the
 * `/c4` proxy keeping the iframe same-origin.
 *
 * DIVERGENCE FROM THE RUST, on purpose: the Rust needed `js_sys::Reflect` everywhere because
 * `wasm_bindgen::JsCast` casts (`dyn_into`/`instanceof`) always fail across the parent/iframe
 * realm boundary. This is plain TypeScript running in the SAME JS engine as the DOM it touches —
 * property access (`el.tagName`, `frame.contentWindow`) works directly across frames; only
 * `instanceof`/constructor-identity checks are realm-sensitive, and this module makes none. The
 * one place a foreign constructor still matters is the synthetic wheel event: it is built from
 * the IFRAME's OWN `WheelEvent` (not the parent's), because react-flow's internal handling runs
 * in that realm — the same rule as the Rust, just without the `Reflect` ceremony to express it.
 */
import { render, h } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { resolveC4Node } from "../../lib/catalog/tree";
import type { C4PathHop } from "../../lib/catalog/tree";

/** Hide LikeC4's merged-workspace nav panel (its view picker lists EVERY diagram across all
 *  books — `/c4` is one merged build); UX scoping only. */
const SCOPE_CSS = '[class~="layerStyle_likec4.panel"] { display: none !important; }';

function injectScopeStyle(doc: Document): void {
  if (doc.getElementById("__syn-c4-inject")) return;
  const style = doc.createElement("style");
  style.id = "__syn-c4-inject";
  style.textContent = SCOPE_CSS;
  (doc.head ?? doc.documentElement)?.appendChild(style);
}

/** The click-to-docs bridge (oracle: `attachNodeBridge`): a CAPTURE-phase click listener on the
 *  same-origin iframe document. The composed path (target-first) feeds the pure `resolveC4Node`;
 *  on a hit the click is swallowed and the docs panel opens. */
function attachNodeBridge(doc: Document, onSelect: (id: string) => void): void {
  doc.addEventListener(
    "click",
    (event) => {
      const hops: C4PathHop[] = [];
      for (const target of event.composedPath()) {
        const el = target as Partial<Element>;
        if (typeof el.tagName !== "string") continue; // window/document hops drop out
        const classes = typeof el.className === "string" ? el.className : "";
        const dataId = el.getAttribute ? el.getAttribute("data-id") : null;
        hops.push([el.tagName, classes, dataId]);
      }
      const id = resolveC4Node(hops);
      if (id != null) {
        event.stopPropagation();
        event.preventDefault();
        onSelect(id);
      }
    },
    { capture: true },
  );
}

/** `"translate(12px, 3px) scale(1.25)"` → `125`. */
function parseScalePct(transform: string): number | null {
  const start = transform.indexOf("scale(");
  if (start === -1) return null;
  const rest = transform.slice(start + "scale(".length);
  const end = rest.indexOf(")");
  if (end === -1) return null;
  const value = Number.parseFloat(rest.slice(0, end));
  return Number.isFinite(value) ? Math.round(value * 100) : null;
}

// ─────────────────────────────────────────────────────────────────────────────
// DISCOVERY
// ─────────────────────────────────────────────────────────────────────────────

export function hydrateC4Embeds(root: ParentNode, onSelect: (id: string) => void): number {
  let count = 0;
  for (const frame of Array.from(root.querySelectorAll<HTMLIFrameElement>("iframe[src^='/c4/']"))) {
    const parent = frame.parentElement;
    const src = frame.getAttribute("src");
    if (!parent || src == null) continue;
    // Wrap: <div.c4-embed> around the iframe (re-parenting reloads it — accepted; the load
    // listener below re-fires all wiring), plus a host div for the button mount.
    const wrap = document.createElement("div");
    wrap.className = "c4-embed";
    parent.insertBefore(wrap, frame);
    wrap.appendChild(frame);
    const host = document.createElement("div");
    wrap.appendChild(host);
    render(h(C4Embed, { frame, wrap, src, onSelect }), host);
    count += 1;
  }
  return count;
}

// ─────────────────────────────────────────────────────────────────────────────
// THE INLINE EMBED: Enlarge + overlay guard + scope style
// ─────────────────────────────────────────────────────────────────────────────

function C4Embed({
  frame,
  wrap,
  src,
  onSelect,
}: {
  frame: HTMLIFrameElement;
  wrap: HTMLElement;
  src: string;
  onSelect: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);

  // The overlay guard: watch `.likec4-overlay[open]` inside the SAME-ORIGIN iframe with a
  // MutationObserver (childList+subtree catch the dialog's first insertion; the `open` attribute
  // filter catches show/close — the <dialog> lingers once used). Re-wired on every iframe load.
  useEffect(() => {
    let observer: MutationObserver | null = null;
    const wire = (): void => {
      observer?.disconnect();
      const doc = frame.contentDocument;
      if (!doc) return;
      observer = new MutationObserver(() => {
        const overlay = doc.querySelector(".likec4-overlay[open]") != null;
        wrap.classList.toggle("c4-embed--overlay", overlay);
      });
      observer.observe(doc.documentElement, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["open"],
      });
      injectScopeStyle(doc);
      attachNodeBridge(doc, onSelect);
    };
    frame.addEventListener("load", wire);
    wire();
    return () => {
      frame.removeEventListener("load", wire);
      observer?.disconnect();
    };
  }, [frame, wrap, onSelect]);

  return (
    <>
      <button class="c4-embed__zoom" aria-label="Enlarge diagram" onClick={() => setOpen(true)}>
        ⤢ Enlarge
      </button>
      {open && <C4Zoom src={src} onClose={() => setOpen(false)} onSelect={onSelect} />}
    </>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE FULLSCREEN IFRAME ZOOM
// A NEW iframe with the same src fills the modal (LikeC4 owns its own pan/zoom); one 300 ms poll
// reads the live scale % AND the overlay state.
// ─────────────────────────────────────────────────────────────────────────────

function C4Zoom({ src, onClose, onSelect }: { src: string; onClose: () => void; onSelect: (id: string) => void }) {
  const [scalePct, setScalePct] = useState<number | null>(null);
  const [overlay, setOverlay] = useState(false);
  const frameRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // The one poll: live scale % (the viewport's `scale(N)` transform) + overlay state. A
  // MutationObserver inside a live React canvas would fire every pan frame; one timer parsing
  // one transform is cheaper and dies with the modal.
  useEffect(() => {
    const id = window.setInterval(() => {
      const doc = frameRef.current?.contentDocument;
      if (!doc) return;
      injectScopeStyle(doc);
      const style = doc.querySelector(".react-flow__viewport")?.getAttribute("style") ?? null;
      setScalePct(style ? parseScalePct(style) : null);
      setOverlay(doc.querySelector(".likec4-overlay[open]") != null);
    }, 300);
    return () => window.clearInterval(id);
  }, []);

  // ± steps ≈ ±25%: a synthetic ctrl+wheel pinch built from the IFRAME's OWN `WheelEvent`
  // constructor, dispatched at the react-flow pane's centre (deltaY −16 in · +16 out).
  const zoomStep = (zoomIn: boolean): void => {
    const frame = frameRef.current;
    const doc = frame?.contentDocument;
    const win = frame?.contentWindow;
    const pane = doc?.querySelector(".react-flow__pane");
    if (!doc || !win || !pane) return;
    const rect = pane.getBoundingClientRect();
    // `Window` (lib.dom.d.ts) doesn't type its global constructors as instance properties, but
    // every real Window object carries one — and the IFRAME's OWN constructor is the one that
    // matters here (react-flow's handling runs in that realm).
    const WheelEventCtor = (win as unknown as { WheelEvent: typeof WheelEvent }).WheelEvent;
    const event = new WheelEventCtor("wheel", {
      deltaY: zoomIn ? -16 : 16,
      clientX: rect.left + rect.width / 2,
      clientY: rect.top + rect.height / 2,
      bubbles: true,
      cancelable: true,
      ctrlKey: true,
    });
    pane.dispatchEvent(event);
  };

  return (
    <div class="diagram-zoom-scrim" onClick={onClose}>
      <div
        class={overlay ? "diagram-zoom diagram-zoom--fill diagram-zoom--c4-overlay" : "diagram-zoom diagram-zoom--fill"}
        onClick={(event) => event.stopPropagation()}
      >
        <button class="diagram-zoom__close" aria-label="Close" onClick={onClose}>
          ✕ Close
        </button>
        <div class="diagram-zoom__live">
          <iframe
            class="diagram-zoom__iframe"
            src={src}
            title="LikeC4 diagram"
            ref={frameRef}
            onLoad={() => {
              const doc = frameRef.current?.contentDocument;
              if (doc) attachNodeBridge(doc, onSelect);
            }}
          ></iframe>
          <div class="diagram-zoom__controls">
            <button class="diagram-zoom__ctl" aria-label="Zoom out" onClick={() => zoomStep(false)}>
              −
            </button>
            <span class="diagram-zoom__level">{scalePct != null ? `${scalePct}%` : "—"}</span>
            <button class="diagram-zoom__ctl" aria-label="Zoom in" onClick={() => zoomStep(true)}>
              +
            </button>
            <span class="diagram-zoom__hint">or pinch / Ctrl+scroll to zoom · scroll or drag to pan</span>
          </div>
        </div>
      </div>
    </div>
  );
}
