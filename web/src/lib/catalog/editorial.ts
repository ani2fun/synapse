/**
 * The editorial document model (pure half of the Editorial tab). One markdown string in — the
 * sidecar or the inline `<details>` tail, they share a shape — and a typed `EditorialDoc` out:
 * approaches for the stepper, sections for the jump bar, complexity claims for the cards.
 *
 * Two authored formats exist. Type 1 is flat (`## Intuition` / `## Approach` / `## Solution` /
 * `## Complexity Analysis`); type 2 nests the same set as `###` subsections under one `##` heading
 * PER APPROACH (`## Brute Force`, `## Optimal 1`, …). Both are parsed by the same splitter; `multi`
 * is DETECTED, never declared. Anything else — arbitrary headings, plain fences, no headings at all
 * — degrades to a plain sectioned document and the view shows no stepper or cards.
 */

import { normalizeLabel } from "./pane";
import { solutionComplexities } from "../execution/practice";

export type SectionKind = "Intuition" | "Approach" | "Solution" | "Complexity" | "Other";

/** One rendered section: the author's heading (empty for an approach's leading prose) and its body
 *  WITHOUT the heading line — the view renders its own numbered header. */
export interface SectionDoc {
  label: string;
  kind: SectionKind;
  md: string;
}

/** One approach: its heading label ("" for the single format), the complexity claims from its first
 *  `solution` fence meta, and its sections in authoring order. */
export interface ApproachDoc {
  label: string;
  time: string | null;
  space: string | null;
  sections: SectionDoc[];
}

export interface EditorialDoc {
  /** Prose before the first heading — always visible, above the jump bar's sections. */
  preamble: string;
  /** One entry for the single format, two or more when the stepper applies. */
  approaches: ApproachDoc[];
  /** Whether `approaches` came from top-level `##` approach headings. */
  multi: boolean;
}

/** One complexity card's content: the O-value and its explanation prose. */
export interface ComplexityProse {
  time: [string, string] | null;
  space: [string, string] | null;
}

// ─────────────────────────────────────────────────────────────────────────────
// PARSE
// ─────────────────────────────────────────────────────────────────────────────

export function parseEditorial(md: string): EditorialDoc {
  md = stripSpoilerWrapper(md);
  if (md.trim() === "") {
    return { preamble: "", approaches: [], multi: false };
  }

  let preamble: string;
  let top: [string, string][];
  {
    const [pre, sections] = splitAtHeadings(md, "## ");
    if (sections.length === 0) {
      [preamble, top] = splitAtHeadings(md, "# ");
    } else {
      preamble = pre;
      top = sections;
    }
  }

  if (top.length === 0) {
    // No headings at all: one bare approach, everything stays in the preamble.
    const [time, space] = complexityClaims(firstSolutionMeta(md));
    return {
      preamble: md.trim(),
      approaches: [{ label: "", time, space, sections: [] }],
      multi: false,
    };
  }

  const multi = top.length >= 2 && top.some(([, body]) => hasCanonicalSubheading(body));
  let approaches: ApproachDoc[];
  if (multi) {
    approaches = top.map(([label, body]) => {
      const [leading, subs] = splitAtHeadings(body, "### ");
      const sections: SectionDoc[] = [];
      if (leading !== "") sections.push({ label: "", kind: "Other", md: leading });
      sections.push(...subs.map(section));
      return buildApproach(label, sections);
    });
  } else {
    approaches = [buildApproach("", top.map(section))];
  }
  return { preamble, approaches, multi };
}

/**
 * The canonical kind behind a heading, via the same normalisation the remembered-section matcher
 * uses. `startsWith` on complexity covers both "Complexity" and "Complexity Analysis".
 */
export function sectionKind(label: string): SectionKind {
  const norm = normalizeLabel(label);
  switch (norm) {
    case "intuition":
      return "Intuition";
    case "approach":
      return "Approach";
    case "solution":
    case "solutions":
    case "code":
      return "Solution";
    default:
      return norm.startsWith("complexity") ? "Complexity" : "Other";
  }
}

function section([label, md]: [string, string]): SectionDoc {
  return { label, kind: sectionKind(label), md };
}

function buildApproach(label: string, sections: SectionDoc[]): ApproachDoc {
  synthesizeSolution(sections);
  let meta: string | null = null;
  for (const s of sections) {
    const m = firstSolutionMeta(s.md);
    if (m !== null) {
      meta = m;
      break;
    }
  }
  const [time, space] = complexityClaims(meta);
  return { label, time, space, sections };
}

/**
 * Fence-aware split at line-start heading markers. The heading line is consumed into the label;
 * bodies and the preamble come back trimmed.
 */
function splitAtHeadings(md: string, marker: string): [string, [string, string][]] {
  let inFence = false;
  const preamble: string[] = [];
  const sections: [string, string[]][] = [];
  for (const line of md.split("\n")) {
    if (line.trimStart().startsWith("```")) {
      inFence = !inFence;
    } else if (!inFence && line.startsWith(marker)) {
      sections.push([line.slice(marker.length).trim(), []]);
      continue;
    }
    const last = sections[sections.length - 1];
    if (last) last[1].push(line);
    else preamble.push(line);
  }
  return [
    preamble.join("\n").trim(),
    sections.map(([label, body]): [string, string] => [label, body.join("\n").trim()]),
  ];
}

/** Whether a top-level section body carries a canonical `###` subsection — the multi-approach
 *  discriminator (an approach heading is free text; its CONTENTS are the recognisable part). */
function hasCanonicalSubheading(body: string): boolean {
  const [, subs] = splitAtHeadings(body, "### ");
  return subs.some(([label]) => sectionKind(label) !== "Other");
}

/**
 * The inline editorial arrives still wearing its spoiler wrapper (`problemContentSplit` cuts AT the
 * `<details` line). Per-section fragment rendering would smear that unbalanced pair across
 * fragments, so the OUTER wrapper — and only it — comes off here: the opening line, its `<summary>`
 * line, and the last fence-outside `</details>`. Nested details deeper in the document are content
 * and survive.
 */
function stripSpoilerWrapper(md: string): string {
  const trimmed = md.trim();
  if (!trimmed.startsWith("<details")) return md;
  const lines = trimmed.split("\n");
  lines.shift();
  const first = lines.findIndex((line) => line.trim() !== "");
  if (first !== -1) {
    const head = lines[first]!.trim();
    if (head.startsWith("<summary") && head.endsWith("</summary>")) lines.splice(first, 1);
  }
  let inFence = false;
  let lastClose: number | null = null;
  lines.forEach((line, i) => {
    if (line.trimStart().startsWith("```")) inFence = !inFence;
    else if (!inFence && line.trim() === "</details>") lastClose = i;
  });
  if (lastClose !== null) lines.splice(lastClose, 1);
  return lines.join("\n").trim();
}

// ─────────────────────────────────────────────────────────────────────────────
// SOLUTION FENCES — the synthesized section and the complexity claims
// ─────────────────────────────────────────────────────────────────────────────

/**
 * The verified type-2 shape has NO `### Solution` heading — the fences sit at the tail of
 * `### Approach`. When no Solution section exists but a `solution` fence does, the owning section
 * splits at the first fence line and the tail becomes a synthetic Solution section right after it.
 * An explicit Solution heading suppresses this entirely.
 */
function synthesizeSolution(sections: SectionDoc[]): void {
  if (sections.some((s) => s.kind === "Solution")) return;
  for (let i = 0; i < sections.length; i++) {
    const fenceLine = solutionFenceStart(sections[i]!.md);
    if (fenceLine === null) continue;
    const lines = sections[i]!.md.split("\n");
    const head = lines.slice(0, fenceLine).join("\n").trim();
    const tail = lines.slice(fenceLine).join("\n").trim();
    const solution: SectionDoc = { label: "Solution", kind: "Solution", md: tail };
    if (head === "") {
      sections[i] = solution;
    } else {
      sections[i]!.md = head;
      sections.splice(i + 1, 0, solution);
    }
    return;
  }
}

/** Line index of the first fence OPENING whose info string carries the whitespace-delimited
 *  `solution` token (the same predicate render.ts groups on). Plain fences don't count. */
function solutionFenceStart(md: string): number | null {
  let inFence = false;
  const lines = md.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const t = lines[i]!.trimStart();
    if (t.startsWith("```")) {
      if (!inFence) {
        const info = t.replace(/^`+/, "").trim();
        if (info.split(/\s+/).some((word) => word === "solution")) return i;
      }
      inFence = !inFence;
    }
  }
  return null;
}

/** The full info string of the first `solution` fence, e.g. `python solution time=O(N) space=O(K)`
 *  — `solutionComplexities` reads the claims out. */
export function firstSolutionMeta(md: string): string | null {
  const at = solutionFenceStart(md);
  if (at === null) return null;
  const line = md.split("\n")[at];
  return line === undefined ? null : line.trimStart().replace(/^`+/, "").trim();
}

function complexityClaims(meta: string | null): [string | null, string | null] {
  if (meta === null) return [null, null];
  const pairs = solutionComplexities(meta);
  const find = (key: string): string | null => pairs.find(([name]) => name === key)?.[1] ?? null;
  return [find("time"), find("space")];
}

// ─────────────────────────────────────────────────────────────────────────────
// COMPLEXITY PROSE — `**Time Complexity:** O(…) – explanation` → the two cards
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Parse a Complexity section's authored `**Time Complexity:** … / **Space Complexity:** …`
 * paragraphs. Either axis may miss; when BOTH do the section isn't card-shaped and the caller
 * renders the prose as-is.
 */
export function complexityProse(md: string): ComplexityProse | null {
  let time: [string, string] | null = null;
  let space: [string, string] | null = null;
  let inFence = false;
  const lines = md.split("\n");
  let i = 0;
  while (i < lines.length) {
    const t = lines[i]!.trim();
    if (t.startsWith("```")) {
      inFence = !inFence;
      i += 1;
      continue;
    }
    const marker = inFence ? null : stripMarker(t);
    if (!marker) {
      i += 1;
      continue;
    }
    const [isTime, rest] = marker;
    // The paragraph: the marker line's remainder plus following lines until a blank line, a fence,
    // or the next marker.
    const paragraph = [rest.trim()];
    i += 1;
    while (i < lines.length) {
      const next = lines[i]!.trim();
      if (next === "" || next.startsWith("```") || stripMarker(next) !== null) break;
      paragraph.push(next);
      i += 1;
    }
    const parsed = splitValueProse(paragraph.join(" "));
    if (isTime) time = time ?? parsed;
    else space = space ?? parsed;
  }
  return time !== null || space !== null ? { time, space } : null;
}

function stripMarker(line: string): [boolean, string] | null {
  const lower = line.toLowerCase();
  const needles: [string, boolean][] = [
    ["**time complexity:**", true],
    ["**space complexity:**", false],
  ];
  for (const [needle, isTime] of needles) {
    if (lower.startsWith(needle)) return [isTime, line.slice(needle.length)];
  }
  return null;
}

/**
 * Split an authored complexity paragraph into (O-value, explanation). Authors separate the two with
 * a dash (`O(1) – Using …`), a period (`O(1). As no extra …`) or a comma (`O(1), The operations …`)
 * — all verified in the real content. The value starts at the first `O(` and runs to the first
 * separator sitting OUTSIDE parentheses after at least one closed group, so `O(min(N1, N2))` and
 * `O(sqrt(N)) + O(K*log(K)) – …` both survive whole. A paragraph without an `O(` group is not card
 * material.
 */
function splitValueProse(text: string): [string, string] | null {
  const SEPARATORS = [" – ", " — ", " - ", ". ", ", "];
  text = text.trim();
  const start = text.indexOf("O(");
  if (start < 0) return null;
  const tail = text.slice(start);
  let depth = 0;
  let closed = false;
  let split: [number, number] | null = null;
  for (let at = 0; at < tail.length; at++) {
    if (depth === 0 && closed) {
      const rest = tail.slice(at);
      const sep = SEPARATORS.find((s) => rest.startsWith(s));
      if (sep) {
        split = [at, sep.length];
        break;
      }
    }
    const c = tail[at];
    if (c === "(") depth += 1;
    else if (c === ")") {
      depth -= 1;
      if (depth === 0) closed = true;
    }
  }
  let value: string;
  let prose: string;
  if (split) {
    value = tail.slice(0, split[0]);
    prose = tail.slice(split[0] + split[1]);
  } else {
    value = tail;
    prose = "";
  }
  value = value.trim().replace(/[.,]+$/, "").replace(/\s+$/, "");
  return value === "" ? null : [value, prose.trim()];
}

// ─────────────────────────────────────────────────────────────────────────────
// DISPLAY HELPERS
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Display prettifier for complexity claims: authors write `O(sqrt(N)+K*log(K))` and `O(N^2)`, the
 * design shows `O(√N+K·log(K))` and `O(N²)`. Purely cosmetic — never fed back into parsing.
 */
export function prettyO(o: string): string {
  let out = "";
  let rest = o;
  let at: number;
  while ((at = rest.indexOf("sqrt(")) !== -1) {
    out += rest.slice(0, at);
    const inner = rest.slice(at + "sqrt(".length);
    let depth = 1;
    let close: number | null = null;
    for (let offset = 0; offset < inner.length; offset++) {
      const c = inner[offset];
      if (c === "(") depth += 1;
      else if (c === ")") {
        depth -= 1;
        if (depth === 0) {
          close = offset;
          break;
        }
      }
    }
    if (close !== null) {
      out += "√" + inner.slice(0, close);
      rest = inner.slice(close + 1);
    } else {
      out += rest.slice(at);
      rest = "";
    }
  }
  out += rest;
  return superscriptPowers(out.replace(/\*/g, "·"));
}

/** `^` followed by a WHOLE integer becomes a superscript (`N^2` → `N²`); a fractional power like
 *  `N^1.5` has no clean superscript and stays as authored. */
function superscriptPowers(s: string): string {
  const DIGITS = ["⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹"];
  const chars = [...s];
  let out = "";
  let i = 0;
  while (i < chars.length) {
    const c = chars[i]!;
    i += 1;
    if (c !== "^") {
      out += c;
      continue;
    }
    let power = "";
    while (i < chars.length && /[0-9]/.test(chars[i]!)) {
      power += chars[i]!;
      i += 1;
    }
    if (power === "" || chars[i] === ".") {
      out += "^" + power;
    } else {
      for (const d of power) out += DIGITS[d.charCodeAt(0) - 48];
    }
  }
  return out;
}

/** The scroll-spy: the ACTIVE section is the last one whose top has passed the threshold (tops are
 *  relative to the scroll container's top; sections above have negative tops). */
export function activeSection(sectionTops: number[], threshold: number): number {
  let active = 0;
  sectionTops.forEach((top, i) => {
    if (top <= threshold) active = i;
  });
  return active;
}
