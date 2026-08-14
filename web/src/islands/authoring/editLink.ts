// The "Suggest an edit" affordance on a lesson page.
//
// The link is server-rendered VISIBLE but GATED — the problem workbench's Submit grammar: someone
// who cannot use it still sees that it exists and, on hover, how to ask for access. That default
// is also the safe one, because it is what a reader gets if this island never runs.
//
// This upgrades it in exactly two ways:
//   · the server says the caller may edit  → the href is granted, ordinary tooltip;
//   · editing is switched off entirely     → removed, since there is nothing to ask for.
// Anything else leaves the gated default alone.
//
// One network call, cached across the page's lifetime. `canEdit` already folds in "editing is
// enabled", "signed in" and "on the content-editor list", so there is nothing to re-derive here.

import { onEditGate } from "../../lib/api/editGate";
import * as log from "../../lib/log";

const LINK = "[data-edit-link]";
const TIP = "[data-edit-tip]";
const GATED = "lesson-edit-link--gated";
const ACTIVE_TIP = "Edit this page and open a change request";

/** Hand the link its destination. The server ships the anchor WITHOUT an href, so until this runs
 *  there is nothing to click through to — no listener to swallow the click, and no window between
 *  first paint and hydration in which a gated reader can still reach the editor. */
function activate(link: HTMLAnchorElement, tip: HTMLElement | null): void {
  const href = link.dataset.editHref;
  if (href == null) return; // no destination to grant; the gated default stands
  link.href = href;
  link.classList.remove(GATED);
  link.removeAttribute("aria-disabled");
  tip?.setAttribute("data-tip", ACTIVE_TIP);
}

function apply(gate: { enabled: boolean; canEdit: boolean }): void {
  const link = document.querySelector<HTMLAnchorElement>(LINK);
  const tip = document.querySelector<HTMLElement>(TIP);
  if (!link) return;
  if (!gate.enabled) {
    // The deployment does not offer editing at all — an affordance nobody can ever earn is
    // worse than none, so it goes away rather than sitting there permanently gated.
    (tip ?? link).remove();
    return;
  }
  if (gate.canEdit) {
    activate(link, tip);
    log.info('edit: "Suggest an edit" is live');
  } else {
    log.debug("edit: not a content editor — the affordance stays gated");
  }
}

function init(): void {
  if (document.querySelector<HTMLAnchorElement>(LINK) == null) return;
  // Through the shared gate: this page now has several affordances asking the same question —
  // this link, and an Edit pill on every d2 figure — and it resolves once for all of them,
  // re-resolving when auth flips. A failed or absent config lands on "editing is off", which is
  // the same gated default this started with.
  onEditGate(apply);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
