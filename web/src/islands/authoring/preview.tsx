// The live preview — the quality gate this whole feature turns on. It renders the edited buffer
// through the EXACT pipeline the reader page uses, into the EXACT DOM the reader hydrates, so
// "did my table render?" and "did my mermaid diagram parse?" are questions the contributor
// answers themselves, before a reviewer's time is spent on them.
//
// Three things make the preview the page rather than an approximation of it, and all three reuse
// what the reader already runs:
//   1. the same markdown pipeline — `renderLesson` from lib/markdown/render, lazily imported;
//   2. the same DOM + stylesheets — `.lesson > .lesson-header + .lesson-body.synapse-prose`, and
//      the edit page imports the reader's stylesheet set;
//   3. the same hydrators, scoped to THIS container — every hydrator takes a ParentNode root,
//      which is how islands/problem scopes its own pass.

import { render, h } from "preact";

import * as log from "../../lib/log";
import { splitFrontmatter, titleOf, summaryOf } from "../../lib/markdown/frontmatter";
import { VIZ_RESCAN } from "../workbench/contracts";

export interface PreviewHtml {
  /** The reader header (title + optional lede) as an HTML string. */
  readonly headerHtml: string;
  /** The rendered lesson body as an HTML string — `renderLesson` output. */
  readonly bodyHtml: string;
}

/** Minimal HTML escape for the header text — the title/summary are plain frontmatter values that
 *  land in `textContent` positions, so only the five markup characters need handling. */
function escape(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * Render `source` to the header + body HTML the reader shows. Deliberately returns STRINGS rather
 * than touching the DOM: the caller hands them to Preact via `dangerouslySetInnerHTML`, so the
 * rendered markup is opaque to reconciliation and a re-render cannot wipe it (a raw
 * `body.innerHTML =` into a Preact-managed node does — the bug the signed-in e2e caught). Hydrate
 * with `hydratePreview` AFTER Preact has committed the body.
 */
export async function renderPreview(source: string): Promise<PreviewHtml> {
  const title = titleOf(source) ?? "Untitled";
  const summary = summaryOf(source);
  const headerHtml =
    `<h1 class="reader-prose__title">${escape(title)}</h1>` +
    (summary ? `<p class="reader-prose__lede">${escape(summary)}</p>` : "");

  // Only the body below the frontmatter is prose — the reader strips the fence before rendering,
  // and so must the preview, or the raw `--- title: … ---` block shows up as text.
  const { body: markdown } = splitFrontmatter(source);
  const { renderLesson } = await import("../../lib/markdown/render");
  const bodyHtml = await renderLesson(markdown);
  log.debug("preview: rendered");
  return { headerHtml, bodyHtml };
}

let vizRequested = false;
let codebenchMounted = false;

/** Run the reader's own hydrators, scoped to the preview body — never the document-wide pass, so
 *  no page-level singletons fire. Every hydrator is imported by its own MODULE, not through
 *  `islands/widgets`, whose import would trigger that module's whole-document auto-hydration.
 *  Call this AFTER the body's HTML is in the DOM (Preact committed it). */
export async function hydratePreview(body: HTMLElement): Promise<void> {
  const [{ hydrateQuizzes }, { hydrateDiagrams }, { hydrateC4Embeds }, { hydrateWorkbenches }, { hydratePractices }] =
    await Promise.all([
      import("../widgets/Quiz"),
      import("../widgets/Diagrams"),
      import("../widgets/C4Embed"),
      import("../workbench"),
      import("../practice"),
    ]);

  hydrateQuizzes(body);
  hydrateDiagrams(body);
  hydrateC4Embeds(body, () => {
    /* the preview has no C4 docs panel — selecting a component is a no-op here */
  });
  hydrateWorkbenches(body);
  hydratePractices(body);
  // The "Try in Editor" modal is a body-mounted singleton — a fence-group button in the preview
  // opens it exactly as on the real page. Mounted from its component directly (not through
  // `islands/widgets`, whose import runs the whole-document pass), and only once.
  await mountCodebench();

  // ```viz fences plant `.viz-widget` placeholders that only the lazy wasm bundle mounts. Import
  // the loader the first time one appears, and on every later render nudge it to re-scan — the
  // marker-idempotent seam the crate already exposes, so a re-render never double-mounts.
  if (body.querySelector(".viz-widget")) {
    if (!vizRequested) {
      vizRequested = true;
      void import("../viz");
    }
    window.dispatchEvent(new Event(VIZ_RESCAN));
  }
  log.debug("preview: hydrated");
}

async function mountCodebench(): Promise<void> {
  if (codebenchMounted) return;
  codebenchMounted = true;
  const { CodebenchModal } = await import("../widgets/Codebench");
  const host = document.createElement("div");
  document.body.appendChild(host);
  render(h(CodebenchModal, {}), host);
}
