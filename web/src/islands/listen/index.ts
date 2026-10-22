// ──────────────────────────────────────────────────────────────────
// MAY THIS READER LISTEN? — the only part every lesson page loads
// ──────────────────────────────────────────────────────────────────
// One config probe. `canListen` already folds in "a speech engine is configured", "signed in" and
// "on the listen list", so there is nothing to re-derive here. Only a yes loads the affordances,
// the player and the extractor — nobody else downloads a byte of them. A 404 (no engine on this
// deployment) and a failed probe both read as "not offered".
//
// Sign-in resolves after first paint, so a "no" is asked again when auth changes.

import { listenConfig } from "../../lib/api/client";
import * as log from "../../lib/log";
import { AUTH_CHANGED } from "../workbench/contracts";

let mounted = false;

async function probe(): Promise<void> {
  if (mounted || !document.querySelector("article.lesson-body")) return;
  try {
    const config = await listenConfig();
    if (!config.canListen) {
      log.debug("listen: not enabled for this reader");
      return;
    }
    mounted = true;
    const { mount } = await import("./listener");
    mount(config);
  } catch {
    log.debug("listen: not offered on this deployment");
  }
}

void probe();
window.addEventListener(AUTH_CHANGED, () => void probe());
