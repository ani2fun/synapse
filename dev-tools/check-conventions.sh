#!/usr/bin/env bash
# ── CONVENTION GATE (RS001 · hexagon purity · three-layer purity · file caps) ─
# The Rust edition of Synapse's gate — the conventions that must never be green
# by discipline alone:
#
#   1. SERVER DOMAIN PURITY: files under any server `domain/` layer use NO
#      axum / tower / hyper / tokio / sqlx / reqwest / utoipa — the domain is
#      pure Rust (std + serde at most), and such a `use` means a port was
#      skipped.
#   2. CLIENT LOGIC PURITY: files under any client `logic/` layer use NO
#      leptos / web-sys / wasm-bindgen / js-sys / gloo — pure logic stays
#      native-testable (the three-layer rule).
#   3. FILE-SIZE CAPS: server & shared ≤ 500 lines/file, client (+ TS islands)
#      ≤ 800 — source AND tests. A file over its cap is doing too much or
#      explaining too much; split it along the hexagonal / three-layer seams.
#
# Run from the repo root (CI runs it before the toolchain — it needs nothing
# but grep/find/wc). Exit 1 with every violation listed, so one run shows the
# whole cleanup, not the first file of it.
#
# Usage: check-conventions.sh
set -euo pipefail

fail=0

# ── 1 · Server domain purity ─────────────────────────────────────────────────
echo "→ server domain purity (no axum/tower/hyper/tokio/sqlx/reqwest/utoipa under domain/)"
if [[ -d server/src ]]; then
  impure=$(find server/src -path "*/domain/*" -name "*.rs" -print0 2>/dev/null |
    xargs -0 grep -l -E '^\s*use (axum|tower|hyper|tokio|sqlx|reqwest|utoipa)' 2>/dev/null || true)
  if [[ -n "$impure" ]]; then
    echo "✗ domain files using infrastructure:"
    echo "$impure" | while read -r f; do
      grep -n -E '^\s*use (axum|tower|hyper|tokio|sqlx|reqwest|utoipa)' "$f" | sed "s|^|    $f:|"
    done
    fail=1
  else
    echo "  ok"
  fi
fi

# ── 2 · Client logic purity ──────────────────────────────────────────────────
# viz/engine/ joined in step 59: the whole engine is pure by design (it moved out of
# synapse-shared for exactly that property) but only logic/ was ever gated — shapes.rs and
# decoder.rs sat at viz/ root, clean yet unprotected. They moved under engine/ and the gate
# now covers the folder, so the discipline is structural rather than observed.
echo "→ client logic purity (no leptos/web-sys/wasm-bindgen/js-sys/gloo under logic/ + viz/engine/)"
if [[ -d client/src ]]; then
  impure=$(find client/src \( -path "*/logic/*" -o -path "*/viz/engine/*" \) -name "*.rs" -print0 2>/dev/null |
    xargs -0 grep -l -E '^\s*use (leptos|web_sys|wasm_bindgen|js_sys|gloo)' 2>/dev/null || true)
  if [[ -n "$impure" ]]; then
    echo "✗ logic files using the web layer:"
    echo "$impure" | while read -r f; do
      grep -n -E '^\s*use (leptos|web_sys|wasm_bindgen|js_sys|gloo)' "$f" | sed "s|^|    $f:|"
    done
    fail=1
  else
    echo "  ok"
  fi
else
  echo "  (no client/ yet — arrives step 02)"
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

echo "→ file-size caps (server/shared ≤ 500 · client/web ≤ 800)"
server_ok=0
check_caps 500 find server shared -name "*.rs" -not -path "*/target/*" || server_ok=1
client_ok=0
if [[ -d client ]]; then
  check_caps 800 find client \( -name "*.rs" -o -name "*.ts" \) \
    -not -path "*/node_modules/*" -not -path "*/target/*" -not -path "*/dist/*" \
    -not -path "*/pkg/*" || client_ok=1
fi
web_ok=0
if [[ -d web ]]; then
  check_caps 800 find web \( -name "*.ts" -o -name "*.tsx" -o -name "*.astro" \) \
    -not -path "*/node_modules/*" -not -path "*/dist/*" -not -path "*/.astro/*" || web_ok=1
fi
if ((server_ok == 0 && client_ok == 0 && web_ok == 0)); then
  echo "  ok"
else
  echo "✗ files over their cap (listed above) — split along the layer seams"
  fail=1
fi

exit $fail
