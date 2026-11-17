/**
 * The pure half of the exercise-set widget (`Exercise.ts`): what each blank expects, and how a
 * typed answer is graded against it. No DOM here, so vitest covers it in the node environment.
 *
 * Content authors never write the expected fills down. They come from the pair the course
 * already has: the exercise item (`Tom ______ au tennis… (jouer)`) and its answer line
 * (`Tom jouait au tennis… (jouer)`). Diffing the two recovers the fill even where the answer
 * bolds only part of it: `L'électricité n'**existait** pas` expects `n'existait pas`.
 */

/** A run of three or more underscores is a blank. Shorter runs are left as prose. */
export const BLANK = /_{3,}/g;

/** Collapse whitespace and unify apostrophes, so prompt and answer line compare as text. */
export function normalizeSpace(text: string): string {
  return text.replace(/[’ʼ]/g, "'").replace(/\s+/g, " ").trim();
}

/** Remove combining marks: `é` → `e`, `ç` → `c`. For lenient comparisons only. */
export function foldAccents(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").normalize("NFC");
}

/** A trailing parenthetical hint, `(jouer)` or `(exister/négation)`. */
const TRAILING_HINT = /\s*\([^)]*\)\s*[.!?]?\s*$/;

/** Locate the prompt's fixed segments, in order, in the answer line; the gaps are the fills. */
function diffFills(segments: string[], answer: string): string[] | null {
  const first = segments[0];
  const last = segments[segments.length - 1];
  if (!answer.startsWith(first) || !answer.endsWith(last)) return null;
  if (first.length + last.length > answer.length) return null;

  const fills: string[] = [];
  let pos = first.length;
  const end = answer.length - last.length;
  for (let i = 1; i < segments.length; i += 1) {
    const isLast = i === segments.length - 1;
    // The last segment is anchored to the end, so a short word like `à` in the middle of a fill
    // cannot be mistaken for it. A middle segment must be non-empty, or there is no way to tell
    // where one fill stops and the next starts.
    if (!isLast && segments[i].trim() === "") return null;
    const at = isLast ? end : answer.indexOf(segments[i], pos);
    if (at < pos) return null;
    const fill = answer.slice(pos, at).trim();
    if (fill === "") return null;
    fills.push(fill);
    pos = at + segments[i].length;
  }
  return fills;
}

/**
 * The expected text of each blank in `promptText`, or `null` when it cannot be worked out (the
 * widget then offers Show answer only for that item).
 *
 * @param promptText the exercise item's text (markup already stripped, e.g. `textContent`)
 * @param answerText the answer item's first line, before any translation line
 * @param boldTexts  the answer line's bold runs, in order — the fallback when diffing fails
 */
export function expectedFills(promptText: string, answerText: string, boldTexts: string[]): string[] | null {
  const prompt = normalizeSpace(promptText);
  const answer = normalizeSpace(answerText);
  const blanks = prompt.match(BLANK)?.length ?? 0;
  if (blanks === 0) return null;

  const split = (text: string) => text.split(BLANK).map((segment) => segment.replace(/^ | $/g, ""));
  const fills =
    diffFills(split(prompt), answer) ??
    // Answers sometimes drop or reword the hint: `(se promener)` against `(**se promener**)` is
    // fine, but `(exister/négation)` against no hint at all is not. Retry without it.
    diffFills(split(prompt.replace(TRAILING_HINT, "")), answer.replace(TRAILING_HINT, ""));
  if (fills && fills.length === blanks) return fills;

  const bold = boldTexts.map(normalizeSpace).filter((text) => text !== "");
  return bold.length === blanks ? bold : null;
}

export type Grade = "right" | "accent" | "wrong";

/**
 * Grade one typed blank against its expected fill.
 *
 * - `right`  — the reader wrote the expected French.
 * - `accent` — right letters, wrong or missing accents (`jouàit`, `preparais`): shown amber with
 *              "check the accents", not counted in the score.
 * - `wrong`  — anything else.
 *
 * Rules:
 * - Case does not count. A blank at the start of a sentence has a capital in the answer, and the
 *   reader should not have to guess that. `Jouait` and `jouait` are both right.
 * - Spacing and apostrophes do not count: `n’existait`, `n'existait` and `n' existait` are the
 *   same answer. Typing it on a phone keyboard must not be a trap.
 * - Accents count, but as a separate verdict: `preparais` for `préparais` is `accent`, so the
 *   reader sees the conjugation was right and the spelling was not.
 * - An empty input is `wrong`. Check on an empty blank then shows the answer, which is what a
 *   reader who presses Check without typing wants.
 */
function comparable(text: string): string {
  return normalizeSpace(text).replace(/' /g, "'").toLocaleLowerCase("fr");
}

export function grade(typed: string, expected: string): Grade {
  const got = comparable(typed);
  const want = comparable(expected);
  if (got === "") return "wrong";
  if (got === want) return "right";
  return foldAccents(got) === foldAccents(want) ? "accent" : "wrong";
}
