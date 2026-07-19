# Step 56 — The ADR index, and a document that lived on one machine

*(105 citations pointing at nothing a reader here could open.)*

## The defect

`docs/adr/` held two decisions. The chapters and the code cite **nineteen more** — `ADR-S007`,
`ADR-S010`, `ADR-S026` and the rest — **105 times**, and none of them resolved to anything a
reader of this repository could open. The oracle's `docs/adr-synapse/` is not vendored, and a
citation carries a number but no title, so following one meant already knowing what it said.

For a project whose whole proposition is that the reasoning is followable — 55 chapters
explaining why each decision looked right at the time — a dead link to its most-cited authority
is a real defect. It only bites a *second* reader, which is exactly why it survived fifty-five
steps: the author never needed it.

## What it is not

**They are still not vendored, and that is the decision rather than an omission.** Copying
thirty-three ADRs here would create a second copy that drifts from the oracle silently, and the
oracle is the authority — the same reasoning that made these citations rather than restatements
in the first place. Copying would fix the symptom by creating the disease.

What `docs/adr/README.md` gains instead is an **index**: every S-number this repository actually
cites, its real title taken **verbatim from the source**, and how often it is leaned on. Titles
were extracted from the oracle file programmatically rather than retyped, because nineteen
hand-copied titles is nineteen chances to paraphrase one into something subtly wrong.

The count column earns its place. `ADR-S010` (the content directory layout) is cited 17 times
and `ADR-S026` 15 — those are the load-bearing inherited decisions, and the table says so
without anyone having to assert it.

Worth recording: **all nineteen resolved.** I half expected a citation to a number that never
existed, given they were never checkable. There wasn't one.

## The other half

`docs/product-assessment.md` — the document that produced steps 48 through 56 — **existed on one
machine and was never committed.** It has been the most-referenced artifact of the last nine
steps and a disk failure would have taken it.

That is a smaller-sounding problem than it is: every one of those steps opens by justifying
itself against that assessment, so without it the chapters cite a document that is not in the
repository either. Same defect as the ADRs, one level up.

## What this deliberately does not do

**No per-ADR anchor links.** The oracle keeps all thirty-three in one 1204-line README with
`## ADR-S0NN — Title` headings. GitHub anchors would be reproducible but fragile to a heading
edit, and the oracle repo is local — a URL would be a guess. The index gives the file path and
the exact heading text, which is searchable and cannot rot into a wrong-but-plausible link.

**It does not list all thirty-three.** Fourteen are Scala/Laminar-specific or superseded by an
RS decision; including them would pad the table with rows no chapter cites.

**It does not restate any ADR's content.** A one-line summary alongside the title would be
useful right up until the oracle revises the decision and the summary quietly disagrees with it.

## Verified

```
19 distinct ADRs cited, 105 citations total
19 resolved against the oracle, 0 unresolvable
33 ADRs exist upstream; 14 uncited here
```

Conventions clean. No code changed, so the suite is unmoved: 435 rust + 83 vitest + 7 e2e.

This commit touches only `docs/`, which makes it the first live exercise of step 48's release
gate — `changes` should report `code=false` and the release should stand down. That path was
proven against git history at the time but has never actually fired in CI.

## The lesson

**Documentation rots from the reader's side, not the writer's.** Every one of those 105
citations was correct when written and stayed correct — the ADRs exist, the numbers are right,
nothing drifted. They were still useless to anyone but the person who already knew the answer,
because the thing they pointed at was not reachable from where they sat. The assessment doc is
the same failure with the pointer inverted: perfectly good content, in a place the repository
could not reach.

Neither shows up in a gate, a test, or a review of the diff that introduced it. The only thing
that surfaces it is someone trying to follow a link — which, in a single-author repository, is
nobody, until it is somebody.
