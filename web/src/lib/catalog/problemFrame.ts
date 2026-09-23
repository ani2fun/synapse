// The problem page's frame — crumbs, the two panes with their tab bars, the splitter, the docked
// nav — as ONE HTML string, built from the lesson payload.
//
// It lives here rather than in the page's JSX because two renderers need the same markup: the
// server-rendered problem page, and the private lesson island, which builds a problem page in the
// browser once the session settles (a private book is never server-rendered). `islands/problem`
// hydrates whatever carries `.pwb[data-problem]`, so as long as both produce THIS frame, the
// workbench, the tabs, the canvas and the submissions feed behave the same on either path. The
// hidden Contents-drawer source and the docked nav's book counter come from the caller, which is
// the one that has the index.
//
// Text from the payload is escaped here; the description HTML is trusted — it is the output of
// the reader's own markdown pipeline.

export interface ProblemCounter {
  problems: string[];
  at: number;
}

export interface ProblemFrameInput {
  title: string;
  lede: string | null;
  bookName: string;
  difficulty: string | null;
  /** The description markdown, already rendered. */
  descriptionHtml: string;
  /** The RAW editorial markdown — the stepper island parses it on first open. */
  editorialMd: string;
  /** The SAMPLE suite, JSON-serialisable, or null when the lesson has none. */
  tests: unknown | null;
  counter: ProblemCounter | null;
  prev: string | null;
  next: string | null;
  /** The left pane's starting width, in percent. */
  leftPct: number;
}

/** `count-all-digits` → `Count All Digits`. */
export function humanize(slug: string): string {
  return slug
    .split("-")
    .map((w) => (w === "" ? "" : w[0].toUpperCase() + w.slice(1)))
    .join(" ");
}

function esc(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

const ICON = (cls: string, inner: string) =>
  `<svg class="${cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${inner}</svg>`;

const ICONS = {
  plan: ICON("pwb__plan-ic", '<path d="M9 18h6 M10 22h4 M12 2a7 7 0 0 0-4 12.7V17h8v-2.3A7 7 0 0 0 12 2z" />'),
  description: ICON(
    "problem-tab__ic",
    '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z" /><path d="M14 2v5h5 M16 13H8 M16 17H8 M10 9H8" />',
  ),
  editorial: ICON(
    "problem-tab__ic",
    '<path d="M12 7v14 M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4" /><path d="M21 18a1 1 0 0 0 1-1V4a1 1 0 0 0-1-1h-5a4 4 0 0 0-4 4" />',
  ),
  coach: ICON("problem-tab__ic", '<path d="M22 10v6 M2 10l10-5 10 5-10 5z" /><path d="M6 12v5c3 3 9 3 12 0v-5" />'),
  submissions: ICON("problem-tab__ic", '<path d="M3 3v5h5" /><path d="M3.05 13A9 9 0 1 0 6 5.3L3 8 M12 7v5l4 2" />'),
  think: ICON(
    "pwb__rtab-ic",
    '<path d="M12 5a3 3 0 0 0-3-3 2.5 2.5 0 0 0-2.5 2.5A2.5 2.5 0 0 0 4 7c0 1 .4 1.7 1 2.2A2.7 2.7 0 0 0 4 11.5c0 1 .5 1.9 1.3 2.4A2.6 2.6 0 0 0 5 15.5C5 17 6.2 18 7.7 18H9a3 3 0 0 0 3-3z" /><path d="M12 5a3 3 0 0 1 3-3 2.5 2.5 0 0 1 2.5 2.5A2.5 2.5 0 0 1 20 7c0 1-.4 1.7-1 2.2a2.7 2.7 0 0 1 1 2.3c0 1-.5 1.9-1.3 2.4.2.4.3.9.3 1.6 0 1.5-1.2 2.5-2.7 2.5H15a3 3 0 0 1-3-3z" /><path d="M12 18v4" />',
  ),
  code: ICON("pwb__rtab-ic", '<path d="M16 18l6-6-6-6 M8 6l-6 6 6 6" />'),
  pin: ICON(
    "pwb__rpin-ic",
    '<path d="M12 17v5" /><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z" />',
  ),
  contents: ICON("pwb__contents-ic", '<rect width="18" height="18" x="3" y="3" rx="2" /><path d="M9 3v18 M14 9l3 3-3 3" />'),
};

function tab(kind: keyof typeof ICONS, label: string, active: boolean): string {
  return (
    `<button class="problem-tab problem-tab--${kind}${active ? " problem-tab--active" : ""}" data-tab="${kind}" type="button">` +
    `${ICONS[kind]}${label}</button>`
  );
}

function step(target: string | null, label: string, next: boolean): string {
  if (!target) return "";
  const title = esc(humanize(target.split("/").pop() ?? target));
  const text = `<span class="pwb__step-text"><span class="pwb__step-label">${label}</span><span class="pwb__step-title">${title}</span></span>`;
  return next
    ? `<a class="pwb__step pwb__step--next" href="/synapse/${esc(target)}">${text}<span class="pwb__step-chev">›</span></a>`
    : `<a class="pwb__step pwb__step--prev" href="/synapse/${esc(target)}"><span class="pwb__step-chev">‹</span>${text}</a>`;
}

function counterNav(counter: ProblemCounter | null): string {
  if (!counter) return "";
  const dots = counter.problems
    .map(
      (p, i) =>
        `<a class="pwb__dot${i === counter.at ? " pwb__dot--current" : ""}" href="/synapse/${esc(p)}" title="${esc(humanize(p.split("/").pop() ?? p))}" aria-label="Problem ${i + 1}"></a>`,
    )
    .join("");
  return `<span class="pwb__nav-count">Problem ${counter.at + 1} / ${counter.problems.length}</span><div class="pwb__nav-dots">${dots}</div>`;
}

/**
 * The `.pwb[data-problem]` frame. `data-sample-tests` carries the SAMPLE testcases (server-
 * filtered from the tests.json sidecar; hidden judge cases never ship) and `data-editorial` the
 * raw editorial markdown — both URL-encoded, both read by `islands/problem`.
 */
export function problemFrame(input: ProblemFrameInput): string {
  const tests = input.tests ? encodeURIComponent(JSON.stringify(input.tests)) : "";
  const editorial = input.editorialMd ? encodeURIComponent(input.editorialMd) : "";
  const lede = input.lede ? `<p class="pwb__lede">${esc(input.lede)}</p>` : "";
  const difficulty = input.difficulty
    ? `<span class="problem-diff problem-diff--${esc(input.difficulty)}">${esc(input.difficulty)}</span>`
    : "";
  return (
    `<div class="pwb not-prose" data-problem data-sample-tests="${tests}">` +
    `<nav class="pwb__crumbs" aria-label="Breadcrumb">` +
    `<a class="pwb__crumb" href="/">Home</a><span class="pwb__crumb-sep">›</span>` +
    `<span class="pwb__crumb">${esc(input.bookName)}</span><span class="pwb__crumb-sep">›</span>` +
    `<span class="pwb__crumb pwb__crumb--current">${esc(input.title)}</span></nav>` +
    `<div class="pwb__panes">` +
    `<div class="pwb__left" style="width: ${input.leftPct}%">` +
    `<div class="pwb__head"><div class="pwb__head-row"><h1 class="pwb__title">${esc(input.title)}</h1>` +
    `<span class="pwb__plan-pill">${ICONS.plan}<strong>Think</strong> before you code.</span></div>${lede}</div>` +
    `<div class="problem-tabs">` +
    tab("description", "Description", true) +
    tab("editorial", "Editorial", false) +
    tab("coach", "Coach", false) +
    tab("submissions", "Submissions", false) +
    `${difficulty}</div>` +
    `<div class="pwb__pane-host">` +
    `<div class="pwb__pane" data-pane="description"><div class="pwb__pane-scroll synapse-prose"><div class="pwb-description">${input.descriptionHtml}</div></div></div>` +
    // No `.pwb__pane-scroll` wrapper on the editorial pane: the stepper island renders its OWN
    // `.pwb-epane > .pwb__pane-scroll` into this host on first open.
    `<div class="pwb__pane hidden" data-pane="editorial"><div class="pwb-editorial-host" data-editorial="${editorial}"></div></div>` +
    `<div class="pwb__pane hidden" data-pane="coach"><div class="pwb__pane-scroll synapse-prose"><div class="pwb-coach-host"></div></div></div>` +
    `<div class="pwb__pane hidden" data-pane="submissions"><div class="pwb__pane-scroll synapse-prose"><div class="psub-host"></div></div></div>` +
    `</div></div>` +
    `<div class="wb-split" aria-label="Resize the panes"><div class="wb-split__grip"><span></span><span></span><span></span></div></div>` +
    // Think before Code, in that order, and Think is the tab that OPENS — the plan comes before
    // the typing. The pin is the only thing that writes the preference; a tab click is a visit.
    `<div class="pwb__right">` +
    `<div class="pwb__rtabs" role="tablist" aria-label="Workbench mode">` +
    `<button class="pwb__rtab pwb__rtab--think pwb__rtab--active" data-rtab="think" type="button">${ICONS.think}Think</button>` +
    `<button class="pwb__rtab pwb__rtab--code" data-rtab="code" type="button">${ICONS.code}Code</button>` +
    `<span class="pwb__rtabs-spacer"></span>` +
    `<button class="pwb__rpin" data-rpin type="button" aria-pressed="false">${ICONS.pin}<span class="pwb__rpin-text" data-rpin-text>Default</span></button>` +
    `</div>` +
    `<div class="pwb__rpane-host">` +
    `<div class="pwb__rpane" data-rpane="think"><div class="pcanvas-host"></div></div>` +
    `<div class="pwb__rpane hidden" data-rpane="code"><div class="pwb__nowb">Loading the workbench…</div></div>` +
    `</div></div></div>` +
    `<nav class="pwb__nav" aria-label="Problem navigation">` +
    `<div class="pwb__nav-left"><button class="pwb__contents" aria-label="Contents — the book's lessons and problems" type="button">${ICONS.contents}<span>Contents</span></button></div>` +
    `<div class="pwb__nav-mid">${counterNav(input.counter)}</div>` +
    `<div class="pwb__nav-right">${step(input.prev, "Previous", false)}${step(input.next, "Next", true)}</div>` +
    `</nav></div>`
  );
}
