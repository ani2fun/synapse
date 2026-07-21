/**
 * The embedded practice-problem widget: a two-pane `.pwb--embedded` card inline at the
 * reading-column width — Description/Editorial tabs on the left (the editorial splitting into
 * Brute Force / Optimal approach tabs when authored), the reused workbench (Run only — no
 * Submit) on the right, a draggable 9px splitter (28–64%), and an Enlarge toggle that
 * CSS-promotes the SAME live panes to a near-fullscreen modal (Monaco and all state survive).
 * Copy-to-editor in a revealed solution lands in the workbench tab MATCHING the solution's
 * language.
 *
 * The Workbench is mounted IMPERATIVELY into the right pane (as problem.tsx does): its `root` is
 * the event surface LOAD_CODE/USE_CASE target, so the editorial's SolutionViewers reach it
 * through a getter (`workbenchRoot`) rather than a shared signal — the `workbench/contracts.ts`
 * contract.
 */
import { render, h } from "preact";
import { useEffect, useMemo, useRef, useState } from "preact/hooks";

import type { PracticeSpec } from "../../lib/execution/practice";
import { Workbench } from "../workbench/Workbench";
import { MarkdownPane } from "./panes";
import * as log from "../../lib/log";

const BOOK_ICON = (
  <svg class="problem-tab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"></path>
    <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"></path>
  </svg>
);

const BULB_ICON = (
  <svg class="problem-tab__ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M15 14c.2-1 .7-1.7 1.5-2.5a6 6 0 1 0-9 0c.8.8 1.3 1.5 1.5 2.5"></path>
    <path d="M9 18h6"></path>
    <path d="M10 22h4"></path>
  </svg>
);

export function PracticeProblem({ spec, title, lessonPath }: { spec: PracticeSpec; title: string; lessonPath: string[] }) {
  const [expanded, setExpanded] = useState(false);
  // 0 = Description; 1.. = the editorial approaches.
  const [tab, setTab] = useState(0);
  // panes 0..seen have mounted (editorials are lazy).
  const [seen, setSeen] = useState(1);
  const [leftPct, setLeftPct] = useState(46.0);

  const panesRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);
  const wbRoot = useRef<HTMLElement | null>(null);
  const dragging = useRef(false);
  const workbenchRoot = useMemo(() => () => wbRoot.current, []);

  // The workbench, mounted imperatively into the right pane (its `root` is the event surface). Done
  // once; Preact renders an empty `.pwb__right` and this fills it, so the two never fight.
  useEffect(() => {
    const pane = rightRef.current;
    if (!pane) return;
    const wrap = document.createElement("div");
    pane.replaceChildren(wrap);
    wbRoot.current = wrap;
    render(
      h(Workbench, { variants: spec.variants, spec: spec.spec, lessonPath, root: wrap, practice: true }),
      wrap,
    );
    log.info(`practice widget "${title}" — workbench (${spec.variants.map((v) => v.language).join("/")})`);
    return () => {
      render(null, wrap);
      wbRoot.current = null;
    };
  }, []);

  // Escape collapses only an actually-open modal (per instance — widgets don't interfere).
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && expanded) setExpanded(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [expanded]);

  // The splitter drag: document-level move/up so the pointer can outrun the 9px rail.
  useEffect(() => {
    const move = (event: PointerEvent) => {
      if (!dragging.current) return;
      const panes = panesRef.current;
      if (!panes) return;
      const rect = panes.getBoundingClientRect();
      if (rect.width <= 0) return;
      const pct = ((event.clientX - rect.left) / rect.width) * 100;
      setLeftPct(Math.min(Math.max(pct, 28.0), 64.0));
    };
    const up = () => (dragging.current = false);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  }, []);

  const select = (paneIndex: number) => {
    setTab(paneIndex);
    setSeen((s) => Math.max(s, paneIndex + 1));
  };

  return (
    <div class={`pwb pwb--embedded${expanded ? " pwb--expanded" : ""}`}>
      <div class="pwb__scrim" onClick={() => setExpanded(false)}></div>
      <div class="pwb__panes" ref={panesRef}>
        <div class="pwb__left" style={`width: ${leftPct.toFixed(2)}%`}>
          <div class="pwb__head">
            <span class="pwb__badge">PRACTICE</span>
            <div class="pwb__title">{title}</div>
          </div>
          <div class="problem-tabs">
            <button class={`problem-tab${tab === 0 ? " problem-tab--active" : ""}`} onClick={() => select(0)}>
              {BOOK_ICON}
              Description
            </button>
            {spec.editorials.map((approach, i) => (
              <button
                class={`problem-tab${tab === i + 1 ? " problem-tab--active" : ""}`}
                onClick={() => select(i + 1)}
              >
                {BULB_ICON}
                {approach.label}
              </button>
            ))}
          </div>
          <div class="pwb__pane-scroll">
            <div class={`pwb__pane${tab !== 0 ? " hidden" : ""}`}>
              <MarkdownPane md={spec.problemMd} solutions="none" forceOpenDetails={false} workbenchRoot={workbenchRoot} />
            </div>
            {/* Lazy: each editorial approach (and its Monaco solution viewers) mounts on first open,
                then only toggles visibility. */}
            {spec.editorials.map((approach, i) =>
              seen > i + 1 ? (
                <div class={`pwb__pane${tab !== i + 1 ? " hidden" : ""}`}>
                  <MarkdownPane
                    md={approach.md}
                    solutions="revealed"
                    forceOpenDetails={false}
                    workbenchRoot={workbenchRoot}
                  />
                </div>
              ) : null,
            )}
          </div>
        </div>
        <div class="wb-split" onPointerDown={(event) => { event.preventDefault(); dragging.current = true; }}>
          <div class="wb-split__grip">
            <span></span>
            <span></span>
            <span></span>
          </div>
        </div>
        <div class="pwb__right" ref={rightRef}></div>
        <button
          class="pwb__enlarge"
          aria-label={expanded ? "Close fullscreen" : "Enlarge to fullscreen"}
          onClick={() => setExpanded((e) => !e)}
        >
          {expanded ? "✕ Close" : "⤢ Enlarge"}
        </button>
      </div>
    </div>
  );
}
