#!/usr/bin/env bash
# ── CONVENTION GATE (RS001 · hexagon purity · three-layer purity · file caps) ─
# The Rust edition of Synapse's gate — the conventions that must never be green
# by discipline alone:
#
#   1. SERVER DOMAIN PURITY: files under any server `domain/` layer neither
#      import NOR name axum / tower / hyper / tokio / sqlx / reqwest / utoipa —
#      the domain is pure Rust (std + serde at most), and such a reference
#      means a port was skipped.
#   2. VIZ ENGINE PURITY: files under viz-wasm/src/engine/ neither import nor
#      name leptos / web-sys / wasm-bindgen / js-sys / gloo — the engine stays
#      pure and native-testable.
#   3. FILE-SIZE CAPS: server & shared ≤ 500 lines/file, viz-wasm & web ≤ 800 —
#      source AND tests. A file over its cap is doing too much or explaining
#      too much; split it along the layer seams. `*.gen.ts` is exempt: a
#      generated schema is machine output, not prose to split — the same way
#      dist/pkg/node_modules are not walked at all.
#
# Run from the repo root (CI runs it first — it needs no toolchain, only
# find/grep/sed/awk/wc). Every check accumulates into `fail` and the exit comes
# at the end, so one run shows the whole cleanup, not the first file of it.
#
# Usage: check-conventions.sh
set -euo pipefail

fail=0

# ── The purity scan ──────────────────────────────────────────────────────────
# Two patterns, because matching `use` alone leaves a hole big enough to drive a derive
# through: `#[derive(sqlx::FromRow)]` and a bare `web_sys::window()` name their crate without
# importing it, and both used to pass. Comments are stripped first — these layers explain
# themselves constantly, and a doc comment naming the adapter that owns sqlx is prose, not a
# dependency. The crate alternation matches family prefixes (`gloo` covers `gloo_net::`).
scan_purity() {
  local label="$1" crates="$2"
  shift 2
  local dirty=0 file found
  while IFS= read -r -d '' file; do
    found=$(sed 's://.*$::' "$file" |
      grep -nE "(^[[:space:]]*use[[:space:]]+(${crates})|(^|[^[:alnum:]_])(${crates})[[:alnum:]_]*::)" || true)
    if [[ -n "$found" ]]; then
      if ((dirty == 0)); then echo "✗ ${label}:"; fi
      dirty=1
      sed "s|^|    ${file}:|" <<<"$found"
    fi
  done < <("$@" -print0 2>/dev/null)
  return $dirty
}

# ── 1 · Server domain purity ─────────────────────────────────────────────────
echo "→ server domain purity (no axum/tower/hyper/tokio/sqlx/reqwest/utoipa under domain/)"
if [[ -d server/src ]]; then
  if scan_purity "domain files using infrastructure" \
    "axum|tower|hyper|tokio|sqlx|reqwest|utoipa" \
    find server/src -path "*/domain/*" -name "*.rs"; then
    echo "  ok"
  else
    fail=1
  fi
fi

# ── 2 · Viz engine purity ────────────────────────────────────────────────────
# The whole engine is pure by design (contract, vocabulary, geometry, adapt) and the purity
# is structural: a web-layer reference under engine/ fails the gate, so it cannot erode quietly.
echo "→ viz engine purity (no leptos/web-sys/wasm-bindgen/js-sys/gloo under viz-wasm/src/engine/)"
if [[ -d viz-wasm/src/engine ]]; then
  if scan_purity "engine files using the web layer" \
    "leptos|web_sys|wasm_bindgen|js_sys|gloo" \
    find viz-wasm/src/engine -name "*.rs"; then
    echo "  ok"
  else
    fail=1
  fi
fi

# ── 3 · File-size caps ───────────────────────────────────────────────────────
check_caps() {
  local cap="$1"
  shift
  local over=0
  while IFS= read -r line; do
    local n f
    n=$(awk '{print $1}' <<<"$line")
    f=$(awk '{$1=""; sub(/^ /,""); print}' <<<"$line")
    if ((n > cap)); then
      echo "    $f — $n/$cap"
      over=1
    fi
  done < <("$@" -print0 2>/dev/null | xargs -0 wc -l 2>/dev/null | grep -v " total$" || true)
  return $over
}

# ── 4 · The dev-only content feature must never reach the image ───────────────
# `render-local-only` renders study material that must not be published (ADR-RS002). It is a
# cargo feature so the production binary cannot contain the branch — which only holds while the
# image build declines to enable it.
echo "→ render-local-only stays out of the production image"
if grep -nE -- "--features[^\"]*render-local-only" Dockerfile dev-tools/start.sh 2>/dev/null; then
  echo "  ✗ the image build enables render-local-only — that publishes local-only-content/"
  fail=1
else
  echo "  ok"
fi

echo "→ file-size caps (server/shared ≤ 500 · viz-wasm/web ≤ 800 · *.gen.ts exempt)"
server_ok=0
check_caps 500 find server shared -name "*.rs" -not -path "*/target/*" || server_ok=1
client_ok=0
check_caps 800 find viz-wasm \( -name "*.rs" -o -name "*.ts" \) \
  -not -path "*/node_modules/*" -not -path "*/target/*" -not -path "*/dist/*" \
  -not -path "*/pkg/*" -not -name "*.gen.ts" || client_ok=1
web_ok=0
if [[ -d web ]]; then
  check_caps 800 find web \( -name "*.ts" -o -name "*.tsx" -o -name "*.astro" \) \
    -not -path "*/node_modules/*" -not -path "*/dist/*" -not -path "*/.astro/*" \
    -not -name "*.gen.ts" || web_ok=1
fi
if ((server_ok == 0 && client_ok == 0 && web_ok == 0)); then
  echo "  ok"
else
  echo "✗ files over their cap (listed above) — split along the layer seams"
  fail=1
fi

exit $fail
