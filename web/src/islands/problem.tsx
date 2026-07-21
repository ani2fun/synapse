/**
 * The standalone problem page's interactive layer. The Astro page server-renders the whole
 * static frame — crumbs, the left pane's head + tab bar, the DESCRIPTION markdown, the docked
 * nav bar — and this island wires the living parts over it:
 *
 *   · the splitter (28–64%, the one thing the page still remembers, `pane.ts`),
 *   · the Description | Editorial | Submissions tabs (mount-once, `.hidden`; every problem opens
 *     on Description),
 *   · the right pane's Workbench, with the FIRST description workbench EXTRACTED into it,
 *   · the remaining description workbenches + fence-group bars, hydrated in place,
 *   · the Submissions feed (lazy, refetched on submit) and the anonymous sign-in bar,
 *   · the Contents pill, which opens the reader's nav drawer by event (`reader.ts`).
 *
 * DOUBLE-HYDRATION GUARD: this module imports `Workbench`/`parseVariants`/`hydrateFenceGroups`
 * DIRECTLY, never `islands/workbench` (whose bottom line auto-hydrates `document`). The problem
 * page's `.astro` therefore loads `islands/problem` INSTEAD of `islands/workbench`, so the first
 * workbench is extracted and mounted exactly once — by this island — with no auto-hydrator racing
 * it for the same placeholder.
 *
 * The Editorial tab mounts the stepper island (`practice/EditorialPane`) on first open; the
 * Coach tab mounts `coach/CoachPane` the same way, reading its problem path off the URL and its
 * live code snapshot off the workbench root's bubbling `synapse:code-changed`. Description
 * workbenches also get the diagram + codebench-modal treatment (`islands/widgets`). Visualise
 * renders once the viz island installs `window.__synapseViz`; Edit/Submit/Submissions reflect
 * real auth state through `window.__synapseAuth` — anonymous readers see the correct restricted
 * experience.
 */
import { h, render } from "preact";
import { useEffect, useState } from "preact/hooks";

import * as log from "../lib/log";
import { PROBLEM_PANE_KEY, get as storageGet, set as storageSet } from "../lib/storage";
import { DEFAULT_LEFT_PCT, MAX_LEFT_PCT, MIN_LEFT_PCT, parseLeftPct, serializeLeftPct } from "../lib/catalog/pane";
import { parseVariants } from "../lib/execution/blocks";
import type { Variant } from "../lib/execution/blocks";
import type { TestSpec } from "../lib/execution/judge";
import { Workbench } from "./workbench/Workbench";
import { hydrateFenceGroups } from "./workbench/fenceGroups";
import { AUTH_CHANGED, isAuthed, OPEN_CONTENTS, RELAYOUT, SUBMITTED } from "./workbench/contracts";
import { SubmissionsFeed } from "./problem-submissions";
import { EditorialPane } from "./practice/EditorialPane";
import { CoachPane } from "./coach/CoachPane";
import { hydrateDiagrams } from "./widgets/Diagrams";
// Side-effect import: mounts the page-wide codebench modal — a description-pane fence
// group can still carry a "Try in Editor" button even though the whole-document quiz/diagram/c4
// pass this module also offers stands down on a problem page (see its own module doc).
import "./widgets/index";

// The workbench root (its event target) is minted when the right pane mounts; the Submissions feed
// dispatches LOAD_CODE / USE_CASE onto it, so it is shared through this module-scoped getter.
let workbenchRootEl: HTMLElement | null = null;
const workbenchRoot = (): HTMLElement | null => workbenchRootEl;

function lessonPathFromUrl(): string[] {
  const path = window.location.pathname;
  if (!path.startsWith("/synapse/")) return [];
  return path
    .slice("/synapse/".length)
    .split("/")
    .filter((segment) => segment !== "");
}

function decodedAttr(element: Element, name: string): string | null {
  const raw = element.getAttribute(name);
  if (raw == null) return null;
  try {
    return decodeURIComponent(raw);
  } catch {
    return null;
  }
}

/** Decode a `div.workbench` placeholder's variants (+ optional suite). `null` when unusable. */
function decodeWorkbench(element: Element): { variants: Variant[]; spec: TestSpec | null } | null {
  const variantsJson = decodedAttr(element, "data-variants");
  const variants = variantsJson ? parseVariants(variantsJson) : null;
  if (!variants || variants.length === 0) return null;
  let spec: TestSpec | null = null;
  const specJson = decodedAttr(element, "data-spec");
  if (specJson != null) {
    try {
      spec = JSON.parse(specJson) as TestSpec;
    } catch {
      spec = null;
    }
  }
  return { variants, spec };
}

// ─────────────────────────────────────────────────────────────────────────────
// CONTENT HYDRATION — the description's REMAINING workbenches + fence groups
// ─────────────────────────────────────────────────────────────────────────────

/** Hydrate every in-place `div.workbench` under `root` (the first is already gone to the right
 *  pane), then its fence-group bars. Mirrors `execution::view::hydrate_workbenches` + the
 *  fence-group pass, minus the extraction. */
function hydrateContent(root: ParentNode, lessonPath: string[]): void {
  let count = 0;
  for (const element of root.querySelectorAll("div.workbench")) {
    const decoded = decodeWorkbench(element);
    if (!decoded) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(Workbench, { variants: decoded.variants, spec: decoded.spec, lessonPath, root: host }), host);
    count += 1;
  }
  // Diagrams, but NOT quiz/c4 — the docked description pane hydrates diagrams alongside its
  // workbenches/fence-groups and leaves quiz/c4 to the lesson body (islands/widgets'
  // whole-document pass, which stands down on this page).
  const diagrams = hydrateDiagrams(root);
  const groups = root.querySelectorAll("div.fence-group").length;
  hydrateFenceGroups(root);
  if (count > 0 || groups > 0 || diagrams > 0) {
    log.debug(`hydrated ${count} in-pane workbench(es), ${groups} fence group(s), ${diagrams} diagram(s)`);
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE RIGHT PANE — the extracted workbench + the anonymous sign-in bar
// ─────────────────────────────────────────────────────────────────────────────

function RightPane({
  variants,
  spec,
  lessonPath,
  root,
}: {
  variants: Variant[];
  spec: TestSpec | null;
  lessonPath: string[];
  root: HTMLElement;
}) {
  const [, setAuthTick] = useState(0);
  useEffect(() => {
    const onAuth = () => setAuthTick((n) => n + 1);
    window.addEventListener(AUTH_CHANGED, onAuth);
    return () => window.removeEventListener(AUTH_CHANGED, onAuth);
  }, []);
  const authed = isAuthed();
  return (
    <>
      {!authed && (
        <div class="wb__edit-bar" key="editbar">
          <span class="wb__edit-status">
            <span class="wb__edit-dot"></span>
            Sign in to edit and submit — you can still Run the starter
          </span>
        </div>
      )}
      <Workbench key="wb" variants={variants} spec={spec} lessonPath={lessonPath} root={root} fill={true} />
    </>
  );
}

/** Replace the right pane's "Loading the workbench…" placeholder with the live Workbench. The wrap
 *  div is the workbench ROOT (its event target) and the single flex child practice.css expects. */
function mountRightPane(
  pwb: HTMLElement,
  extracted: { variants: Variant[]; spec: TestSpec | null },
  lessonPath: string[],
): void {
  const rightPane = pwb.querySelector<HTMLElement>(".pwb__right");
  if (!rightPane) return;
  const wrap = document.createElement("div");
  rightPane.replaceChildren(wrap);
  workbenchRootEl = wrap;
  render(
    h(RightPane, { variants: extracted.variants, spec: extracted.spec, lessonPath, root: wrap }),
    wrap,
  );
  log.info(`workbench mounted in the right pane (${extracted.variants.map((v) => v.language).join("/")})`);
}

// ─────────────────────────────────────────────────────────────────────────────
// THE SPLITTER — 28–64%, persisted as a bare width (pane.ts)
// ─────────────────────────────────────────────────────────────────────────────

function wireSplitter(pwb: HTMLElement): void {
  const left = pwb.querySelector<HTMLElement>(".pwb__left");
  const split = pwb.querySelector<HTMLElement>(".wb-split");
  const panes = pwb.querySelector<HTMLElement>(".pwb__panes");
  if (!left || !split || !panes) return;

  const apply = (pct: number) => {
    left.style.width = `${pct.toFixed(2)}%`;
  };
  apply(parseLeftPct(storageGet(PROBLEM_PANE_KEY)));

  let dragging = false;
  split.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    dragging = true;
  });
  window.addEventListener("pointermove", (event) => {
    if (!dragging) return;
    const rect = panes.getBoundingClientRect();
    if (rect.width <= 0) return;
    const pct = ((event.clientX - rect.left) / rect.width) * 100;
    apply(Math.min(Math.max(pct, MIN_LEFT_PCT), MAX_LEFT_PCT));
  });
  // Persist on RELEASE, not on move — this fires for every window pointerup, so it is gated on
  // `dragging` rather than writing storage at pointer rate.
  window.addEventListener("pointerup", () => {
    if (!dragging) return;
    dragging = false;
    const pct = parseFloat(left.style.width) || DEFAULT_LEFT_PCT;
    storageSet(PROBLEM_PANE_KEY, serializeLeftPct(pct));
    log.debug(`splitter pinned → ${serializeLeftPct(pct)}%`);
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// THE TABS — Description | Editorial | Submissions, mount-once (always opens on Description)
// ─────────────────────────────────────────────────────────────────────────────

function wireTabs(pwb: HTMLElement, lessonPath: string[], spec: TestSpec | null): void {
  const buttons = Array.from(pwb.querySelectorAll<HTMLElement>(".problem-tab[data-tab]"));
  const panes = Array.from(pwb.querySelectorAll<HTMLElement>(".pwb__pane[data-pane]"));
  const seen = new Set<string>();

  // Editorial: the stepper island renders itself into the host on first open, parsing the raw
  // editorial markdown the SSR carried on `data-editorial` and hydrating GATED solution viewers
  // whose Copy-to-editor targets the right pane's workbench (via the `workbenchRoot` getter).
  const onFirstOpen = (tab: string): void => {
    if (seen.has(tab)) return;
    seen.add(tab);
    if (tab === "editorial") {
      const host = pwb.querySelector<HTMLElement>('[data-pane="editorial"] .pwb-editorial-host');
      if (!host) return;
      const md = decodedAttr(host, "data-editorial") ?? "";
      host.replaceChildren();
      render(h(EditorialPane, { md, workbenchRoot }), host);
      log.debug(`editorial stepper mounted (${md.length} chars)`);
    } else if (tab === "coach") {
      const host = pwb.querySelector<HTMLElement>('[data-pane="coach"] .pwb-coach-host');
      if (!host) return;
      render(h(CoachPane, { problem: lessonPath.join("/"), workbenchRoot }), host);
      log.debug("coach pane mounted");
    } else if (tab === "submissions") {
      const host = pwb.querySelector<HTMLElement>('[data-pane="submissions"] .psub-host');
      if (host) render(h(SubmissionsFeed, { path: lessonPath, spec, workbenchRoot }), host);
    }
  };

  const activate = (tab: string): void => {
    for (const button of buttons) button.classList.toggle("problem-tab--active", button.dataset.tab === tab);
    for (const pane of panes) pane.classList.toggle("hidden", pane.dataset.pane !== tab);
    onFirstOpen(tab);
    // A revealed pane may hold a Monaco that was measured while hidden — nudge it to re-layout.
    window.dispatchEvent(new Event(RELAYOUT));
    log.info(`problem tab → ${tab}`);
  };

  for (const button of buttons) {
    const tab = button.dataset.tab;
    if (!tab) continue;
    button.addEventListener("click", () => activate(tab));
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE CONTENTS PILL — opens the reader's nav drawer (reader.ts listens for the event)
// ─────────────────────────────────────────────────────────────────────────────

function wireContents(pwb: HTMLElement): void {
  const button = pwb.querySelector<HTMLElement>(".pwb__contents");
  if (!button) return;
  button.addEventListener("click", () => {
    log.debug("contents pill → open drawer");
    window.dispatchEvent(new Event(OPEN_CONTENTS));
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// SUBMITTED → the feed refetches (it listens on document itself); this is only for the log flow
// ─────────────────────────────────────────────────────────────────────────────

function init(): void {
  const pwb = document.querySelector<HTMLElement>(".pwb[data-problem]");
  if (!pwb) return;
  const lessonPath = lessonPathFromUrl();
  log.info(`problem page — /${lessonPath.join("/")}`);

  wireSplitter(pwb);
  wireContents(pwb);

  // Extract the FIRST description workbench into the right pane, hydrate the rest in place.
  const description = pwb.querySelector<HTMLElement>(".pwb-description");
  let extracted: { variants: Variant[]; spec: TestSpec | null } | null = null;
  if (description) {
    const first = description.querySelector("div.workbench");
    if (first) {
      extracted = decodeWorkbench(first);
      if (extracted) first.remove();
    }
    hydrateContent(description, lessonPath);
  }
  if (extracted) {
    mountRightPane(pwb, extracted, lessonPath);
  } else {
    const rightPane = pwb.querySelector<HTMLElement>(".pwb__right");
    if (rightPane) rightPane.innerHTML = '<div class="pwb__nowb">No runnable block in this problem.</div>';
    log.warn("problem page has no workbench to extract");
  }

  // The Submissions feed listens for SUBMITTED itself; this line only keeps the flow followable.
  document.addEventListener(SUBMITTED, () => log.debug("submit completed (SUBMITTED bubbled to document)"));

  wireTabs(pwb, lessonPath, extracted?.spec ?? null);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
