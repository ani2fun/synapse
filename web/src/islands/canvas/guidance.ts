/**
 * What each area of the canvas is FOR — the text behind every ℹ️, and the chips that seed the two
 * areas readers most often stare at blankly.
 *
 * Every entry links its source. The canvas is not this app's invention: it is HiredInTech's
 * Algorithm Design Canvas as extended by the startupnextdoor write-up (which adds Inputs, Return,
 * Error/N-A and Maintenance to the original four), and a reader who wants the long version should
 * be one click from the people who wrote it rather than from a paraphrase.
 */

export interface AreaGuidance {
  /** The area's display name — also the modal's heading. */
  title: string;
  /** Why the area exists, in the voice of someone who has watched it go wrong. */
  what: string;
  /** The checklist: what a filled-in version of this area actually contains. */
  bullets: string[];
  /** The fragment of the appendix lesson that covers this area at length. */
  lessonHash: string;
  /** The outside write-up this area comes from, credited by name. */
  href: string;
  linkLabel: string;
}

/** The in-app long form: the appendix lesson in the DSA book, which carries the whole method plus
 *  the material the two source articles leave out (the N-to-complexity budget, overflow, auxiliary
 *  vs total space, deriving tests from constraints). The modal leads with THIS and offers the
 *  external source second — the reader should not have to leave to get the full answer, but the
 *  provenance stays one click away, and an outside link still works where the DSA book is not
 *  mounted. */
export const APPENDIX_PATH = "/synapse/dsa/appendix/algorithm-design-canvas";

/** Keyed by the `Area` union in `model.ts`, plus `ideas` — which is an area of the canvas but not
 *  a text field, so it is not in `AREAS`. */
export const GUIDANCE: Record<string, AreaGuidance> = {
  problem: {
    title: "Problem",
    what: "Name the problem and restate it in one sentence, in your own words. If you cannot restate it, you do not understand it yet — and every later area will inherit the confusion.",
    bullets: [
      "A one-line restatement the interviewer would agree with",
      "The category you think it belongs to (arrays, graphs, DP…)",
      "Anything in the wording you are unsure about — ask now",
      "The success criterion: what does a correct answer look like?",
    ],
    lessonHash: "1--problem",
    href: "https://www.hiredintech.com/algorithms/algorithm-design-canvas/what-is-the-canvas/",
    linkLabel: "What is the canvas",
  },
  constraints: {
    title: "Constraints",
    what: "Every limit the problem or the interviewer imposes. An ill-defined problem is unsolvable: sorting 50 numbers and sorting 5 billion million-character strings are different problems. Ask — never assume.",
    bullets: [
      "Min and max for every key value, especially N",
      "Range and type of the values (ints, chars, unicode, negatives, zeros)",
      "Duplicates allowed? sorted? unique?",
      "Input shape: adjacency matrix vs list, stream vs in-memory",
      "May you mutate the input? memory ceiling? time budget?",
      "Anything a value affects in your complexity — ask about it",
    ],
    lessonHash: "2--constraints",
    href: "https://www.hiredintech.com/files/the-common-constraints-handout.pdf",
    linkLabel: "Common constraints handout (PDF)",
  },
  maintenance: {
    title: "Maintenance",
    what: "State your algorithm has to keep in sync while it runs. These are the bugs that bite in interviews — the pointer you forgot to move, the counter you forgot to increment.",
    bullets: [
      "Pointers to keep valid (tail of a linked list, window edges)",
      "Running values (best so far, current sum, seen-map)",
      "Updates that must happen after a loop, not inside it",
      "The invariant that must hold at every iteration boundary",
    ],
    lessonHash: "6--maintenance",
    href: "https://startupnextdoor.com/my-algorithm-design-canvas/",
    linkLabel: "Source: the original canvas",
  },
  inputs: {
    title: "Inputs",
    what: "The signature, concretely: what arrives, in what type, at what size. Write it the way you would write a function docstring.",
    bullets: [
      'Each parameter with its type ("array of 32-bit ints")',
      'Sizes and lengths ("len ≤ 1e4")',
      'Structure ("string of ASCII", "adjacency matrix")',
      "Whether the input is already sorted / de-duplicated",
    ],
    lessonHash: "3--inputs",
    href: "https://startupnextdoor.com/my-algorithm-design-canvas/",
    linkLabel: "Source: the original canvas",
  },
  ret: {
    title: "Return",
    what: "What your function hands back. Getting this wrong wastes the whole coding round, and it is a 10-second question.",
    bullets: [
      'Value and type ("int[] of two indices")',
      "Indices or values? any order, or a required order?",
      "In-place mutation instead of a return value?",
      "Void plus printed output (e.g. printing all permutations)?",
    ],
    lessonHash: "4--return",
    href: "https://startupnextdoor.com/my-algorithm-design-canvas/",
    linkLabel: "Source: the original canvas",
  },
  errors: {
    title: "Error / N/A",
    what: "The behaviour when there is no good answer. Agree it with the interviewer rather than inventing it silently mid-code.",
    bullets: [
      "Not found → −1, None, NULL, or another sentinel?",
      "Bad input → throw, or defensive return?",
      "Empty input → what exactly?",
      "Multiple valid answers → any, or first?",
    ],
    lessonHash: "5--error--na",
    href: "https://startupnextdoor.com/my-algorithm-design-canvas/",
    linkLabel: "Source: the original canvas",
  },
  ideas: {
    title: "Ideas",
    what: "One to three approaches, brute force first, then refined. Each is a short description any interviewer can follow — plus its time (T) and space (S) complexity, which is where the trade-off conversation happens.",
    bullets: [
      "Start with the obvious brute force; say it out loud, then improve it",
      "Two or three lines: the data structure and why it helps",
      "T and S in Big-O, and say what n means",
      "Name the trade-off you are making (time vs memory)",
      "Only code the idea you and the interviewer agreed on",
    ],
    lessonHash: "7--ideas",
    href: "https://www.hiredintech.com/algorithms/algorithm-design-canvas/complexity/",
    linkLabel: "Complexity lesson + handout",
  },
  tests: {
    title: "Tests",
    what: "The cases you will run your code against, written before you code. Your constraints section is the raw material: hit its minimums and maximums.",
    bullets: [
      "Empty input: null, empty string, empty array",
      "Single element, then two elements",
      "Negative values and zero",
      "Even and odd lengths (midpoints, jumps, binary search)",
      "No-solution case",
      "Load test at the maximum size the constraints allow",
    ],
    lessonHash: "8--tests",
    href: "https://www.hiredintech.com/algorithms/algorithm-design-canvas/testing-your-code/",
    linkLabel: "Testing your code",
  },
};

/** One click appends `· <label> — ` and drops the caret after it. Only the two areas whose blank
 *  state is genuinely paralysing get chips: the rest are answered by reading the problem. */
export const CHIPS: Record<string, readonly string[]> = {
  constraints: ["max N", "value range", "duplicates?", "sorted?", "memory limit", "mutate input?", "unicode?"],
  tests: ["empty", "single element", "two elements", "negatives", "zero", "odd / even length", "no solution", "max size"],
};

/** The line a chip plants. Exported so the form and its test agree on one spelling. */
export function chipLine(current: string, label: string): string {
  const lead = current !== "" && !current.endsWith("\n") ? "\n" : "";
  return `${current}${lead}· ${label} — `;
}
