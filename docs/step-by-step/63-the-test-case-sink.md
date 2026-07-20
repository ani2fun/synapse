# Step 63 — The test-case sink

*(a rejection told you which case broke and then left you holding it.)*

## The dead end

Submit judges against a hidden suite. When it rejects, the verdict names the first failing case and
shows what your code printed — and there the trail stops. The one thing a learner wants next is to
run *that* input locally and watch it fail in front of them, and there was no way to do it. You
could read the failure; you could not work on it.

The fix turned out to be almost entirely a rendering problem. `FailedCaseDto.args` has carried the
failing input end-to-end since step 13 — judge → domain → JSONB → wire → `SubmitState::Done` — and
`done_panel` binds the failure and renders its index, expected, stdout and stderr while never
touching `args`. **Zero server change.** The data was already in the browser, one field away from
being useful.

## Cases had to become appendable

`TestsState.values` is `RwSignal<BTreeMap<String, String>>` and `FailedCaseDto.args` is
`BTreeMap<String, String>` — byte-for-byte the same type. Seeding the grid is a `set`. What was not
free was giving the panel a *new chip*: the suite arrived as `StoredValue<TestSpec>`, which is
non-reactive, and the chip row was a `Vec` materialised once at component build. The count was
frozen twice over.

So `spec` is now an `RwSignal<TestSpec>` and the chip row is a closure over its length. The
alternative — an `extra: Vec<TestCase>` beside the authored ones — was rejected because it does not
avoid the reactive-chips rewrite (that is the bulk of the work either way) and buys only nominal
immutability, while costing a second index space that four call sites would have to translate
between. Promotion keeps `seed_values`/`expected_for` byte-identical and keeps one index space.

The immutability objection is nominal rather than structural: `RunnableBlock` takes
`spec: Option<TestSpec>` **by value**, and Submit sends only `(path, language, source)` — the server
re-fetches its own suite. Nothing outside the block can observe the mutation, and no locally added
case can ever reach the judge.

One read is deliberately untracked. The verdict `Effect` calls `expected_for(&spec.read_untracked(),
case)`; a tracked read there would make the Effect a dependent of the spec and re-run it on every
append.

## One append path, because two things silently rot

Three routes add a case now — the verdict panel's button, a Submissions row's button, and the bare
`+`. All three go through `CaseSink::append`, which exists to make two mistakes impossible:

**Append, never insert.** `TestsState.verdicts` is a sparse map keyed by case index. Inserting
mid-list would slide every existing ✓/✗ onto the wrong chip — the badges would still be there, still
look authoritative, and be one case out.

**Route through `on_switch`.** The output panel judges against `ran_case`, not `active_case`. Append
without firing the callback the chips use and a previous case's result stays on screen, still
labelled with *its* expected, while the new chip sits selected. Nothing errors; the panel just lies
until the next Run.

## The guard, and why the button is disabled rather than hidden

Problems are judged against a two-tier suite: a `<stem>.tests.json` sidecar when one exists,
otherwise the authored ```` ```testcases ```` fence. For sidecar problems the judged suite is larger
and invisible, and its arg ids need not match the fence's. Copy misaligned args and the values land
under keys with no input field, while `stdin_for` — which iterates the *visible* args — feeds the
program something the judge never fed it. Silently wrong, which is the worst kind.

`can_reproduce` is the guard: every declared arg must have a value in the failure. Extra keys are
fine, since `stdin_for` ignores them.

When it fails the button renders **disabled with a tooltip**, not hidden. On the one problem class
where this happens, a vanished button reads as a bug, and the tooltip is the only place the learner
can find out why. There is a residual the guard cannot cover — a sidecar may declare the same ids in
a different *order*, and the client never sees the judged args — so the button's copy promises to
reproduce the *input*, not the judge's exact stdin.

The failing case number is the *judged* suite's index. It is shown in the failure card, where it is
true, and never used as a chip label: on a sidecar problem "case 7" would append as visible
"Case 4".

## The `+` and its deliberate lack of a badge

The bare `+` appends `TestCase { args: {}, expected: None }` — blank fields, no Expected block. Run
it and it gets **no badge at all**: `judge` returns `Finished` when there is nothing to compare
against, and only `Accepted`/`WrongAnswer`/`Errored` paint a chip. That is right. A case with
nothing to check cannot pass or fail. A crash on it still yields `Errored` → ✗, which is the signal
worth having.

## `Output` moved out first

`runnable.rs` stood at 786/800 and already carried `#[allow(clippy::too_many_lines)]`. This step
needed room, so it opens by moving `Output` + `error_panel` + `result_panel` + `stream_block` — 94
self-contained lines — into `execution/view/output.rs`.

That seam was not the one predicted in step 47. The guess then was the language toolbar; the honest
cut is `Output`, which both module docs had already named as a separate oracle unit
("`Workbench` + `WorkbenchOutput`"), which was already `pub(crate)`, and which `codebench.rs` was
already importing across the module boundary. The toolbar stays put — extracting it means threading
four closed-over values through a signature. `runnable.rs` ends the step at 706/800.

`TestsPanel` then tripped the 100-line function cap, so the values grid became `value_fields`.

## Verified

Gates: conventions (runnable.rs 706/800), fmt, clippy, **463 rust** (+5) + 83 vitest.

Live, anonymously: the `+` appends Case 5 to a four-case problem, selects it, blanks the fields and
drops the Expected block; filling it and Running lands a result with **no chip badge**; switching
back to Case 1 re-seeds `95` / `Grade A` and clears the stale output while Case 5 survives. The
codebench modal (the extracted `Output`'s `spec=None` consumer) still opens with Monaco.

**Not verified live, and worth stating plainly:** the two submit-driven buttons need a signed-in
session, since Submit is auth-gated. They share `CaseSink::append` with the `+` — which is exercised
above — and `can_reproduce` is unit-tested against all five shapes, but the rendering of a real
`FailedCaseDto`'s args and the disabled state on a sidecar problem have not been seen in a browser.

## The lesson

**The data was already there.** Three steps' worth of "we should surface the failing case" was, in
the end, one unread field on a DTO that had carried it correctly since step 13. Before designing a
wire change, read what the wire already carries — `done_panel` had `f.args` in scope and in reach
the whole time.
