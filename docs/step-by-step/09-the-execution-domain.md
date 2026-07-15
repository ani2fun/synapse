# Step 09 — The execution domain: languages, run vocabulary, the port

*(oracle: synapse step 09 — `Language`, `RunStatus`, `BackendLimits`, `RunCodeService`,
`CodeRunner`, `ExecutionError`; `LanguageSpec` + `RunCodeServiceSpec` + `RunStatusSpec` ported)*

## The wire vocabulary (`shared/src/execution.rs`)

- **`RunStatus`** — `Accepted · CompileError · RuntimeError · TimeLimitExceeded · InternalError`,
  crossing the wire as the CASE NAME string, never a Judge0-style magic int (the quality bar's
  canonical example, kept). `is_success()` is true only for `Accepted`; labels are display-ready.
- **`RunRequest { language, source, stdin? }`** — `language` is a fence alias, resolved
  server-side. **`RunResult`** — camelCase wire fields (`compileOutput`, `timeSeconds`,
  `memoryKb`); the measurements are optional (absent when the backend didn't measure).
- **`GO_JUDGE_LIMITS`** — the sandbox's hard edges, hardcoded exactly like the oracle's
  `BackendLimits.goJudge`: 1 MiB stdout · 64 KiB source · 16 KiB stdin · 10 s default timeout.

A badly-running program is a **200 with a non-`Accepted` status** — the run path's central
design decision, stated here and enforced by the port's contract.

## The domain (`execution/domain/language.rs`)

`Language` — eleven runnable languages as an enum with display labels and fence aliases
(`py`/`python3`, `c++`/`cxx`, `node`, …). `resolve()` trims, lowercases, and returns `None` for
blank/unknown. A test pins global alias uniqueness and round-tripping; downstream exhaustive
matches (the go-judge recipes, step 10) make "added a language, forgot the recipe" a compile
error.

## The application (`execution/application/`)

`CodeRunner` — the output port (native async-fn-in-trait, generic service, like the catalog's):
returns `RunResult` even for failed programs; only backend machinery uses
`ExecutionError::{BackendUnavailable → 503, BackendFailed → 502}`. `RunCodeService` is
deliberately thin — resolve (`UnknownLanguage` → 422), enforce the byte caps (**UTF-8 byte
count, INCLUSIVE** — at-limit passes, one over fails: `PayloadTooLarge` → 413), then run. No
go-judge knowledge, no concurrency gate — the adapter owns those in step 10.

## Tests

12 new: 4 language (aliases, case/trim, unknown/blank, global uniqueness), 5 service over a
recording fake runner (unknown language never reaches the runner; oversized source/stdin
rejected before running; inclusive caps; resolved language + payload arrive verbatim; backend
failures propagate), 3 shared wire pins (case-name status JSON, camelCase result fields,
success/labels). Suite: 87.

## Verified

`cargo test --workspace` 87 green; clippy `-D warnings`; purity/caps/fmt green. (No HTTP surface
yet — the endpoint, adapter, and go-judge ITs land in step 10.)
