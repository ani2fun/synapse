import { describe, expect, it } from "vitest";

import javaHarness from "./java-harness.java?raw";
import pythonHarness from "./python-harness.py?raw";
import { wrapJava } from "./java";
import { wrapPython } from "./python";

/**
 * The tracer wrappers. Small surface, but everything here fails SILENTLY when it breaks: a wrap
 * that leaves a placeholder behind still compiles and still runs, it just traces nothing, and
 * the Visualise modal shows an empty or failed card that reads like a user error. The e2e suite
 * cannot see any of it — a trace with the wrong contents is still a trace.
 */

const PLACEHOLDER = "__SYNAPSE_USER_SOURCE_B64__";

/** Undo what the wrapper did, so the assertions are about the round trip, not the encoding. */
function decodeEmbedded(wrapped: string, marker: RegExp): string {
  const match = wrapped.match(marker);
  if (!match) throw new Error("no embedded payload found");
  const bytes = Uint8Array.from(atob(match[1]), (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

describe("the placeholder is fully substituted", () => {
  // THE trap. `java-harness.java` mentions the placeholder twice — once in a header comment
  // (line 3) and once in the constant that actually matters (line 80) — so `replace` would
  // substitute the COMMENT and leave `USER_SOURCE_B64` holding the literal string. The harness
  // then compiles, runs, and decodes a placeholder as if it were the user's program.
  it("leaves no literal placeholder in the wrapped Java", () => {
    expect(javaHarness.split(PLACEHOLDER).length - 1).toBeGreaterThan(1);
    expect(wrapJava("class Main {}")).not.toContain(PLACEHOLDER);
  });

  // Python's harness mentions it once today, so `replace` would be equivalent — which is
  // precisely why this test exists. The day someone adds a second mention, `replaceAll` must
  // already be in place rather than being noticed afterwards.
  it("leaves no literal placeholder in the wrapped Python", () => {
    expect(pythonHarness.split(PLACEHOLDER).length - 1).toBeGreaterThanOrEqual(1);
    expect(wrapPython("print(1)")).not.toContain(PLACEHOLDER);
  });
});

describe("non-ASCII source survives the encoding", () => {
  // `btoa` only accepts Latin-1 and THROWS on anything above U+00FF, so a naive
  // `btoa(source)` would not corrupt the trace — it would break the wrap outright the first
  // time someone put a non-English string or an emoji in their program.
  const tricky = 'x = "héllo · 世界 · 🎉"\nprint(x)\n';

  it("round-trips through the Python wrap", () => {
    const wrapped = wrapPython(tricky);
    expect(decodeEmbedded(wrapped, /b64decode\("([^"]+)"\)/)).toBe(tricky);
  });

  it("round-trips through the Java wrap", () => {
    const source = '// héllo · 世界 · 🎉\nclass Main {}\n';
    const wrapped = wrapJava(source);
    expect(decodeEmbedded(wrapped, /USER_SOURCE_B64 = "([^"]+)"/)).toBe(source);
  });

  it("does not throw on characters btoa alone would reject", () => {
    expect(() => wrapPython("# 🎉\n")).not.toThrow();
    expect(() => wrapJava("// 🎉\n")).not.toThrow();
  });
});

describe("the Java sentinel contract", () => {
  // A cross-LANGUAGE contract with no compiler on either side of it. The server's
  // `JAVA_TRACER_SENTINEL` (server/src/execution/infrastructure/java_rewriter.rs) matches on
  // this exact string to know that traced Java already defines `Main` and must pass through
  // WITHOUT entrypoint rewriting. Reformat the harness, add a licence header, or let an editor
  // insert a blank first line, and traced Java silently gets rewritten and stops working.
  it("is the first line of the harness, byte for byte", () => {
    expect(javaHarness.split("\n")[0]).toBe("// __SYNAPSE_TRACER__");
  });

  it("survives the wrap", () => {
    expect(wrapJava("class Main {}").split("\n")[0]).toBe("// __SYNAPSE_TRACER__");
  });
});

describe("the wrap keeps the harness intact", () => {
  // The wrapped program is what /api/run compiles. If the substitution ever damaged the harness
  // — a stray global replace across the whole file, say — the failure would surface as a
  // compiler error attributed to the user's code.
  it("preserves the heap markers the decoder splits on", () => {
    const wrapped = wrapPython("print(1)");
    expect(wrapped).toContain("__SYNAPSE_HEAP_BEGIN__");
    expect(wrapped).toContain("__SYNAPSE_HEAP_END__");
  });

  it("changes nothing but the payload", () => {
    const a = wrapPython("print(1)");
    const b = wrapPython("print(2)");
    // Same length is not guaranteed in general, but these two payloads encode to the same size,
    // so any other divergence would mean the wrap touched the harness itself.
    expect(a.length).toBe(b.length);
    expect(a).not.toBe(b);
  });
});
