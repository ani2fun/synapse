# Step 55 — The tracer wrap, and a comment in the wrong file

*(nine tests on forty lines of TypeScript — everything they cover fails silently.)*

## Why these forty lines

The backlog item said "unit-test the untested TypeScript islands", and after step 52 most of
that scope evaporated: the e2e suite now exercises Monaco, mermaid and d2 at the level that
matters — do they mount, do they render. Duplicating that in jsdom would be work without cover.

What e2e **cannot** see is the tracer wrap, because everything it does fails *silently*. A wrap
that leaves a placeholder behind still compiles and still runs; it simply traces nothing, and
the Visualise modal shows an empty card that reads like the user's program was at fault. A trace
with the wrong contents is still a trace.

The scope narrowed to two files totalling about forty lines, and they turned out to be carrying
three contracts with nothing holding them.

## What I expected to test, and was wrong about

I had this down as "the budget arithmetic" — 600 steps, 400 objects, 60 depth, the 512 KB
quarter-drop-tail. That was wrong, and worth stating plainly because it changed the step. Those
budgets live in `python-harness.py` and `java-harness.java`, which are Python and Java resource
files. vitest cannot reach them; only a real traced run can, which is a sandbox-gated e2e
concern, not a unit one.

What the TypeScript actually does is smaller and more fragile: base64-encode the user's source
and substitute it into the harness.

## The three contracts

**`replaceAll` is load-bearing — in the file that did not explain it.** `java-harness.java`
names the placeholder twice: once in a header comment on line 3, once in the `USER_SOURCE_B64`
constant on line 80. `replace` would substitute the *comment* and leave the constant holding the
literal string, so the harness would compile, run, and dutifully decode a placeholder as if it
were the user's program.

The comment explaining this was in **`python.ts`** — where `python-harness.py` mentions the
placeholder exactly once and `replace` would have been perfectly equivalent. `java.ts`, where
the trap is real, had `replaceAll` and no explanation at all. Both are corrected, and the
behaviour is now pinned in both files rather than asserted in prose.

**`btoa` cannot encode non-ASCII.** It accepts Latin-1 and throws above U+00FF, so a naive
`btoa(source)` would not corrupt a trace — it would break the wrap outright the first time
anyone wrote a non-English string or an emoji. Both wrappers already encode UTF-8 bytes first;
now three tests prove the round trip survives `héllo · 世界 · 🎉` and that neither wrapper throws.

**The Java sentinel is a cross-language contract with no compiler on either side.** The server's
`JAVA_TRACER_SENTINEL` (`execution/infrastructure/java_rewriter.rs`) matches the exact string
`// __SYNAPSE_TRACER__` to know that traced Java already defines `Main` and must pass through
*without* entrypoint rewriting. That string is line 1 of a `.java` resource in the client. Add a
licence header, reformat the file, or let an editor insert a leading blank line, and traced Java
silently starts getting rewritten. Two tests: the harness's first line is that string byte for
byte, and it survives the wrap.

## What this deliberately does not do

**No tests for monaco, mermaid or the d2 wrapper.** Step 52 covers them where it counts. Testing
a thin loader against a mock of the library it loads asserts that the mock was configured.

**No jsdom DOM tests.** These are pure string functions; the moment a test needs a document, it
belongs in e2e.

**No assertion on the harness budgets.** They are not reachable from TypeScript. Proving them
needs a real traced run against the sandbox, which belongs behind `E2E_SANDBOX` with the other
gated suites.

**No shared `utf8ToBase64`.** It is nine identical lines in both wrappers. Extracting it would
create a module that exists solely to be shared by two files that are already deliberate twins —
the same reasoning that let step 18's blog carry its own fence parser rather than reach into the
catalog's.

## Verified

```
Test Files  3 passed (3)
Tests      83 passed (83)      ← was 74
```

The Java placeholder test asserts the harness mentions it **more than once** before asserting
the wrap removes it — so if a future edit collapses it to a single mention, the test that
protects `replaceAll` starts failing rather than quietly becoming vacuous.

435 rust + 83 vitest (+9) + 7 e2e.

## The lesson

**A comment is not a test, and it is not even reliably in the right file.** The `replaceAll`
explanation was accurate, carefully written, and attached to the wrapper where the hazard does
not exist — while the wrapper that genuinely depends on it had nothing. Nobody misled anybody;
the two files are near-identical twins and the comment was written once and pasted where it
happened to land. Prose drifts silently because nothing checks it. Writing the test is what
made me count the occurrences, and counting is what found it.
