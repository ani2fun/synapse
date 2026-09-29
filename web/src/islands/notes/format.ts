// The Notes toolbar's edits, as pure functions over (text, selection) → (text, selection).
//
// Every button writes MARKDOWN, never markup: a note is plain text the reader owns, so what they
// see in Write is exactly what they would paste anywhere else. That rules out the two controls a
// rich-text bar usually carries — underline and alignment — because markdown has no spelling for
// either, and the alternatives (raw `<u>`, a private syntax) would render nowhere but here.
//
// Each action TOGGLES: pressing Bold inside `**word**` takes the stars away rather than nesting a
// second pair, and pressing Bullet on lines that are all bullets unmarks them. A toolbar that can
// only add is one the reader has to undo by hand.
//
// Kept free of the DOM so the vitest suite (node, no document) can pin every case.

export type FormatAction =
  | "bold"
  | "italic"
  | "strike"
  | "code"
  | "heading"
  | "bullet"
  | "numbered"
  | "task"
  | "quote"
  | "codeblock"
  | "link"
  | "image";

/** A buffer and the selection to leave behind after an edit. */
export interface Edit {
  text: string;
  start: number;
  end: number;
}

/** The hard ceiling on a note. Enforced by the textarea (`maxLength`) for typing and by the pane
 *  for toolbar edits, which can grow the text past what the reader typed. */
export const NOTES_MAX = 10_000;

// ── inline wraps ─────────────────────────────────────────────────────────────

/** Marker and the word planted when nothing is selected — a placeholder the reader types over,
 *  selected, so the next keystroke replaces it. */
const WRAPS: Record<"bold" | "italic" | "strike" | "code", [string, string]> = {
  bold: ["**", "bold text"],
  // `_`, not `*`: a single star would read as half of a bold marker when toggling next to one.
  italic: ["_", "italic text"],
  strike: ["~~", "struck text"],
  code: ["`", "code"],
};

function wrap(text: string, start: number, end: number, mark: string, placeholder: string): Edit {
  const before = text.slice(0, start);
  const picked = text.slice(start, end);
  const after = text.slice(end);
  if (before.endsWith(mark) && after.startsWith(mark)) {
    return {
      text: before.slice(0, -mark.length) + picked + after.slice(mark.length),
      start: start - mark.length,
      end: end - mark.length,
    };
  }
  // A selection that INCLUDES its markers (a double-click on `**word**` in some browsers).
  if (picked.length >= mark.length * 2 && picked.startsWith(mark) && picked.endsWith(mark)) {
    const inner = picked.slice(mark.length, -mark.length);
    return { text: before + inner + after, start, end: start + inner.length };
  }
  const inner = picked === "" ? placeholder : picked;
  const at = start + mark.length;
  return { text: before + mark + inner + mark + after, start: at, end: at + inner.length };
}

// ── line prefixes ────────────────────────────────────────────────────────────

/** The whole lines a selection touches. A selection that ends at column 0 of the next line (a
 *  triple-click, a drag to the line below) does not claim that line. */
function lineSpan(text: string, start: number, end: number): [number, number] {
  const from = text.lastIndexOf("\n", start - 1) + 1;
  const stop = end > start && text[end - 1] === "\n" ? end - 1 : end;
  const newline = text.indexOf("\n", stop);
  return [from, newline < 0 ? text.length : newline];
}

interface LineKind {
  /** Matches EXACTLY this kind's marker at the start of a line — what toggling off looks for. */
  marker: RegExp;
  /** A wider family replaced when the marker goes on, where one exists. */
  replaces?: RegExp;
  /** The marker for the i-th marked line (numbering is the only kind that varies). */
  prefix: (i: number) => string;
}

const LINE_KINDS: Record<"heading" | "bullet" | "numbered" | "task" | "quote", LineKind> = {
  // Toggles only on `## `; any other level is converted to it rather than read as "already done".
  heading: { marker: /^## /, replaces: /^#{1,6} /, prefix: () => "## " },
  // A task item also starts with `- `; the lookahead keeps Bullet from calling one a bullet.
  bullet: { marker: /^[-*+] (?!\[[ xX]\] )/, prefix: () => "- " },
  numbered: { marker: /^\d+[.)] /, prefix: (i) => `${i + 1}. ` },
  task: { marker: /^[-*+] \[[ xX]\] /, prefix: () => "- [ ] " },
  quote: { marker: /^> ?/, prefix: () => "> " },
};

/** Every OTHER list-ish marker, stripped before a new one goes on — Bullet on a numbered line
 *  converts it rather than stacking `- 1. item`. Headings and quotes compose with lists, so they
 *  are left alone. */
const LIST_MARKER = /^(?:[-*+] \[[ xX]\] |[-*+] |\d+[.)] )/;

function prefixLines(text: string, start: number, end: number, kind: keyof typeof LINE_KINDS): Edit {
  const { marker, replaces, prefix } = LINE_KINDS[kind];
  const [from, to] = lineSpan(text, start, end);
  const lines = text.slice(from, to).split("\n");
  // Blank lines inside a multi-line selection stay blank: an empty bullet between paragraphs is
  // never what a reader selecting three paragraphs meant. A lone blank line is the exception —
  // that is the caret on an empty line, asking for a fresh item.
  const counts = (line: string) => lines.length === 1 || line.trim() !== "";
  const unmark = lines.filter(counts).every((line) => marker.test(line));
  let n = 0;
  const next = lines.map((line) => {
    if (!counts(line)) return line;
    if (unmark) return line.replace(marker, "");
    const isList = kind === "bullet" || kind === "numbered" || kind === "task";
    const bare = line.replace(isList ? LIST_MARKER : (replaces ?? marker), "");
    return prefix(n++) + bare;
  });
  const block = next.join("\n");
  const out = text.slice(0, from) + block + text.slice(to);
  // A caret stays a caret, parked at the end of its line so typing continues the item. A real
  // selection comes back as the whole reworked block, so a second press toggles it straight back.
  if (start === end && lines.length === 1) {
    const caret = from + block.length;
    return { text: out, start: caret, end: caret };
  }
  return { text: out, start: from, end: from + block.length };
}

// ── blocks and links ─────────────────────────────────────────────────────────

function codeBlock(text: string, start: number, end: number): Edit {
  const before = text.slice(0, start);
  const picked = text.slice(start, end);
  const after = text.slice(end);
  // A fence only opens at the start of a line, and only closes on one of its own.
  const lead = before === "" || before.endsWith("\n") ? "" : "\n";
  const tail = after === "" || after.startsWith("\n") ? "" : "\n";
  const body = picked.endsWith("\n") ? picked.slice(0, -1) : picked;
  const at = before.length + lead.length + "```\n".length;
  return {
    text: `${before}${lead}\`\`\`\n${body}\n\`\`\`${tail}${after}`,
    start: at,
    end: at + body.length,
  };
}

const URL_LIKE = /^(?:https?:\/\/|mailto:|\/)\S*$/;

/** `[label](url)` / `![alt](url)`. A selection that is already a URL becomes the target, and the
 *  label is left selected; any other selection becomes the label, and the url slot is selected. */
function link(text: string, start: number, end: number, image: boolean): Edit {
  const before = text.slice(0, start);
  const picked = text.slice(start, end).trim();
  const after = text.slice(end);
  const bang = image ? "!" : "";
  if (URL_LIKE.test(picked)) {
    const label = image ? "alt text" : "link text";
    const at = before.length + bang.length + 1;
    return { text: `${before}${bang}[${label}](${picked})${after}`, start: at, end: at + label.length };
  }
  const label = picked === "" ? (image ? "alt text" : "link text") : picked;
  const url = "https://";
  const at = before.length + bang.length + 1 + label.length + 2;
  return { text: `${before}${bang}[${label}](${url})${after}`, start: at, end: at + url.length };
}

// ── the one entry point ──────────────────────────────────────────────────────

export function applyFormat(text: string, start: number, end: number, action: FormatAction): Edit {
  const lo = Math.max(0, Math.min(start, end, text.length));
  const hi = Math.min(text.length, Math.max(start, end));
  switch (action) {
    case "bold":
    case "italic":
    case "strike":
    case "code": {
      const [mark, placeholder] = WRAPS[action];
      return wrap(text, lo, hi, mark, placeholder);
    }
    case "heading":
    case "bullet":
    case "numbered":
    case "task":
    case "quote":
      return prefixLines(text, lo, hi, action);
    case "codeblock":
      return codeBlock(text, lo, hi);
    case "link":
      return link(text, lo, hi, false);
    case "image":
      return link(text, lo, hi, true);
  }
}

/** The smallest splice turning `before` into `after`: the shared prefix and suffix are left in
 *  place. The pane applies an edit through this so the browser's own undo stack records ONE
 *  replacement of the changed region, rather than the whole buffer being swapped out from under
 *  Cmd+Z. */
export function spliceOf(before: string, after: string): { from: number; to: number; insert: string } {
  let head = 0;
  const max = Math.min(before.length, after.length);
  while (head < max && before[head] === after[head]) head += 1;
  let tail = 0;
  while (
    tail < max - head &&
    before[before.length - 1 - tail] === after[after.length - 1 - tail]
  ) {
    tail += 1;
  }
  return { from: head, to: before.length - tail, insert: after.slice(head, after.length - tail) };
}
