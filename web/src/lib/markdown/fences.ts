// ──────────────────────────────────────────────────────────────────
// FINDING ```d2 FENCES IN A MARKDOWN DOCUMENT
// ──────────────────────────────────────────────────────────────────
// Three things have to agree about what a d2 fence is:
//
//   · remark, when the reader's page is rendered (`render.ts`);
//   · `dev-tools/render-d2.mjs`, which no longer draws anything for the reader (ADR-RS009 put a
//     renderer beside the app) but is still pinned to this shape by `renderD2Script.test.ts`;
//   · this, when `/d2` reaches into a lesson to load the diagram someone clicked Edit on.
//
// The script cannot import this file — it runs from a plain Node checkout in a content repo with
// no TypeScript — so it carries its own copy, and `renderD2Script.test.ts` compares all three
// against remark. A disagreement is silent by construction: the page renders, the figure is just
// never drawn, or the editor opens the wrong diagram.
//
// The rules that matter, from CommonMark:
//   · an opening fence is THREE OR MORE backticks, and the closing run must be at least as long;
//   · the info string of a backtick fence may not contain a backtick.
// Together those are what make ````markdown … ```d2 … ```` a documentation example rather than a
// diagram — the inner three-backtick run cannot close a four-backtick fence.

/** A fenced block: the language, everything after it on the fence line, and the body. */
export interface Fence {
  lang: string;
  meta: string;
  source: string;
  /** 1-based line the opening fence sits on, for diagnostics that name a place. */
  line: number;
  /** Character offsets of the WHOLE block — opening run through closing run — so a caller can
   *  cut it out exactly. Two byte-identical fences in one document are distinguishable only by
   *  these, which is why they are carried rather than re-derived by searching for the text. */
  start: number;
  end: number;
}

// (1) the opening run, (2) the info string, (3) the body, then a closing run at least as long.
const FENCE = /^(`{3,})([^`\n]*)\n([\s\S]*?)^\1`*[ \t]*$/gm;

/** Every fenced block in a document, in order. */
export function fences(markdown: string): Fence[] {
  const found: Fence[] = [];
  for (const match of markdown.matchAll(FENCE)) {
    const info = (match[2] ?? "").trim();
    const space = info.search(/\s/);
    found.push({
      lang: (space === -1 ? info : info.slice(0, space)).toLowerCase(),
      meta: space === -1 ? "" : info.slice(space + 1).trim(),
      // The newline before the closing fence belongs to the FENCE, not to the source. remark's
      // `code.value` carries none, and that string is the cache key every drawn figure is found
      // by — a stray "\n" here changes every hash and misses every lookup.
      source: (match[3] ?? "").replace(/\n$/, ""),
      line: lineAt(markdown, match.index ?? 0),
      start: match.index ?? 0,
      end: (match.index ?? 0) + match[0].length,
    });
  }
  return found;
}

/** Every ```d2 fence, in order. The index into THIS list is what a figure's `data-fence-at`
 *  names, and what `/d2` uses to find the diagram again. */
export function d2Fences(markdown: string): Fence[] {
  return fences(markdown).filter((fence) => fence.lang === "d2");
}

/**
 * Every ```mermaid fence, in order — the same ordinal contract, on its own list.
 *
 * Per LANGUAGE, not per diagram: `/mermaid?at=1` is the second mermaid figure in the lesson, and
 * a d2 fence sitting between two mermaid ones does not advance it. The two editors would open
 * each other's diagrams if they shared a counter.
 *
 * Only TWO implementations have to agree about this one — remark and this — because nothing
 * draws mermaid ahead of time. The three-way pin in `renderD2Script.test.ts` is d2's alone.
 */
export function mermaidFences(markdown: string): Fence[] {
  return fences(markdown).filter((fence) => fence.lang === "mermaid");
}

function lineAt(text: string, index: number): number {
  let line = 1;
  for (let i = 0; i < index; i += 1) if (text[i] === "\n") line += 1;
  return line;
}
