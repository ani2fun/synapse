// ──────────────────────────────────────────────────────────────────
// MAY THIS READER LISTEN? — the only part every lesson page loads
// ──────────────────────────────────────────────────────────────────
// The header's Listen pill is server-rendered VISIBLE and GATED, like "Suggest an edit". One
// config probe then decides what it becomes:
//
//   · no speech engine on this deployment (404) or the probe failed → the pill is removed: an
//     affordance nobody can ever earn is worse than none;
//   · an engine, and this reader is not on the listen list → it stays gated, and selecting prose
//     shows a gated popover too, both carrying the tooltip that says how to ask;
//   · on the list → the live player, extractor and follow-along load. Nobody else downloads them.
//
// Sign-in resolves after first paint, so the answer is asked again when auth changes, and a gated
// page upgrades to live without a reload.

import { listenConfig } from "../../lib/api/client";
import * as log from "../../lib/log";
import { AUTH_CHANGED } from "../workbench/contracts";

type Mode = "unknown" | "absent" | "gated" | "live";

let mode: Mode = "unknown";
let teardown: (() => void) | null = null;

function removePill(): void {
  (document.querySelector("[data-listen-tip]") ?? document.querySelector("[data-listen-header]"))?.remove();
}

async function probe(): Promise<void> {
  const body = document.querySelector<HTMLElement>("article.lesson-body");
  if (!body || mode === "live" || mode === "absent") return;
  let config;
  try {
    config = await listenConfig();
  } catch {
    mode = "absent";
    removePill();
    log.debug("listen: not offered on this deployment");
    return;
  }
  if (config.canListen) {
    teardown?.();
    mode = "live";
    const { mount } = await import("./listener");
    teardown = mount(config);
  } else if (mode !== "gated") {
    mode = "gated";
    const { mountPopover } = await import("./popover");
    teardown = mountPopover(body, {});
    log.debug("listen: gated — not on the listen list");
  }
}

void probe();
window.addEventListener(AUTH_CHANGED, () => void probe());
