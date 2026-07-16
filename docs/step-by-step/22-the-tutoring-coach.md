# Step 22 — The tutoring coach: a local Socratic hint machine, off by default

*(oracle: synapse step 20 / ADR-S025, with the post-step "coming soon" off-copy folded in as
final design.)*

## The context (server)

Domain-free hexagon (a chat turn's role+content IS the whole model): `TutoringService` owns
the SYSTEM PROMPT — steering, never scoring ("nudge them toward their OWN solution — never
hand over a complete, working answer"); a learner can never be blocked by it. The context
folds in per turn: the problem path, then the current editor code as a fenced block (blank
code = absent — no empty fence). History passes through untouched: the client sends the FULL
transcript every turn, the server is stateless, the conversation dies with the page.

The adapter speaks **`POST /v1/chat/completions`** — the OpenAI-compatible shape Ollama,
LM Studio, and vLLM all serve — non-streaming, no options, the system turn always first.
HTTP/1.1 forced (the go-judge h2c lesson), 10 s connect / 60 s per request (local CPU
inference is slow — generous, not infinite). Connect/timeout failures → 503 with the
`TUTOR_URL` hint; everything else (non-2xx, unparsable reply) → 502.

**Disabled is structural, not an error**: `TutorError` deliberately has no `Disabled` case —
when `TUTOR_ENABLED` is off (the default; prod never sets it, Ollama isn't a prod service),
the chat route is simply NEVER MOUNTED. `/api/tutor/config` always answers; `/api/tutor/chat`
404s because it doesn't exist. Config: `TUTOR_ENABLED`/`TUTOR_URL`/`TUTOR_MODEL`
(`false` / `http://localhost:11434` / `llama3.1`).

## The Coach pane (client)

A flat thin feature on problem pages: config fetched on mount, and ANY failure falls to Off —
never a chat box that 404s. Off shows exactly **"The coach is off / This feature is coming
soon."** — what prod renders. On: the model badge, the bubble log (user right / assistant
left, an italic "…" while sending, a calm error card on failure — the user's bubble stays),
and the composer (Enter sends, Shift+Enter newline). The editor snapshot rides a
`code_ctx: RwSignal<(String, String)>` threaded through hydration into `RunnableBlock` —
seeded from the variant, updated on every Monaco edit, and read UNTRACKED at send time (a
snapshot, not a subscription).

## Tests + verified live

+15: wire 5 (system-turn-first order pin, empty history, the reply parse + two loud
failures), service 5 (prompt folding over a capturing fake — bare/path/fence/blank-code/
error-passthrough), route ITs 5 (config answers both ways · delegate · 503-with-hint/502 ·
the structural 404 · the assembled dev app end-to-end). Suite: 231 Rust + 40 vitest; 439/700
KiB gz. Verified live BOTH ways: default server → the off-note renders on the problem page +
curl chat 404; then `TUTOR_ENABLED=true TUTOR_MODEL=gemma4:latest` against the REAL local
Ollama → "I'm stuck — where should I start?" came back with a genuinely Socratic,
problem-aware reply ("Since you need to reverse the array *in place*, think about what two
pointers are used for. Where do your left pointer …") — the folded problem path + starter
code visibly steering the model.

RS-P6 is COMPLETE. Next: RS-P7 — the viz engine, pure Rust against the cortex-goldens.
