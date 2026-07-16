# Step 13 — The submission aggregate and the async judge

*(oracle: synapse step 14 — `Submission`/`SubmissionState`/`SuiteOutcome`, `SubmitSolution` +
ports, `SubmitSolutionSpec`; plus the shared `TestSpec`/`TestRun` judging vocabulary from the
execution island that both the workbench and the judge reuse)*

## The shared judging vocabulary (`shared/execution/test_run.rs`)

`TestSpec`/`ArgSpec`/`TestCase` — the authored suite, with the oracle's Scala-keyword dodge kept
on the wire (`type` in JSON ↔ `tpe` in code, pinned by a round-trip test). `stdin_for` — ONE
line per declared arg in declaration order, missing values become empty lines, trailing newline.
`judge` — non-clean run → `Errored`; clean with no expected → `Finished` (counts as a pass);
otherwise trimmed stdout comparison.

## The domain: a state ADT, not a status column

`Submission` carries the catalog path, the fence alias (resolved by the EXECUTION context, not
here), and the **anonymous seam** — `user_id: Option<String>`, `None` until identity fills it.
`SubmissionState` is `Pending | Judging | Completed{outcome, at}` — a verdict on a pending row
is unrepresentable in memory; Postgres flattens at the edge only (next step).
`SuiteOutcome::{Accepted, Rejected, JudgeFailed}` encodes the judging contract: run in AUTHORED
ORDER, stop at the first failure, `passed` = consecutive passes from the top (the "8/118"
semantics); `JudgeFailed` is machinery trouble, never a verdict on the code.

## The application: submit → 202 → detached judge → poll

`SubmitSolution<Repo, Tests, Runner>` clones as `Arc` handles, which is exactly what lets
`submit` fire `judge_and_complete` as a DETACHED `tokio::spawn` (the oracle's `forkDaemon`) and
answer immediately. The judge drives the execution context's OWN `RunCodeService`
(customer–supplier — never a duplicated runner). `judge_and_complete` is infallible with a
backstop: a store failure records `JudgeFailed` best-effort so a row is never left stuck on
Judging. Ports (`SubmissionRepository` with `by_user` scoping, `ProblemTests`) already carry the
identity seams so that step slots in without reshaping the aggregate.

## Tests

Nine behaviors over in-memory fakes, with `judge_and_complete` driven DIRECTLY for determinism
(the detached task stays fire-and-forget, exactly the oracle's testing stance): not-a-problem
stores nothing · pending+anonymous persisted · all-pass Accepted in authored order with the
exact stdin shapes ("0\n"…) · stops at first failure (the third case never runs — call-count
pinned) · a crash is a Rejection carrying the crash status · machinery failure mid-suite is
JudgeFailed with passes-so-far · no-expected counts as a pass · judging→completed state walk
never sticks · unknown get is 404-shaped. Suite: 138 Rust + 40 vitest.

Next step: the Postgres repository (sqlx, the JSONB outcome flatten), `FileSystemProblemTests`
(sidecar-or-fence), and the 202/poll HTTP surface.
