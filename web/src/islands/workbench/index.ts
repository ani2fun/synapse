/**
 * Workbench hydration (port of client/src/execution/view/hydrate.rs): find every
 * `div.workbench` the pipeline planted, decode `data-variants` (+ optional `data-spec`), and
 * render a live Preact Workbench into it. Fence-group bars hydrate alongside. The lesson path
 * comes from the URL — the same directory-mirror segments the payload was fetched for.
 */
import { render, h } from "preact";

import * as log from "../../lib/log";
import { parseVariants } from "../../lib/execution/blocks";
import type { TestSpec } from "../../lib/execution/judge";
import { hydrateFenceGroups } from "./fenceGroups";
import { Workbench } from "./Workbench";

function decodedAttr(element: Element, name: string): string | null {
  const raw = element.getAttribute(name);
  if (raw == null) return null;
  try {
    return decodeURIComponent(raw);
  } catch {
    return null;
  }
}

function lessonPathFromUrl(): string[] {
  const path = window.location.pathname;
  if (!path.startsWith("/synapse/")) return [];
  return path
    .slice("/synapse/".length)
    .split("/")
    .filter((segment) => segment !== "");
}

export function hydrateWorkbenches(root: ParentNode): void {
  const lessonPath = lessonPathFromUrl();
  let workbenches = 0;
  for (const element of root.querySelectorAll("div.workbench")) {
    const variantsJson = decodedAttr(element, "data-variants");
    const variants = variantsJson ? parseVariants(variantsJson) : null;
    if (!variants || variants.length === 0) continue;
    let spec: TestSpec | null = null;
    const specJson = decodedAttr(element, "data-spec");
    if (specJson != null) {
      try {
        spec = JSON.parse(specJson) as TestSpec;
      } catch {
        spec = null;
      }
    }
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(Workbench, { variants, spec, lessonPath, root: host }), host);
    workbenches += 1;
  }
  const groups = root.querySelectorAll("div.fence-group").length;
  hydrateFenceGroups(root);
  log.info(`hydrated ${workbenches} workbench(es), ${groups} fence group(s)`);
}

// Auto-hydrate on import — the lesson page's script tag is the trigger.
hydrateWorkbenches(document);
