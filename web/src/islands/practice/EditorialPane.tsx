/**
 * The problem page's Editorial pane: an approach STEPPER for multi-approach editorials (numbered
 * circles over a connector rail, per-approach complexities), a sticky JUMP bar with a scroll-spy
 * over one continuously scrolling document of numbered sections, the solution gated behind a
 * reveal card (collapses again on approach switch, by design), and the Complexity section
 * rendered as Time/Space cards when its prose parses.
 *
 * The pure half lives in `lib/catalog/editorial.ts` — this component only spends the parsed
 * `EditorialDoc`: fragments render through `renderLesson` per section (via `MarkdownPane`) and
 * hydrate GATED solutions. The active approach persists under its OWN key (`PROBLEM_APPROACH_KEY`).
 *
 * SCROLL THE PANE, NOT THE WINDOW: `scrollIntoView` walks scrollable ancestors and crept the page
 * 64px per jump; below the 1024px breakpoint the pane stops scrolling and the PAGE carries the
 * content, so the same math targets the window there.
 */
import { useMemo, useRef, useState } from "preact/hooks";

import { activeSection, complexityProse, parseEditorial, prettyO } from "../../lib/catalog/editorial";
import type { ApproachDoc, EditorialDoc, SectionDoc } from "../../lib/catalog/editorial";
import { sectionIndex } from "../../lib/catalog/pane";
import { PROBLEM_APPROACH_KEY, get as storageGet, set as storageSet } from "../../lib/storage";
import * as log from "../../lib/log";
import { MarkdownPane } from "./panes";

/** The spy threshold and the sections' `scroll-margin-top` are a pair: a section counts as active
 *  once its top passes 84px below the container top, and a jump lands it at 70px. */
const SPY_THRESHOLD_PX = 84.0;
const JUMP_OFFSET_PX = 70.0;

export interface EditorialPaneProps {
  md: string;
  workbenchRoot: () => HTMLElement | null;
}

export function EditorialPane({ md, workbenchRoot }: EditorialPaneProps) {
  const doc = useMemo<EditorialDoc>(() => parseEditorial(md), [md]);
  const approachLabels = useMemo(() => doc.approaches.map((a) => a.label), [doc]);
  const [active, setActive] = useState(() => sectionIndex(approachLabels, storageGet(PROBLEM_APPROACH_KEY) ?? ""));

  if (doc.approaches.length === 0) {
    return <p class="psub__note">No editorial yet for this problem.</p>;
  }

  const selectApproach = (i: number) => {
    if (i === active) return;
    setActive(i);
    storageSet(PROBLEM_APPROACH_KEY, approachLabels[i]!);
    log.debug(`editorial approach → ${approachLabels[i] || "(single)"}`);
  };

  const approach = doc.approaches[Math.min(active, doc.approaches.length - 1)]!;

  return (
    <div class="pwb-epane">
      {doc.multi ? (
        <ApproachStepper doc={doc} active={active} onSelect={selectApproach} />
      ) : (
        <SingleApproachBar doc={doc} />
      )}
      {/* Keyed by approach index: a switch mounts a FRESH scroll container, so it opens at the top
          (no manual scroll reset) and the gated solutions collapse again. */}
      <ApproachBody key={active} approach={approach} preamble={doc.preamble} workbenchRoot={workbenchRoot} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE SUB-HEAD STRIP — stepper (multi) / complexity bar (single)
// ─────────────────────────────────────────────────────────────────────────────

function ApproachStepper({ doc, active, onSelect }: { doc: EditorialDoc; active: number; onSelect: (i: number) => void }) {
  const count = doc.approaches.length;
  const inset = (50.0 / count).toFixed(2);
  return (
    <div class="pwb-estep">
      <div class="pwb-estep__head">
        <span class="pwb-estep__label">Approaches</span>
        <span class="pwb-estep__hint">brute → optimal</span>
      </div>
      <div class="pwb-estep__row">
        <div class="pwb-estep__rail" style={`left: ${inset}%; right: ${inset}%;`} aria-hidden="true"></div>
        {doc.approaches.map((a, i) => (
          <button class="pwb-estep__btn" onClick={() => onSelect(i)}>
            <span
              class={`pwb-estep__num${active === i ? " pwb-estep__num--active" : ""}${active > i ? " pwb-estep__num--done" : ""}`}
            >
              {i + 1}
            </span>
            <span class={`pwb-estep__name${active === i ? " pwb-estep__name--active" : ""}`}>{a.label}</span>
            <span class="pwb-estep__metrics">
              {a.time != null && (
                <span class={`pwb-estep__time${active === i ? " pwb-estep__time--active" : ""}`}>{prettyO(a.time)}</span>
              )}
              {a.space != null && <span class="pwb-estep__space">{`space ${prettyO(a.space)}`}</span>}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

function SingleApproachBar({ doc }: { doc: EditorialDoc }) {
  const { time, space } = doc.approaches[0]!;
  if (time == null && space == null) return null;
  return (
    <div class="pwb-ebar">
      <span class="pwb-ebar__id">
        <span class="pwb-ebar__star">★</span>
        <span class="pwb-ebar__name">Solution</span>
      </span>
      <span class="pwb-ebar__pills">
        {time != null && <span class="pwb-ebar__pill pwb-ebar__pill--time">{`time ${prettyO(time)}`}</span>}
        {space != null && <span class="pwb-ebar__pill pwb-ebar__pill--space">{`space ${prettyO(space)}`}</span>}
      </span>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE SCROLLING BODY — jump bar, spy, numbered sections
// ─────────────────────────────────────────────────────────────────────────────

function ApproachBody({
  approach,
  preamble,
  workbenchRoot,
}: {
  approach: ApproachDoc;
  preamble: string;
  workbenchRoot: () => HTMLElement | null;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [activeSec, setActiveSec] = useState(0);
  const pending = useRef(false);

  const labels = approach.sections.filter((s) => s.label !== "").map((s) => s.label);

  const onScroll = () => {
    if (pending.current) return;
    pending.current = true;
    requestAnimationFrame(() => {
      pending.current = false;
      const container = scrollRef.current;
      if (!container) return;
      const containerTop = container.getBoundingClientRect().top;
      const nodes = container.querySelectorAll("[data-esec]");
      if (nodes.length === 0) return;
      const tops = Array.from(nodes, (el) => el.getBoundingClientRect().top - containerTop);
      const next = activeSection(tops, SPY_THRESHOLD_PX);
      setActiveSec((cur) => (cur !== next ? next : cur));
    });
  };

  const jumpTo = (index: number) => {
    const container = scrollRef.current;
    if (container) scrollSectionIntoView(container, index);
    setActiveSec(index);
  };

  // Number the LABELED sections; unlabeled leading prose renders headerless above them.
  let number = 0;
  const solutionTag = approach.label;
  const claims: [string | null, string | null] = [approach.time, approach.space];

  return (
    <div class="pwb__pane-scroll synapse-prose pwb-escroll" ref={scrollRef} onScroll={onScroll}>
      {labels.length > 1 && <JumpBar labels={labels} active={activeSec} onJump={jumpTo} />}
      {preamble !== "" && (
        <MarkdownPane md={preamble} solutions="gated" forceOpenDetails={true} workbenchRoot={workbenchRoot} />
      )}
      {approach.sections.map((section) => {
        if (section.label === "") {
          return <MarkdownPane md={section.md} solutions="gated" forceOpenDetails={true} workbenchRoot={workbenchRoot} />;
        }
        const index = number++;
        const tag = section.kind === "Solution" && solutionTag !== "" ? solutionTag : null;
        return (
          <SectionBlock index={index} section={section} tag={tag} claims={claims} workbenchRoot={workbenchRoot} />
        );
      })}
    </div>
  );
}

function JumpBar({ labels, active, onJump }: { labels: string[]; active: number; onJump: (i: number) => void }) {
  return (
    <div class="pwb-ejump">
      <span class="pwb-ejump__label">Jump</span>
      {labels.map((label, i) => (
        <button class={`pwb-ejump__pill${active === i ? " pwb-ejump__pill--active" : ""}`} onClick={() => onJump(i)}>
          {label}
        </button>
      ))}
    </div>
  );
}

/**
 * Scrolls the PANE when the pane is the scroller (`scrollIntoView` walks every scrollable ancestor
 * and crept the page down ~60px per jump). Below the 1024px breakpoint the pane stops scrolling and
 * the PAGE carries the content, so the same math targets the window. The 70px offset lands the
 * section header just clear of the sticky jump bar, paired with the 84px spy threshold.
 */
function scrollSectionIntoView(container: HTMLElement, index: number): void {
  const section = container.querySelector(`[data-esec='${index}']`);
  if (!section) return;
  if (container.scrollHeight > container.clientHeight + 1) {
    const delta = section.getBoundingClientRect().top - container.getBoundingClientRect().top - JUMP_OFFSET_PX;
    container.scrollTo({ top: container.scrollTop + delta, behavior: "smooth" });
  } else {
    const top = window.scrollY + section.getBoundingClientRect().top - JUMP_OFFSET_PX;
    window.scrollTo(0, Math.max(top, 0));
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// ONE NUMBERED SECTION — Complexity renders as Time/Space cards when its prose parses
// ─────────────────────────────────────────────────────────────────────────────

function SectionBlock({
  index,
  section,
  tag,
  claims,
  workbenchRoot,
}: {
  index: number;
  section: SectionDoc;
  tag: string | null;
  claims: [string | null, string | null];
  workbenchRoot: () => HTMLElement | null;
}) {
  let body;
  if (section.kind === "Complexity") {
    const parsed = complexityProse(section.md);
    if (parsed) {
      const time = parsed.time ?? (claims[0] != null ? ([claims[0], ""] as [string, string]) : null);
      const space = parsed.space ?? (claims[1] != null ? ([claims[1], ""] as [string, string]) : null);
      body = <ComplexityCards time={time} space={space} />;
    } else {
      body = <MarkdownPane md={section.md} solutions="gated" forceOpenDetails={true} workbenchRoot={workbenchRoot} />;
    }
  } else {
    body = <MarkdownPane md={section.md} solutions="gated" forceOpenDetails={true} workbenchRoot={workbenchRoot} />;
  }
  return (
    <section class="pwb-esection" data-esec={String(index)}>
      <div class="pwb-esection__head">
        <span class="pwb-esection__no">{String(index + 1).padStart(2, "0")}</span>
        <h3 class="pwb-esection__title">{section.label}</h3>
        {tag != null && <span class="pwb-esection__tag">{tag}</span>}
      </div>
      {body}
    </section>
  );
}

function ComplexityCards({ time, space }: { time: [string, string] | null; space: [string, string] | null }) {
  return (
    <div class="pwb-ecx">
      {time != null && (
        <div class="pwb-ecx__card">
          <span class="pwb-ecx__kind">{CLOCK_ICON} Time</span>
          <span class="pwb-ecx__value pwb-ecx__value--time">{prettyO(time[0])}</span>
          {time[1] !== "" && <p class="pwb-ecx__prose">{time[1]}</p>}
        </div>
      )}
      {space != null && (
        <div class="pwb-ecx__card">
          <span class="pwb-ecx__kind">{GRID_ICON} Space</span>
          <span class="pwb-ecx__value pwb-ecx__value--space">{prettyO(space[0])}</span>
          {space[1] !== "" && <p class="pwb-ecx__prose">{space[1]}</p>}
        </div>
      )}
    </div>
  );
}

const CLOCK_ICON = (
  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <circle cx="12" cy="12" r="9"></circle>
    <path d="M12 7v5l3 2"></path>
  </svg>
);

const GRID_ICON = (
  <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <rect x="4" y="4" width="16" height="16" rx="2"></rect>
    <path d="M4 9h16M9 20V9"></path>
  </svg>
);
