/**
 * The canvas's pure half: the shape of a body, and everything the UI DERIVES from one.
 *
 * An entry's title, its filled-area count and its best complexity are computed here rather than
 * stored — a saved copy of a derived value is a copy that can disagree with the body it claims to
 * describe. The same functions drive the header meter and the saved-entries table, so a row can
 * never report a different reading of an entry than the form does.
 */
import type { CanvasBodyWire, CanvasEntry, CanvasIdeaWire } from "../../lib/api/client";

/** The eight areas, in canvas order. `ideas` is the ninth thing on the page but not a text area,
 *  so it counts separately everywhere below. */
export const AREAS = ["problem", "constraints", "maintenance", "inputs", "ret", "errors", "tests"] as const;
export type Area = (typeof AREAS)[number];

/** Areas + Ideas — what the meter reads out of. */
export const TOTAL_AREAS = AREAS.length + 1;

export interface Idea {
  /** Stable only within a session: the list is keyed on it, and reordering must not remount rows. */
  id: string;
  name: string;
  desc: string;
  time: string;
  space: string;
}

/** The local, TOTAL body. The wire DTO has every field optional (the server accepts a half-filled
 *  canvas, which is the normal state of one being worked on), so it is normalised once on the way
 *  in and nothing downstream repeats `?? ""` eight times. */
export interface CanvasBody {
  problem: string;
  constraints: string;
  maintenance: string;
  inputs: string;
  ret: string;
  errors: string;
  tests: string;
  ideas: Idea[];
}

let ideaSeq = 0;
export function newIdeaId(): string {
  ideaSeq += 1;
  return `i${ideaSeq}-${Date.now()}`;
}

/** A fresh canvas: empty areas and the two ideas the method starts from — brute force first,
 *  then the refined one. Pre-naming them is the prompt; a blank list would not say to write two. */
export function blankBody(): CanvasBody {
  return {
    problem: "",
    constraints: "",
    maintenance: "",
    inputs: "",
    ret: "",
    errors: "",
    tests: "",
    ideas: [
      { id: newIdeaId(), name: "Brute force", desc: "", time: "", space: "" },
      { id: newIdeaId(), name: "Optimized", desc: "", time: "", space: "" },
    ],
  };
}

function text(value: string | undefined): string {
  return typeof value === "string" ? value : "";
}

/** Wire → local. Tolerant by construction: an absent field, a null, or a body written by an older
 *  build all normalise to the empty canvas rather than throwing. */
export function normalizeBody(wire: CanvasBodyWire | null | undefined): CanvasBody {
  const ideas = (wire?.ideas ?? []).map((idea: CanvasIdeaWire) => ({
    id: newIdeaId(),
    name: text(idea?.name),
    desc: text(idea?.description),
    time: text(idea?.time),
    space: text(idea?.space),
  }));
  return {
    problem: text(wire?.problem),
    constraints: text(wire?.constraints),
    maintenance: text(wire?.maintenance),
    inputs: text(wire?.inputs),
    ret: text(wire?.return),
    errors: text(wire?.errors),
    tests: text(wire?.tests),
    ideas: ideas.length > 0 ? ideas : blankBody().ideas,
  };
}

/** Local → wire. `ret` is `return` on the wire (a Rust keyword on the other side); the ids are
 *  session-scoped and deliberately do not travel. */
export function toWire(body: CanvasBody): CanvasBodyWire {
  return {
    problem: body.problem,
    constraints: body.constraints,
    maintenance: body.maintenance,
    inputs: body.inputs,
    return: body.ret,
    errors: body.errors,
    tests: body.tests,
    ideas: body.ideas.map((idea) => ({
      name: idea.name,
      description: idea.desc,
      time: idea.time,
      space: idea.space,
    })),
  };
}

/** How many of the nine areas carry anything. Ideas counts as ONE, and only when an idea has a
 *  DESCRIPTION — the two starter rows ship with names already filled, so counting names would
 *  show a brand-new canvas as 1/8 done before the reader has typed. */
export function filledCount(body: CanvasBody): number {
  let n = 0;
  for (const area of AREAS) if (body[area].trim() !== "") n += 1;
  if (body.ideas.some((idea) => idea.desc.trim() !== "")) n += 1;
  return n;
}

/** The complexity the canvas landed on: the LAST idea carrying a time, because the canvas is
 *  written brute-force-first and refined downward. `—` when no idea names one yet. */
export function bestComplexity(body: CanvasBody): string {
  const timed = body.ideas.filter((idea) => idea.time.trim() !== "");
  const last = timed[timed.length - 1];
  if (!last) return "—";
  return `${last.time} / ${last.space.trim() === "" ? "?" : last.space}`;
}

/** An entry's display name: the first line of Problem, trimmed to fit a table cell. The canvas
 *  asks for a one-sentence restatement there, so that line is already the best title available. */
export function entryTitle(body: CanvasBody): string {
  const first = body.problem.split("\n")[0]?.trim() ?? "";
  return first === "" ? "Untitled canvas" : first.slice(0, 72);
}

/** `true` when nothing has been written — Save refuses, rather than storing an empty snapshot. */
export function isBlank(body: CanvasBody): boolean {
  return filledCount(body) === 0;
}

// ─────────────────────────────────────────────────────────────────────────────
// EXPORT — the published JSON shape
// ─────────────────────────────────────────────────────────────────────────────

/** The export envelope. `schema` is a promise to whoever reads the file later: the version is
 *  what lets a consumer know which shape it is holding. */
export interface ExportEnvelope {
  schema: "algorithm-design-canvas/v1";
  problem: string;
  problemPath: string;
  exportedAt: string;
  draft?: CanvasBodyWire;
  entries?: CanvasEntry[];
}

export function exportEnvelope(
  problem: string,
  path: string[],
  contents: { draft?: CanvasBody; entries?: CanvasEntry[] },
): ExportEnvelope {
  return {
    schema: "algorithm-design-canvas/v1",
    problem,
    problemPath: path.join("/"),
    exportedAt: new Date().toISOString(),
    ...(contents.draft ? { draft: toWire(contents.draft) } : {}),
    ...(contents.entries ? { entries: contents.entries } : {}),
  };
}

/** A filename stem for a download: the problem's last segment, or `canvas`. */
export function fileStem(path: string[]): string {
  const last = path[path.length - 1] ?? "";
  const slug = last.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  return slug === "" ? "canvas" : slug;
}
