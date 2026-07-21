// ──────────────────────────────────────────────────────────────────
// PYTHON TRACER WRAP (step 28)
// ──────────────────────────────────────────────────────────────────
// Embeds the user's Python source (base64, UTF-8-safe) into the sys.settrace
// harness (python-harness.py, imported ?raw). The wrapped program runs through
// the ordinary /api/run — the server is untouched for Python (no sentinel
// needed; only traced Java skips entrypoint rewrite). The multi-line harness
// string is behind the lazy @tracer island so it's off the initial bundle.

import harness from "./python-harness.py?raw";

const PLACEHOLDER = "__SYNAPSE_USER_SOURCE_B64__";

// btoa only handles Latin-1; encode UTF-8 bytes first so non-ASCII source survives.
function utf8ToBase64(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary);
}

/** The traced program to send to /api/run: the harness with the user source embedded. */
export function wrapPython(source: string): string {
  // replaceAll, not replace. Today python-harness.py mentions the placeholder exactly once, so
  // the two are equivalent here — but its Java sibling does not (see java.ts), and a harness
  // that grows a second mention must not quietly start embedding the payload in the wrong
  // place. Pinned by tracer.test.ts rather than left to the comment.
  return harness.replaceAll(PLACEHOLDER, utf8ToBase64(source));
}
