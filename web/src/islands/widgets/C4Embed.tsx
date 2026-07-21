/**
 * The LikeC4 lesson embed chrome: every authored `<iframe src="/c4/…">` is wrapped so an Enlarge
 * button (top-LEFT — LikeC4 owns top-right) floats over it; Enlarge opens the near-fullscreen
 * iframe zoom with parity chrome — − / + buttons driving SYNTHETIC ctrl+wheel pinches at the
 * viewer's `.react-flow__pane`, a live % read from the viewport transform, and the gesture hint.
 * While a `.likec4-overlay[open]` dialog is up inside the iframe, OUR chrome steps aside (its
 * ✕ · Share · Export render exactly where ours sits). Everything relies on the `/c4` proxy
 * keeping the iframe same-origin.
 *
 * CROSS-REALM RULE: this runs in the SAME JS engine as the DOM it touches, so property access
 * (`el.tagName`, `frame.contentWindow`) works directly across frames; only
 * `instanceof`/constructor-identity checks are realm-sensitive across the parent/iframe boundary.
 * The one place a foreign constructor matters is the synthetic wheel event: it is built from the
 * IFRAME's OWN `WheelEvent` (not the parent's), because react-flow's internal handling runs in
 * that realm.
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

/** The click-to-docs bridge: a CAPTURE-phase click listener on the same-origin iframe document.
 *  The composed path (target-first) feeds the pure `resolveC4Node`; on a hit the click is
 *  swallowed and the docs panel opens. */
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
  const [overlay, setOverlay] = useState(false);
  const frameRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // The one poll: the overlay state (our chrome steps aside while LikeC4's dialog is up). A
  // MutationObserver inside a live React canvas would fire every pan frame; one timer is
  // cheaper and dies with the modal. NO ± / % pill of our own here (user call, 2026-07-21):
  // LikeC4's react-flow viewer already renders its own zoom column bottom-left, so the
  // synthetic-wheel controls duplicated it — the parity chrome that made sense on the small
  // inline embed is noise on the fullscreen one.
  useEffect(() => {
    const id = window.setInterval(() => {
      const doc = frameRef.current?.contentDocument;
      if (!doc) return;
      injectScopeStyle(doc);
      setOverlay(doc.querySelector(".likec4-overlay[open]") != null);
    }, 300);
    return () => window.clearInterval(id);
  }, []);

  return (
    <div class="diagram-zoom-scrim" onClick={onClose}>
      <div
        class={overlay ? "diagram-zoom diagram-zoom--fill diagram-zoom--c4-overlay" : "diagram-zoom diagram-zoom--fill"}
        onClick={(event) => event.stopPropagation()}
      >
        {/* modal-btn = the shared teal pill every other overlay's Close button uses — the bare
            class read as an unstyled stray. */}
        <button class="diagram-zoom__close modal-btn" aria-label="Close" onClick={onClose}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M18 6 6 18M6 6l12 12"></path>
          </svg>
          Close
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
        </div>
      </div>
    </div>
  );
}
