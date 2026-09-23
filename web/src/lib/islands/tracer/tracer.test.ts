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

/**
 * The input contract, asserted against the harness SOURCE.
 *
 * These are the lines the `/viz` panel's interactive stepping rests on: the harness must serve
 * input from stdin, log what it served, and stop rather than raise when stdin runs dry. None of
 * it can be checked by running the harness here (it needs the sandbox), and all of it fails
 * quietly — a harness that raised EOFError instead would surface as "the trace ended early",
 * which reads like the reader's program crashing.
 */
describe("the Python harness reports what it did with stdin", () => {
  it("reports the values it served, whether it is waiting, and what it asked", () => {
    // The three fields `HeapTrace` decodes. A rename here silently empties the input log and
    // the panel simply stops asking.
    expect(pythonHarness).toContain('"inputs": _syn_inputs');
    expect(pythonHarness).toContain('"waiting": _syn_waiting[0]');
    expect(pythonHarness).toContain('"prompt": _syn_prompt[0]');
  });

  it("replaces input() in the traced globals rather than leaving the builtin", () => {
    expect(pythonHarness).toContain('"input": _syn_input');
  });

  it("stops on an exhausted stdin instead of raising EOFError", () => {
    // BaseException, so a user's `except Exception` cannot swallow the one signal that tells
    // the client to ask for another line.
    expect(pythonHarness).toContain("class _SynAwaitInput(BaseException)");
    expect(pythonHarness).toContain("except _SynAwaitInput");
  });

  it("records WHICH step read each value, not merely that it was read", () => {
    // The panel walks the steps, so it has to know which values the program had reached by the
    // one on screen — without this every value reads as consumed from step 0.
    expect(pythonHarness).toContain('_syn_inputs.append({"v": value, "at": max(len(_syn_steps) - 1, 0)})');
  });

  it("reports the exception that ended the run, and where a syntax error was", () => {
    // Without these a crash is silent: the trace holds every step up to it, so the reader steps
    // to the end of a story that simply stops, with nothing saying it broke.
    expect(pythonHarness).toContain("except BaseException as _syn_dead:");
    expect(pythonHarness).toContain("except SyntaxError as _syn_bad:");
    expect(pythonHarness).toContain('"error": _syn_error[0]');
  });

  it("trims the steps an uncaught exception leaves behind as it unwinds", () => {
    // A `return` per frame at the line that raised. Kept, they end the story on a return the
    // reader never wrote, with both debugger arrows on one line.
    expect(pythonHarness).toContain("del _syn_steps[_syn_raised_at[0] + 1:]");
  });

  it("trims from the FIRST frame of a propagation, and forgets one that got caught", () => {
    // The event fires again in every frame the exception passes through, so overwriting lands on
    // the outermost — past the very returns the trim exists to remove. And a `line` or `call`
    // after it means execution resumed, which is the only signal that it was handled.
    expect(pythonHarness).toContain("if _syn_raised_at[0] is None:");
    expect(pythonHarness).toMatch(/if event in \("line", "call"\):\n\s+#[^]*?\n\s+_syn_raised_at\[0\] = None/);
  });

  it("records how far the program's output had got at every step", () => {
    // What lets the client fill an output box AS the reader steps, instead of handing them the
    // whole run's answer at step 0.
    expect(pythonHarness).toContain('"out": _syn_stdout.written');
    expect(pythonHarness).toContain('self.written += len(text.encode("utf-8", "replace"))');
  });

  it("names what a frame returned, but never the module's implicit None", () => {
    expect(pythonHarness).toContain('specs[0] = (name, items + [("Return value", arg)], comp)');
    expect(pythonHarness).toContain('frame.f_code.co_name != "<module>"');
  });

  it("records nothing more once the program is waiting", () => {
    // _SynAwaitInput unwinds the stack, and every frame it passes fires a `return` at the line
    // that asked. Recorded, those become steps the reader never wrote, and the last two land on
    // the same line — so the arrow for "just executed" and the one for "next" point at one row.
    expect(pythonHarness).toContain("if _syn_waiting[0]:\n        return _syn_tracer");
  });

  it("hides the injected name from the reader's own locals", () => {
    // Without this, every Global frame lists an `input` the reader never wrote.
    expect(pythonHarness).toContain('_syn_hidden = frozenset(("input",))');
  });

  it("names a class for itself rather than for its metaclass", () => {
    // `type(Solution).__name__` is "type", which tells a reader nothing about the box their
    // variable points at.
    expect(pythonHarness).toContain('v.__name__ + " class"');
  });
});

/**
 * The shape contract, asserted against the harness SOURCE: what a frame and an object carry so
 * the memory lens can draw the program the way it was written. The behaviour itself needs a real
 * interpreter and is pinned by `server/tests/python_tracer_it.rs`; these catch a rename that
 * would silently strand the decoder.
 */
describe("the Python harness reports the program's own shape", () => {
  it("does not step a class body", () => {
    // A class body runs once, at the `class` line; walking it shows a frame named after the class
    // stepping through `def` lines that define rather than run.
    expect(pythonHarness).toContain("_SYN_CO_OPTIMIZED = 0x0001");
    expect(pythonHarness).toMatch(/if event == "call" and not \(frame\.f_code\.co_flags & _SYN_CO_OPTIMIZED\)/);
  });

  it("reads the module's names from its globals while a comprehension is inlined into it", () => {
    // 3.12+: mid-comprehension, a module frame's f_locals holds ONLY the comprehension's variables.
    expect(pythonHarness).toContain('if cur.f_code.co_name == "<module>" and local is not cur.f_globals:');
    expect(pythonHarness).toContain('entry["comp"] = names(comp)');
  });

  it("names a function by its signature and gives the reader's class its members", () => {
    // The decoder reads exactly these keys: `sig`, and `name` + `members`.
    expect(pythonHarness).toContain('{"type": "function", "sig": _syn_signature(v)}');
    expect(pythonHarness).toContain('{"type": "class", "name": v.__name__, "members": members}');
    expect(pythonHarness).toContain('getattr(v, "__module__", None) == "__main__"');
  });

  it("lists the dunder methods the reader wrote, and none of the ones Python adds", () => {
    // `__init__` is the reader's; `__module__`, `__qualname__` and `__dict__` are bookkeeping.
    expect(pythonHarness).toContain('if mk.startswith("__") and not isinstance(mv, types.FunctionType):');
  });
});
