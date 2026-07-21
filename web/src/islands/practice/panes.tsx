/**
 * The markdown panes shared by the practice widget and the problem-page editorial stepper (port of
 * the `markdown_pane` / `markdown_fragment` halves of client/src/execution/view/practice.rs +
 * client/src/catalog/view/editorial.rs). A fragment of editorial/statement markdown renders through
 * the SAME `renderLesson` pipeline the reader uses, then hydrates its interactive placeholders:
 * `.solution-block`s (revealed for practice, gated behind a reveal card for the editorial) and
 * fence-group bars.
 *
 * The gated/revealed split is the ONLY difference between the two callers, expressed as the `gated`
 * flag — the reveal gate is the editorial's, not the viewer's (SolutionViewer.tsx is identical
 * either way).
 */
import { render, h } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { parseVariants } from "../../lib/execution/blocks";
import type { Variant } from "../../lib/execution/blocks";
import { hydrateFenceGroups } from "../workbench/fenceGroups";
import { RELAYOUT } from "../workbench/contracts";
import { hydrateDiagrams } from "../widgets/Diagrams";
import { SolutionViewer } from "./SolutionViewer";
import * as log from "../../lib/log";

function decodedAttr(element: Element, name: string): string | null {
  const raw = element.getAttribute(name);
  if (raw == null) return null;
  try {
    return decodeURIComponent(raw);
  } catch {
    return null;
  }
}

const CODE_ICON = (
  <svg
    viewBox="0 0 24 24"
    width="19"
    height="19"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path d="m9 18 6-6-6-6"></path>
    <path d="M4 6v12" opacity="0.4"></path>
  </svg>
);

const CHEVRON_UP = (
  <svg
    viewBox="0 0 24 24"
    width="14"
    height="14"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path d="m18 15-6-6-6 6"></path>
  </svg>
);

const INFO_ICON = (
  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
    <circle cx="12" cy="12" r="10"></circle>
    <path d="M12 16v-4M12 8h.01"></path>
  </svg>
);

/**
 * The problem-page editorial's reveal gate (oracle: `GatedSolution`). The viewer mounts on reveal
 * (visible, so Monaco measures right away) and unmounts on Hide; an approach switch re-creates the
 * fragment, so it collapses again.
 */
export function GatedSolution({ variants, workbenchRoot }: { variants: Variant[]; workbenchRoot: () => HTMLElement | null }) {
  const [revealed, setRevealed] = useState(false);
  if (!revealed) {
    return (
      <button class="pwb-ereveal" onClick={() => setRevealed(true)}>
        <span class="pwb-ereveal__icon">{CODE_ICON}</span>
        <span class="pwb-ereveal__copy">
          <span class="pwb-ereveal__title">Reveal the solution</span>
          <span class="pwb-ereveal__sub">Give the intuition a shot before you peek at the code.</span>
        </span>
      </button>
    );
  }
  return (
    <div class="pwb-ereveal-open">
      <div class="pwb-ereveal-open__bar">
        <button class="wb__ghost" onClick={() => setRevealed(false)}>
          {CHEVRON_UP} Hide
        </button>
      </div>
      <SolutionViewer variants={variants} workbenchRoot={workbenchRoot} />
      <p class="pwb-ereveal__note">
        {INFO_ICON} Reference only — edit and run it in the panel on the right.
      </p>
    </div>
  );
}

/**
 * Hydrate every `.solution-block` under `root`: a revealed viewer for the practice editorial
 * (oracle: `mount_solutions`), a reveal-gated one for the problem editorial (oracle:
 * `mount_gated_solutions`). Returns the mounted host elements so the caller can unmount them.
 */
export function mountSolutionBlocks(
  root: ParentNode,
  gated: boolean,
  workbenchRoot: () => HTMLElement | null,
): HTMLElement[] {
  const hosts: HTMLElement[] = [];
  for (const element of root.querySelectorAll("div.solution-block")) {
    const json = decodedAttr(element, "data-variants");
    const variants = json ? parseVariants(json) : null;
    if (!variants || variants.length === 0) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(
      gated
        ? h(GatedSolution, { variants, workbenchRoot })
        : h(SolutionViewer, { variants, workbenchRoot }),
      host,
    );
    hosts.push(host);
  }
  return hosts;
}

/** How a fragment's `.solution-block`s hydrate: not at all (a Description/statement tab), revealed
 *  outright (a practice editorial — oracle `mount_solutions`), or behind a reveal card (the
 *  problem-page editorial — oracle `mount_gated_solutions`). */
export type SolutionMode = "none" | "revealed" | "gated";

export interface MarkdownPaneProps {
  md: string;
  solutions: SolutionMode;
  /** The editorial tab IS the answer, so any authored `<details>` inside a fragment force-opens —
   *  only the CODE asks to be revealed. Statements leave `<details>` as authored. */
  forceOpenDetails: boolean;
  workbenchRoot: () => HTMLElement | null;
}

/**
 * Render one markdown fragment and hydrate its placeholders. The oracle's same-breath pattern
 * (`el.innerHTML = html` then mount blocks) avoids a render-effect race; the async render can
 * outlive the mount, so a disposed ref just drops the result.
 */
export function MarkdownPane({ md, solutions, forceOpenDetails, workbenchRoot }: MarkdownPaneProps) {
  const host = useRef<HTMLDivElement>(null);
  const hosts = useRef<HTMLElement[]>([]);

  useEffect(() => {
    const node = host.current;
    if (!node) return;
    let live = true;
    void (async () => {
      try {
        const { renderLesson } = await import("../../lib/markdown/render");
        const html = await renderLesson(md);
        if (!live || !host.current) return;
        node.innerHTML = html;
        if (forceOpenDetails) {
          for (const details of node.querySelectorAll("details")) details.setAttribute("open", "");
        }
        if (solutions !== "none") hosts.current = mountSolutionBlocks(node, solutions === "gated", workbenchRoot);
        hydrateFenceGroups(node);
        // A09 unification (beyond the oracle's split — editorial.rs's markdown_fragment hydrated
        // diagrams, practice.rs's markdown_pane did not; this shared component now does it for
        // both, so a diagram authored inside a practice statement renders instead of sitting inert).
        hydrateDiagrams(node);
        // A viewer revealed inside a freshly-shown pane may have measured 0×0 — nudge Monaco.
        window.dispatchEvent(new Event(RELAYOUT));
      } catch (error) {
        log.error(`editorial markdown failed: ${String(error)}`);
        if (live && host.current) node.textContent = md;
      }
    })();
    return () => {
      live = false;
      for (const el of hosts.current) render(null, el);
      hosts.current = [];
    };
  }, [md]);

  return <div class="pwb__md" ref={host}></div>;
}
