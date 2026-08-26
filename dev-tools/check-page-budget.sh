#!/usr/bin/env bash
# ── PER-PAGE EAGER-JS BUDGET ───────────────────────────────────────────────────
# Astro ships each page only its own eager assets — no single bundle serves every page — so the
# honest measure is PER PAGE — fetch the page, collect what its HTML makes the browser download
# before the reader sees content (module scripts + stylesheets), gzip-sum them. Lazy islands
# (Monaco, keycloak-js, mermaid, the viz wasm) are dynamic imports and never appear in the HTML —
# they stay out of the sum BY CONSTRUCTION, not by glob.
#
# Six page kinds, one budget each: the landing, a prose lesson, a problem page, the blog list,
# and the two diagram editors.
# Measured against fixture content, in the shape this script runs in (pre-rendering OFF):
# landing 43 · lesson 54 · problem 52 · blog 13 · d2lab 47 · mmdlab 47 KiB gz. With pre-rendering ON the
# lesson measures ~64 — inline SVG, counted in the document's own gzip, not new JavaScript.
# (Astro 7 / Vite 8 took 1-2 KiB off every page: rolldown chunks the same graph slightly tighter.)
# These drift with ordinary feature work (the reader redesign, the long-form stylesheet and the
# authoring context each moved them a KiB or two) — re-measure when you touch this line rather
# than assuming a rise is a regression. The signal is the 250 KiB cap, not the last recorded digit.
#
# Measured in the PROD shape, which `dev-tools/e2e` now runs by default: d2 pre-rendering is off
# in the cluster, so a ```d2 fence ships as a source placeholder and costs this gate almost
# nothing. Turn it on and the lesson jumps to ~61 KiB — server-drawn figures are inline SVG, about
# 4-5 KiB gz each — so if that ever becomes the deployed shape, re-measure before reading the rise
# as a regression. A diagram-dense lesson would be a real weight, and a fixture page is the only
# place it is measured at all.
# Budget 250 KiB: generous headroom over the heaviest page (~5×). If a page ever approaches this
# budget, something structural regressed (an island went eager); tighten, don't raise.
#
# The viz wasm cap rides along when the pkg is a RELEASE build (VIZ_WASM_RELEASE=1 — CI's e2e
# builds release; a local dev-profile pkg would fail the cap for being unoptimized, which is
# noise, not signal).
#
# Usage: check-page-budget.sh [base-url]   (default http://localhost:8280; stack must be UP)
#        PAGE_BUDGET_KIB=250 VIZ_WASM_BUDGET_KIB=350 to override.
set -euo pipefail
cd "$(dirname "$0")/.."

BASE="${1:-http://localhost:8280}"
PAGE_BUDGET_KIB="${PAGE_BUDGET_KIB:-250}"
VIZ_WASM_BUDGET_KIB="${VIZ_WASM_BUDGET_KIB:-350}"

# The four page kinds. The lesson/problem paths are the FIXTURE's (this runs inside
# dev-tools/e2e); against real content pass SYNAPSE_ROOT + override these.
LESSON_PATH="${BUDGET_LESSON:-/synapse/learn/smoke/intro}"
PROBLEM_PATH="${BUDGET_PROBLEM:-/synapse/learn/smoke/problems/threshold/threshold}"

fail=0
echo "→ per-page eager-JS budget (${PAGE_BUDGET_KIB} KiB gz each) against $BASE"

measure_page() {
  local label="$1" path="$2"
  local html total sz kib
  html=$(curl -sf "$BASE$path") || { echo "  ✗ $label ($path) did not serve"; return 1; }
  total=0
  # Everything the HTML loads eagerly: script src + rel=stylesheet href. Astro emits
  # root-relative /_astro/... URLs (the pattern also matches a conventional /assets/... path).
  # Inline hoisted scripts are part of the HTML itself — counted via the document's own gz size
  # below.
  local assets
  assets=$(printf '%s' "$html" |
    grep -oE '(src|href)="/(_astro|assets)/[^"]+\.(js|css)"' |
    grep -oE '/(_astro|assets)/[^"]+' | sort -u)
  sz=$(printf '%s' "$html" | gzip -c | wc -c | tr -d ' ')
  total=$((total + sz))
  for a in $assets; do
    sz=$(curl -sf "$BASE$a" | gzip -c | wc -c | tr -d ' ') || { echo "  ✗ $label: $a 404s"; return 1; }
    total=$((total + sz))
  done
  kib=$((total / 1024))
  local count
  count=$(printf '%s\n' "$assets" | grep -c . || true)
  printf '  %-8s %s — %d assets, %d KiB gz\n' "$label" "$path" "$count" "$kib"
  if ((kib > PAGE_BUDGET_KIB)); then
    echo "  ✗ $label is over the ${PAGE_BUDGET_KIB} KiB page budget"
    return 1
  fi
}

measure_page "landing" "/" || fail=1
measure_page "lesson" "$LESSON_PATH" || fail=1
measure_page "problem" "$PROBLEM_PATH" || fail=1
measure_page "blog" "/blog" || fail=1
# The two diagram editors. Everything heavy they use — Monaco, the d2 engine, mermaid — is behind
# a dynamic import, so this measures whether it STAYS there: a static import of any of them would
# show up here as several hundred KiB rather than as a slow first draw. Both pages mount the same
# shell, so a divergence between these two numbers is itself the signal.
measure_page "d2lab" "/d2" || fail=1
measure_page "mmdlab" "/mermaid" || fail=1

# ── the lazy viz wasm cap (release builds only — see header) ──
if [[ "${VIZ_WASM_RELEASE:-}" == "1" ]]; then
  wasm="web/src/lib/viz-wasm/pkg/viz_wasm_bg.wasm"
  if [[ -f "$wasm" ]]; then
    kib=$(($(gzip -c "$wasm" | wc -c | tr -d ' ') / 1024))
    echo "  viz wasm (lazy) — ${kib} KiB gz (cap ${VIZ_WASM_BUDGET_KIB})"
    ((kib > VIZ_WASM_BUDGET_KIB)) && { echo "  ✗ viz wasm over its cap"; fail=1; }
  else
    echo "  ✗ VIZ_WASM_RELEASE=1 but no $wasm"
    fail=1
  fi
fi

((fail == 0)) && echo "  ok"
exit $fail
