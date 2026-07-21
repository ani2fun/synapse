// ──────────────────────────────────────────────────────────────────
// JAVA TRACER WRAP (step 31)
// ──────────────────────────────────────────────────────────────────
// The JVM counterpart of python.ts. Embeds the user's Java source (base64,
// UTF-8-safe) into the AST-rewrite harness (java-harness.java, imported ?raw) —
// a self-contained `public class Main` (first line `// __SYNAPSE_TRACER__`) that
// go-judge compiles with `javac` and runs; at runtime it recompiles the embedded
// user source WITH instrumentation via the sandbox JDK's `javax.tools` compiler
// and prints the heap trace between the __SYNAPSE_HEAP_* markers. Lazy @tracer
// island, so the ~900-line harness string is off the initial bundle.

import harness from "./java-harness.java?raw";

const PLACEHOLDER = "__SYNAPSE_USER_SOURCE_B64__";

// btoa only handles Latin-1; encode UTF-8 bytes first so non-ASCII source survives (same as python.ts).
function utf8ToBase64(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary);
}

/** The traced program to send to /api/run: the harness with the user Java source embedded. */
export function wrapJava(source: string): string {
  // replaceAll, not replace — and HERE it is load-bearing. java-harness.java names the
  // placeholder twice: once in its header comment (line 3) and once in the `USER_SOURCE_B64`
  // constant that actually matters (line 80). `replace` would substitute the COMMENT and leave
  // the constant holding the literal string, so the harness would compile, run, and decode a
  // placeholder as though it were the user's program. Pinned by tracer.test.ts.
  return harness.replaceAll(PLACEHOLDER, utf8ToBase64(source));
}
