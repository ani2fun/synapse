#!/usr/bin/env bash
# ── BUNDLE BUDGET (oracle: ADR-S033 — 700 KiB gz critical path) ──────────────
# The critical path is what must arrive before the app boots: the Vite entry
# JS + the wasm module. Island chunks are lazy BY DESIGN (the loader pattern)
# and stay out of the sum. Budget inherited from the oracle: 700 KiB gzipped.
# Step-02 baseline: ~176 KiB gz (wasm ~169 + entry ~7) vs the oracle's ~610.
#
# Usage: check-bundle-budget.sh   (after `cd client && npm run build`)
#        BUDGET_KIB=500 check-bundle-budget.sh
set -euo pipefail

BUDGET_KIB="${BUDGET_KIB:-700}"
dist="client/dist/assets"
[[ -d "$dist" ]] || { echo "✗ no $dist — run 'cd client && npm run build' first"; exit 1; }

total=0
shopt -s nullglob
for f in "$dist"/index-*.js "$dist"/*_bg-*.wasm; do
  sz=$(gzip -c "$f" | wc -c | tr -d ' ')
  printf '  %s — %d KiB gz\n' "$(basename "$f")" "$((sz / 1024))"
  total=$((total + sz))
done
kib=$((total / 1024))

echo "critical path: ${kib} KiB gz (budget ${BUDGET_KIB} KiB)"
if ((kib > BUDGET_KIB)); then
  echo "✗ over budget — trim before it ships (wasm-opt flags, feature audit, chunk split)"
  exit 1
fi
echo "  ok"
