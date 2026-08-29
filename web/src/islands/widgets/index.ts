/**
 * The lesson-body widget families — quiz, diagrams, simulators and citations — plus the one
 * page-wide singleton they share, the codebench modal. Auto-hydrates on import, mirroring
 * `workbench/index.ts` and `practice/index.ts`: the entry script imports this unconditionally
 * (`[...path].astro`, `blog/[slug].astro`), and this module decides what applies.
 *
 * The LESSON BODY (and a blog post — see the module doc below) gets the full pass. The PROBLEM
 * PAGE's docked description pane has no room for quiz furniture and hydrates only diagrams, scoped
 * to itself (`islands/problem.tsx` calls `hydrateDiagrams` directly) — so this module's
 * whole-document pass is guarded off `.pwb[data-problem]`, same guard as its siblings. The
 * codebench modal is the one thing EVERY page needs (a problem description can still carry a plain
 * fence-group with a "Try in Editor" button), so it mounts unconditionally.
 */
import { render, h } from "preact";

import * as log from "../../lib/log";
import { hydrateQuizzes } from "./Quiz";
import { hydrateDiagrams } from "./Diagrams";
import { hydrateSimulators } from "./Simulator";
import { hydrateCitations } from "./Citations";
import { CodebenchModal } from "./Codebench";

let codebenchMounted = false;

/** Idempotent — every entry script may call this, only the first actually mounts. */
export function mountCodebenchModal(): void {
  if (codebenchMounted) return;
  codebenchMounted = true;
  const host = document.createElement("div");
  document.body.appendChild(host);
  render(h(CodebenchModal, {}), host);
}

function init(): void {
  mountCodebenchModal();
  // The problem page owns its own (diagrams-only) hydration, scoped to the description pane.
  if (document.querySelector(".pwb[data-problem]")) return;
  const quizzes = hydrateQuizzes(document);
  const diagrams = hydrateDiagrams(document);
  const sims = hydrateSimulators(document);
  // Citations are prose furniture rather than a widget family, so they hydrate over the whole
  // document like the rest but report separately — a lesson carries hundreds of them, and folding
  // that count into the widget line would drown it.
  const cites = hydrateCitations(document);
  if (quizzes > 0 || diagrams > 0 || sims > 0) {
    log.info(`hydrated ${quizzes} quiz card(s), ${diagrams} diagram(s), ${sims} simulator(s)`);
  }
  if (cites > 0) log.debug(`citations: ${cites} marker(s) tappable`);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
