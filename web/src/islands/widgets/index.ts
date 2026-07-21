/**
 * The lesson-body widget families (A09) — quiz, diagrams, and the LikeC4 embed chrome, plus the
 * two page-wide singletons they share (the C4 docs panel and the codebench modal). Auto-hydrates
 * on import, mirroring `workbench/index.ts` and `practice/index.ts`: the entry script imports
 * this unconditionally (`[...path].astro`, `blog/[slug].astro`), and this module decides what
 * applies.
 *
 * Oracle split (reader.rs vs problem.rs): the LESSON BODY (and, new for the migration, a blog
 * post — see the module doc below) gets the full pass — quiz + diagrams + c4 (+ its docs panel).
 * The PROBLEM PAGE's docked description pane has no room for quiz/c4 furniture and hydrates only
 * diagrams, scoped to itself (`islands/problem.tsx` calls `hydrateDiagrams` directly) — so this
 * module's whole-document pass is guarded off `.pwb[data-problem]`, same guard as its siblings.
 * The codebench modal is the one thing EVERY page needs (a problem description can still carry a
 * plain fence-group with a "Try in Editor" button), so it mounts unconditionally.
 */
import { render, h } from "preact";

import * as log from "../../lib/log";
import { hydrateQuizzes } from "./Quiz";
import { hydrateDiagrams } from "./Diagrams";
import { hydrateC4Embeds } from "./C4Embed";
import { C4DocsPanel } from "./C4DocsPanel";
import { CodebenchModal } from "./Codebench";
import { c4Selected } from "./c4Store";

function lessonPathFromUrl(): string[] {
  const path = window.location.pathname;
  if (!path.startsWith("/synapse/")) return [];
  return path
    .slice("/synapse/".length)
    .split("/")
    .filter((segment) => segment !== "");
}

let codebenchMounted = false;

/** Idempotent — every entry script may call this, only the first actually mounts. */
export function mountCodebenchModal(): void {
  if (codebenchMounted) return;
  codebenchMounted = true;
  const host = document.createElement("div");
  document.body.appendChild(host);
  render(h(CodebenchModal, {}), host);
}

let docsPanelMounted = false;

function mountC4DocsPanel(lessonPath: string[]): void {
  if (docsPanelMounted) return;
  docsPanelMounted = true;
  const host = document.createElement("div");
  document.body.appendChild(host);
  render(h(C4DocsPanel, { lessonPath }), host);
}

function init(): void {
  mountCodebenchModal();
  // The problem page owns its own (diagrams-only) hydration, scoped to the description pane.
  if (document.querySelector(".pwb[data-problem]")) return;
  const quizzes = hydrateQuizzes(document);
  const diagrams = hydrateDiagrams(document);
  mountC4DocsPanel(lessonPathFromUrl());
  const c4 = hydrateC4Embeds(document, (id) => c4Selected.set(id));
  if (quizzes > 0 || diagrams > 0 || c4 > 0) {
    log.info(`hydrated ${quizzes} quiz card(s), ${diagrams} diagram(s), ${c4} c4 embed(s)`);
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
