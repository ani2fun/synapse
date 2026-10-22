// ──────────────────────────────────────────────────────────────────
// UNITS — the prose cut into what the speech engine says in one go
// ──────────────────────────────────────────────────────────────────
// A unit is what the server synthesises and caches, so its size is a trade-off the engine sets:
// measured on wk-1, time-to-audio and memory both grow with the length of one request (a 400-char
// unit took up to ~20 s, a one-sentence unit ~1–2 s). Two rules follow:
//
//   · Units are cut WITHIN a block, never across one. A paragraph then always yields the same
//     units wherever the listen started, which is what lets two readers share the cache.
//   · The very first unit of a listen is ONE sentence, so the first audio arrives in a second or
//     two instead of waiting for a full unit.

/** What a packed unit aims for. The server's hard limit (`maxChars`) is higher. */
export const TARGET_CHARS = 300;

/** Sentences of one block. A sentence ends at `.`, `!`, `?` or `…` followed by whitespace —
 *  "e.g. this" splits wrongly, which costs a slightly early pause and nothing else. */
export function sentences(block: string): string[] {
  return block
    .split(/(?<=[.!?…]["')\]]?)\s+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** A sentence longer than `maxChars`, cut on the last space before each limit (or mid-word when
 *  there is no space at all). */
export function hardSplit(sentence: string, maxChars: number): string[] {
  const pieces: string[] = [];
  let rest = sentence;
  while (rest.length > maxChars) {
    const space = rest.lastIndexOf(" ", maxChars);
    const cut = space > 0 ? space : maxChars;
    pieces.push(rest.slice(0, cut).trim());
    rest = rest.slice(cut).trim();
  }
  if (rest) pieces.push(rest);
  return pieces;
}

/**
 * Pack one block's sentences into units: whole sentences joined with a space, each unit as close
 * to `target` as possible without passing it. Every sentence passed in is already at most
 * `maxChars` long (`hardSplit` saw to that), so a single sentence longer than `target` is a unit
 * on its own.
 */
export function packSentences(sentences: readonly string[], target: number): string[] {
  // Greedy, not balanced: a unit's boundaries depend only on the sentences before it in its own
  // block, so an edit late in a paragraph leaves the units ahead of it — and their cached audio —
  // exactly as they were.
  const units: string[] = [];
  let current = "";
  for (const sentence of sentences) {
    if (current && current.length + 1 + sentence.length <= target) {
      current = `${current} ${sentence}`;
    } else {
      if (current) units.push(current);
      current = sentence;
    }
  }
  if (current) units.push(current);
  return units;
}

/** One unit, and where it came from: block `block`, characters `[start, end)` of its text. */
export interface UnitSpan {
  text: string;
  block: number;
  start: number;
  end: number;
}

/**
 * The whole listen: blocks → units, with the first unit cut down to one sentence. Each unit is an
 * exact substring of its block's text (sentences are cut on whitespace and joined with one space,
 * and the text is already folded), so finding it again is a forward search — which is what lets
 * the page highlight the unit being spoken.
 */
export function unitSpans(blocks: readonly string[], maxChars: number): UnitSpan[] {
  const spans: UnitSpan[] = [];
  blocks.forEach((block, index) => {
    let parts = sentences(block).flatMap((s) => hardSplit(s, maxChars));
    const units: string[] = [];
    if (index === 0 && parts.length > 1) {
      // Fast start: the opening sentence goes alone, the rest of the block packs as usual.
      units.push(parts[0]);
      parts = parts.slice(1);
    }
    units.push(...packSentences(parts, Math.min(TARGET_CHARS, maxChars)));
    let cursor = 0;
    for (const text of units) {
      const found = block.indexOf(text, cursor);
      // Not found only if a split cut inside a word; the unit then highlights its whole block.
      const start = found >= 0 ? found : 0;
      const end = found >= 0 ? found + text.length : block.length;
      spans.push({ text, block: index, start, end });
      if (found >= 0) cursor = end;
    }
  });
  return spans;
}

/** The units alone. */
export function splitIntoUnits(blocks: readonly string[], maxChars: number): string[] {
  return unitSpans(blocks, maxChars).map((span) => span.text);
}
