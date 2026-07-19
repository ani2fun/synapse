# Product assessment — is Synapse a product, and what would make it one

> **Nothing here changes code.** This is an honest read of Synapse as a *product* rather than
> as a codebase, done by counting what is actually in the repo and the content tree and by
> checking the market it would have to survive in.
>
> Companions: [production-readiness.md](production-readiness.md) asks "is this operable?" and
> [cutover-plan.md](cutover-plan.md) asks "is it safe to ship?". Both were answered yes. This
> document asks a different question — **is anyone on the other end of it?** — and the answer
> is currently no, for reasons that are structural rather than accidental.

## The verdict in one paragraph

The engineering is genuinely excellent and the product does not exist yet. Those are both true
and they are not in tension. synapse-rs was built as a learning exercise with Synapse as an
oracle, and it succeeded completely at that — the CI discipline, the cortex-goldens differential
test and the Java tracer are work most funded teams do not do. But the things that make software
a *product* — someone can find it, someone comes back, someone pays — were never in scope, so
they were never built. Three cheap changes turn it from invisible-and-unmeasurable into
something you can make a real decision about. Chasing the two most obvious-looking gaps would
be a mistake.

---

## Part 1 — what it actually is today

Counted from the repo and from `~/Development/homelab/synapse-content`, not estimated.

### The content is real

442 shipped lessons, ~757,000 words, across 7 shipped books. System Design from First
Principles (232 lessons, 355k words), Python (42 lessons) and Java (42 lessons) are
substantially complete. This is the least appreciated asset in the project: writing 757k words
of technical prose is years of work that no amount of engineering substitutes for.

### But the interactive surface is much thinner than the engineering implies

| Thing | Count |
|---|---|
| Runnable `run` fences | 727 (434 Python, 287 Java, 4 Rust, 1 Kotlin, 1 JS) |
| **Judged problems (`kind: problem`)** | **30** |
| `viz=` visualiser blocks | 152 |
| Quiz blocks | 251 |
| mermaid / d2 diagrams | 212 / 95 |

**Thirty judged problems exist in the entire corpus**, 29 of them in DSA. `dsa/03-basic-problem-set-1/`
looks like a fourth chapter but is untracked scaffolding — 78 files totalling 714 words.

### The visualiser covers 2 of 11 runnable languages

`server/src/execution/domain/language.rs` runs eleven languages. `client/islands/tracer/`
contains exactly five files, and `loader.ts` exposes only `loadWrapPython()` and
`loadWrapJava()`. There is no dispatch path for Scala, C, C++, Go, Rust, Kotlin, TypeScript,
JavaScript or SQL.

The content already reflects this: 721 of 727 run fences are Python or Java, and 100% of `viz=`
fences are. The flagship feature covers 18% of the execution surface, and each additional
language is a fresh multi-week harness.

### Nobody can find it

This is the largest single gap and it is absolute rather than partial.

- `client/index.html:8` is `<title>Synapse</title>`, hardcoded — and `document.title` is never
  set anywhere in `client/src` or `client/islands`. **All 442 lessons serve the identical title.**
- No `<meta name="description">`, no Open Graph tags, no Twitter cards, no canonical link.
- No `sitemap.xml`, no `robots.txt`. `client/public/` holds one file, `silent-check-sso.html`.
- `leptos` is CSR-only (`Cargo.toml:38`) with no `ssr` feature and no prerender step.
  `server/src/platform/static_routes.rs` returns the same empty `index.html` for every deep link.

Google does execute JavaScript, so these pages *can* be indexed — but 442 identical titles make
them indistinguishable in results, and social preview cards definitively do not run JS. There is
no organic acquisition channel. Not a weak one; none.

### Nobody comes back

- No progress, completion state, streaks or spaced repetition anywhere.
- **251 quiz blocks persist nothing.** `client/src/quiz/` is a single `mod.rs` with no storage.
- No product analytics. `platform/telemetry.rs` is `tracing` spans for request debugging, and
  it deliberately records the *matched* route rather than the concrete URI for cardinality — so
  it cannot tell you which lesson anyone read.
- The six localStorage keys are all cosmetic: theme, reader prefs, sidebar mode, problem tab,
  problem section, workbench language.
- The reading-progress bar (`client/src/catalog/view/chrome.rs:66`) is scroll offset. Nothing
  persists it.

A returning user is indistinguishable from a first-time user.

### Nobody can pay, and nobody else can author

Zero hits for stripe/billing/subscription/plan/paywall across all `.rs`, `.ts` and `.toml`. The
anon-vs-authenticated rate limits are abuse control, not tiers.

`content_root` is a single `String` (`server/src/config.rs:18`). One filesystem checkout, one
author's corpus, no tenant or org column in any migration, no authoring UI, no write routes for
content at all. Content changes by hand-editing markdown and git-pushing.

Signing in buys exactly three things: a bigger run budget (100/hour vs 10/minute), submission
history, and an `admin` flag. In production `SUBMISSION_ALLOWLIST_ENFORCED=true` makes it a
*gate* rather than a benefit — saving an attempt needs a manually granted allowlist row. The
deployed system is architecturally invite-only.

### The ceiling is one machine

`MAX_CONCURRENT_RUNS = 8` (`execution/infrastructure/runner.rs:21`) is the binding constant.
Reading is cheap — the catalog index is in memory and gzipped, so a single node serves thousands
of concurrent readers. Executing is the wall: eight simultaneous JVM compiles (up to 90–120s
each for Scala/Kotlin) saturate the platform. The in-process rate limiter and the submission
reconciler both assume `replicas: 1`, as `production-readiness.md` already documents.

Honest ceiling for one replica: a few hundred concurrent readers with dozens actively running
code. That is fine, and it is not the constraint that matters right now.

---

## Part 2 — the market it would have to survive

Researched from primary sources where possible. **Numbers flagged below as unverified should
not be repeated** — much of what surfaces on these searches is AI-generated SEO content with
confidently-stated fabricated figures.

### The consumer coding-education market is contracting, and AI is the mechanism

- Global edtech VC: **$16.7B (2021) → ~$2.8B (2024, 2025) → $1.0B in H1 2026, −26% YoY**
  (HolonIQ). Company formation collapsed from ~10,500 launches in 2020 to 645 in 2025.
- Crunchbase News (Nov 2025) names the category directly: investors are keen on healthcare
  education and AI-enabled K-12, while **"coding academies and teaching platforms"** face
  headwinds.
- **Skillsoft laid off Codecademy's entire curriculum team in February 2026** — confirmed
  publicly by a senior curriculum director. The best-capitalised consumer interactive-coding
  platform fired the people who write the lessons.
- Chegg is the canonical case: subscribers −40% YoY, 45% of staff cut in October 2025, stock
  down ~99%, fighting delisting. Two mechanisms — free ChatGPT substituting the product, and
  Google AI Overviews destroying top-of-funnel search.
- 2U filed Chapter 11 (July 2024) driven by a **40% drop in coding bootcamp enrollments**.
  BloomTech ceased operating after a CFPB order. Udacity was absorbed into Accenture. Coursera
  and Udemy announced a defensive all-stock merger for $115M of cost synergies.
- Stack Overflow questions fell from 200k+/month (2014) to under 50k by late 2025. Some decline
  predates ChatGPT, but the acceleration is unambiguous: **developers stopped going to websites
  to learn things.**

Counter-evidence, stated honestly: AI *content* demand is booming (Udemy AI enrollments +120%
YoY, Coursera GenAI enrollments doubled), and **LeetCode traffic grew 19% month-over-month in
June 2026**. Demand is rotating from "learn to code" toward "learn to use AI" — and interview
prep is holding up, probably because fewer junior roles means more competition per role.

### The two comparables that matter most

**Python Tutor** is the closest analog to the visualiser and the most important datapoint in
this document. Philip Guo (UC San Diego) has run it since 2010; his UIST 2021 paper reports
10M+ users, and the site now claims 25M+ users and 500M+ visualisations. In that paper he states
he could not get long-term funding and sustained it by "sneaking it into" a conventional
academic career — and observes that **despite billions in VC and big-tech money, no company has
built its own code visualization tool.**

That is simultaneously the strongest differentiation argument available and the strongest
evidence that this capability has never supported a business. Fifteen years, 25M users, zero
dollars.

**Runestone Academy** is the one that needs an answer. Brad Miller, 2011: open-source,
self-hostable, prose-first interactive textbooks with embedded runnable code, Parsons problems,
and CodeLens trace visualisation, plus PreTeXt authoring and a light LMS. Now NSF-supported
through the PROSE Consortium. That is close to a feature-for-feature match with Synapse minus
the AI coach and the diagrams, and it has existed for fifteen years as grant-funded open source.
"Ours is nicer" is not an answer to it.

### What the rest of the field looks like

| Player | Overlap | Note |
|---|---|---|
| **Educative.io** | Closest positioning — prose + embedded execution, and it already ships an AI Tutor | ~$14.6M raised, last round May 2021. Revenue figures circulating are unverified and implausible |
| **Exercism** | Free, prose + practice, 2M users | Publicly could not make payroll, Sept 2024. The CEO wrote that he had lost faith in the nonprofit model |
| **freeCodeCamp** | Free, huge | Runs on "a few hundred thousand dollars per year" in donations. Sets the consumer price floor at zero |
| **LeetCode** | Judged problems — ~3,500 vs Synapse's 30 | Growing 19% MoM. Revenue claims range 10x between sources; cite none of them |
| **VisuAlgo / algorithm-visualizer / USFCA** | Algorithm animation | All free, all unmonetized — and all animate *canonical implementations*, not the learner's own code |
| **DataCamp** | Adjacent | ~$100M ARR expected 2026, cashflow positive, ~6,000 B2B customers, on ~$32M raised. **The B2B business is what works** |
| **AlgoExpert / Boot.dev** | The realistic solo ceiling | $1.9M ARR (14 people, bootstrapped) and ~$6M ARR respectively |

The one genuine differentiator is tracing **the learner's own code** inside a prose lesson.
Shared only with Python Tutor and Runestone's CodeLens (which *is* Python Tutor, embedded).

### The answer to "how would you pitch investors"

Don't — not as a consumer learn-to-code product. The category is named by investors as out of
favour, the closest analogs are dead or unfunded, and there is no traction to show. The honest
seed bar in 2026 is roughly $300–500k ARR plus clear unit economics; Synapse has no revenue
mechanism at all. Every investor in this category now asks the same question: *if OpenAI shipped
your core feature tomorrow, would your users care?* — and "AI tutor" is table stakes, not
differentiation, since Educative already ships one.

There is a fundable adjacent story, and it points the engine somewhere else. **Mintlify** went
from ~$1M to ~$10M ARR in a year with 150% net revenue retention on ~$21M raised — and
Mintlify's API playgrounds **cannot execute arbitrary code**. Nor can ReadMe, Scalar or GitBook.
"Interactive documentation where the code samples actually run, and you can watch the data
structure move" is employer-paid, sits in a growing market rather than a contracting one, and
uses the tracer and viz engine exactly as built. That is a real wedge — but it is a different
company, and it should only be considered after Part 3 produces evidence.

### Two things that would come up in diligence

1. **414,000 words (35% of all prose) is gitignored**, and the largest chunk —
   `local-only/system-design-swiftly`, 66 lessons, 310k words — states in its own `book.json`
   that it is *"adapted from Hello Interview's 'System Design in a Hurry'"*, with lessons
   carrying their original video links. Inert today, and the gitignore is doing real work. It
   becomes a legal problem the moment money is involved.
2. **88 commits over five calendar days**, ~30,700 lines of Rust. That is only explicable as an
   AI-assisted port from a working oracle that carried its own test suite, and it should be
   presented that way deliberately — the alternative reading will not survive one follow-up
   question. The asset is the accumulated design carried from Cortex and Synapse, plus
   demonstrated ability to re-execute it, not these 30k lines.

---

## Part 3 — the ranked backlog

Ranked by leverage per unit of work, not by how obvious the gap looks. Two of the most obvious
gaps are recommendations to **not** build.

### 1. Measurement — nothing else can be prioritised without it

There is currently no way to answer "does anyone read this, and what do they read?" Every other
item below is a guess until that changes, which is why the smallest item is first.

The cheap, privacy-preserving version needs no third party, no client JS, no cookie banner and
no CSP change, because **every lesson view already flows through one endpoint we own**:

- Record `lesson_path` as a span field in the catalog lesson handler (`server/src/catalog/http/`).
  It has to be at the handler, not the layer — `platform/telemetry.rs` records the matched route
  by design, so the layer cannot see which lesson.
- Migration `0003`: an append-only `lesson_view(lesson_path, viewed_at, authed)` or a daily
  rollup. No user id, no IP. This is a content-popularity signal, not user tracking.
- One admin read: what gets read, and what has never been opened.

Roughly a day's work, and it converts every later decision from opinion into data.

### 2. Discoverability — the largest payoff, and full SSR is the wrong way to get it

Adding Leptos's `ssr` feature means hydration and restructuring the client, giving up the CSR
simplicity the entire build rests on. The right fix is **server-side meta injection**, which is
a string substitution:

- `server/src/platform/static_routes.rs:55` — `index()` currently discards the request path and
  serves raw `index.html` bytes through `serve()`. It needs the matched `rest` path and the
  catalog index, which the server already holds in memory
  (`catalog/application/service.rs:27`). Thread the catalog service in via the `AppDeps` wiring
  struct, substitute `<title>`, and inject description / OG / canonical tags before `</head>`.
- The index is already `no-cache` (`INDEX_CACHE`), so per-path HTML costs nothing.
- Add `/sitemap.xml` and `/robots.txt` generated from the same in-memory index. `SPA_SEGMENTS`
  enumerates rather than wildcards, so explicit routes cannot shadow `/api` — the
  Cortex-inherited lesson already recorded at the top of that file.
- Client half: set `document.title` and the description on navigation. Currently never set.
- Keep the `spawn_blocking` + canonicalize traversal guard in `serve()`.
- Pin with an IT asserting two different lesson URLs return two different titles.

### 3. A reason to come back — anonymous progress, in localStorage, not Postgres

The instinct is a `lesson_progress` table behind sign-in. That is the wrong first move: the
reader **is** anonymous, and an authed-only feature would reach almost nobody given the
allowlist.

- localStorage read/complete state, "continue where you left off" on the landing page, and
  completion ticks in the sidebar and the library grid.
- Use a **new key**. Do not extend the prefs blob — `client/src/catalog/logic/prefs.rs` parses
  positionally (`let [s,l,f,w] = … else { return DEFAULT_PREFS }`), so a fifth field silently
  resets everyone's saved settings. This is already a recorded lesson from step 46; don't
  re-learn it.
- Persist quiz results the same way. 251 quiz blocks currently produce zero signal, and "which
  questions do people get wrong" is content feedback that can be acted on.
- Server-side progress becomes worth building only if item 1 shows returning users who sign in.

### 4. Protect it — one Playwright smoke suite

The biggest rigour gap in the project. For a product whose entire value is interactive widgets,
all 48 chapters' "Verified live" sections are a human in a browser tab, and there is no e2e
automation at all. The existing gates — conventions, clippy, unit and integration tests, the
bundle budget — structurally cannot catch a widget that silently stops mounting.

Five paths are enough: a lesson renders · a run executes and returns output · Visualise produces
steps · ⌘K opens and navigates · the mobile drawer opens at 393px. Add as a CI job beside
`client-build`.

### 5. Deepen the 30 problems — do NOT chase problem count

Problem volume is the most commoditized axis in this market and it cannot be won. LeetCode has
~3,500 and is growing 19% MoM. Thirty problems where you can watch *your own code* build the
data structure is a better story than three hundred generic ones, and it plays to the one asset
nobody else has.

Spend the effort on `viz=` coverage and editorial depth for the existing 30, not on number 31.

### What not to build yet

- **More tracer languages.** Each is multi-week, and the corpus is already 99% Python/Java, so
  the marginal value against current content is near zero. Build a third only when a real reader
  or customer asks for it. If one is ever forced, JavaScript — it traces in-browser with no
  sandbox round-trip.
- **Multi-tenancy, a CMS, an authoring UI.** These only pay off if there will be other authors.
  There won't be, unless the positioning changes first.
- **Scaling past one replica.** Eight concurrent runs and an in-process rate limiter are correct
  for current load and already documented. Know the trigger; don't pre-solve it.

### One decision to record rather than build

The Hello Interview material. It is inert while gitignored and the gitignore is doing real work,
but leaving the status ambiguous is itself the risk. Write the decision down — personal study
only, never shipped — so it cannot drift into the product later by accident.

---

## What this project actually is

Worth saying plainly, because the list above is unrelenting and could be read as a verdict on
the work rather than on its scope.

Synapse is an exceptional portfolio artifact and a genuinely good personal learning platform. As
a demonstration of engineering judgement it is stronger than most production systems: the
architecture is enforced by CI rather than by review discipline, the port fidelity is pinned by
a cross-language differential test, and 48 chapters explain *why* every decision was made. The
Java in-sandbox recompile tracer, with its trace-lifetime-stable heap identity and the comment
explaining exactly which bug forced it, is the kind of thing that gets someone hired.

It is not a business, it was never built to be one, and the three items at the top of Part 3
are what it would take to find out whether it could become one. They total a few weeks, and they
are worth doing regardless of the answer — because right now the project cannot tell you whether
anyone is reading it.
