/**
 * The markdown panes shared by the practice widget and the problem-page editorial stepper. A
 * fragment of editorial/statement markdown renders through the SAME `renderLesson` pipeline the
 * reader uses, then hydrates its interactive placeholders: `.solution-block`s (shown outright, in
 * the practice widget and the problem-page editorial alike) and fence-group bars.
 */
import { render, h } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { parseVariants } from "../../lib/execution/blocks";
import type { Variant } from "../../lib/execution/blocks";
import { hydrateFenceGroups } from "../workbench/fenceGroups";
import { RELAYOUT, VIZ_RESCAN } from "../workbench/contracts";
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

/**
 * Hydrate every `.solution-block` under `root` with a solution viewer. Returns the mounted host
 * elements so the caller can unmount them.
 */
export function mountSolutionBlocks(
  root: ParentNode,
  workbenchRoot: () => HTMLElement | null,
): HTMLElement[] {
  const hosts: HTMLElement[] = [];
  for (const element of root.querySelectorAll("div.solution-block")) {
    const json = decodedAttr(element, "data-variants");
    const variants = json ? parseVariants(json) : null;
    if (!variants || variants.length === 0) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(SolutionViewer, { variants, workbenchRoot }), host);
    hosts.push(host);
  }
  return hosts;
}

/** How a fragment's `.solution-block`s hydrate: not at all (a Description/statement tab), or shown
 *  outright (a practice or problem-page editorial). */
export type SolutionMode = "none" | "revealed";

export interface MarkdownPaneProps {
  md: string;
  solutions: SolutionMode;
  /** The editorial tab IS the answer, so any authored `<details>` inside a fragment force-opens —
   *  only the CODE asks to be revealed. Statements leave `<details>` as authored. */
  forceOpenDetails: boolean;
  workbenchRoot: () => HTMLElement | null;
}

/**
 * Render one markdown fragment and hydrate its placeholders. The same-breath pattern
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
        if (solutions !== "none") hosts.current = mountSolutionBlocks(node, workbenchRoot);
        hydrateFenceGroups(node);
        // Diagrams hydrate here too, so a diagram authored inside a practice statement renders
        // instead of sitting inert.
        hydrateDiagrams(node);
        // A ```viz fence in an editorial plants a fresh widget AFTER the viz loader's initial
        // sweep — ask the loader to re-sweep (idempotent, loads on demand).
        window.dispatchEvent(new Event(VIZ_RESCAN));
        // A viewer mounted inside a freshly-shown pane may have measured 0×0 — nudge Monaco.
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
