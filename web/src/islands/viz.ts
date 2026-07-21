/**
 * The lazy viz loader: the ONLY code that touches the viz wasm bundle. The crate
 * (viz-wasm/) ships the whole widget spine — inline widgets, the trace session, the Visualise
 * modal — as a standalone wasm-bindgen bundle; this island decides WHEN a page pays for it and
 * wires the window contracts once it has.
 *
 * When a page wants it:
 *   · it has planted `.viz-widget`s (authored ```viz fences) — load when the FIRST one nears
 *     the viewport (same 600px margin as the workbench's lazy Monaco), eager without IO;
 *   · or a workbench variant carries a `viz=` hint (the Visualise button's fuel) — load on
 *     idle, so the button appears without user action but never blocks hydration.
 * Neither → this module does nothing, and the page never fetches a byte of wasm.
 *
 * Contracts installed after init:
 *   · `window.__synapseViz` (contracts.ts) → `viz_open_modal` — its arrival re-renders
 *     workbenches via VIZ_READY so the Visualise button appears;
 *   · the crate's bearer seam gets a WRAPPER reading `window.__synapseVizToken` at call time, so
 *     the identity island can install/refresh its provider in either order relative to this load.
 */
import * as log from "../lib/log";
import { VIZ_READY, VIZ_RESCAN } from "./workbench/contracts";

type VizModule = typeof import("../lib/viz-wasm/pkg/viz_wasm.js");

let loading: Promise<VizModule> | null = null;

function load(): Promise<VizModule> {
  loading ??= (async () => {
    log.info("viz: loading the wasm bundle");
    const mod = await import("../lib/viz-wasm/pkg/viz_wasm.js");
    await mod.default();
    mod.viz_install_token(() => window.__synapseVizToken?.() ?? null);
    window.__synapseViz = (detail) => {
      mod.viz_open_modal(detail.language, detail.source, detail.vizHint, detail.stdin);
    };
    window.dispatchEvent(new Event(VIZ_READY));
    const mounted = mod.viz_mount_widgets();
    log.info(`viz: ready (${mounted} inline widget(s) mounted)`);
    return mod;
  })();
  return loading;
}

function workbenchWantsViz(): boolean {
  for (const el of document.querySelectorAll("div.workbench[data-variants]")) {
    const raw = el.getAttribute("data-variants");
    if (!raw) continue;
    try {
      const variants = JSON.parse(decodeURIComponent(raw)) as { viz?: string | null }[];
      if (variants.some((v) => typeof v.viz === "string" && v.viz !== "")) return true;
    } catch {
      // a malformed placeholder is the hydrator's problem, not the loader's
    }
  }
  return false;
}

function init(): void {
  const firstWidget = document.querySelector(".viz-widget");
  if (firstWidget) {
    if (typeof IntersectionObserver === "undefined") {
      void load();
      return;
    }
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          io.disconnect();
          void load();
        }
      },
      { rootMargin: "600px 0px" },
    );
    io.observe(firstWidget);
    return;
  }
  if (workbenchWantsViz()) {
    log.debug("viz: no inline widgets, but a workbench carries a viz hint — loading on idle");
    const idle = window.requestIdleCallback ?? ((cb: () => void) => setTimeout(cb, 1500));
    idle(() => void load());
  }
}

// Late-rendered markdown (the editorial pane) may plant fresh widgets after the initial pass —
// re-sweep on request. Mounting is marker-idempotent in the crate, so a body-wide sweep is safe;
// when the page had no reason to load before, the rescan IS the reason (load() mounts on arrival).
window.addEventListener(VIZ_RESCAN, () => {
  if (loading) {
    void loading.then((mod) => {
      const mounted = mod.viz_mount_widgets();
      if (mounted > 0) log.debug(`viz: rescan mounted ${mounted} widget(s)`);
    });
  } else if (document.querySelector(".viz-widget")) {
    void load();
  }
});

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
