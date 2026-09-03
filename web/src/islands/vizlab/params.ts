// ──────────────────────────────────────────────────────────────────
// WHAT THE LAB WAS OPENED ON
// ──────────────────────────────────────────────────────────────────
// `?s=<structure>[:<root>]&lang=` — the shape an authored fence's `viz=` hint already has, so a
// link into the lab reads the same as the fence it came from. Absent means the remembered choice,
// and failing that the default.
//
// Deliberately unaware of the vocabulary: the closed set of structures lives in the crate
// (`viz_structures()`), and a second copy here is how one of them silently grows a token the
// other cannot draw. A token this returns may well be nonsense — the caller checks it against
// what the crate offers.

export interface VizParams {
  /** The structure token as written, un-validated. Null when unset or blank. */
  structure: string | null;
  /** The root variable from `s=array:arr`. Null when the hint named none. */
  root: string | null;
  /** The language tab to open on, lowercased. Null when unset. */
  language: string | null;
}

/** Split a `<structure>[:<root>]` hint. A colon with nothing after it declares no root rather
 *  than an empty one — the same reading `VizStructure::parse` gives it. */
export function splitHint(hint: string): { structure: string | null; root: string | null } {
  const trimmed = hint.trim();
  if (trimmed === "") return { structure: null, root: null };
  const colon = trimmed.indexOf(":");
  if (colon === -1) return { structure: trimmed, root: null };
  const root = trimmed.slice(colon + 1).trim();
  return { structure: trimmed.slice(0, colon).trim() || null, root: root === "" ? null : root };
}

/** Compose the hint back — what the crate is handed, and what a `viz=` fence would carry. */
export function composeHint(structure: string, root: string | null): string {
  const cleanRoot = root?.trim() ?? "";
  return cleanRoot === "" ? structure : `${structure}:${cleanRoot}`;
}

export function paramsFromUrl(search: string): VizParams {
  const params = new URLSearchParams(search);
  const { structure, root } = splitHint(params.get("s") ?? "");
  const language = params.get("lang")?.trim().toLowerCase();
  return { structure, root, language: language === "" || language == null ? null : language };
}
