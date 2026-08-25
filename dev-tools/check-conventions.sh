#!/usr/bin/env bash
# ── CONVENTION GATE (RS001 · purity · file caps · test placement) ────────────
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
#   4. RENDER-LOCAL-ONLY: the image build never enables the cargo feature that
#      publishes local-only study material (ADR-RS002).
#   5. TEST PLACEMENT: server & shared unit tests live in a sibling
#      `<module>_tests.rs` behind `#[cfg(test)] #[path]` — never inline, always
#      plural, always declared. coverage.sh tells test code from production BY
#      FILENAME and the caps count every line, so placement moves lines across
#      a measured boundary; every way of getting it wrong is otherwise green.
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


# ── 5 · Test placement ───────────────────────────────────────────────────────
# Unit tests live in a SIBLING `<module>_tests.rs` reached by `#[cfg(test)] #[path]`, never in an
# inline block. Two gates depend on that file name and neither can see inside a file:
# coverage.sh tells test code from production purely by `_tests\.rs$`, and the 500-line cap counts
# whatever the file holds. So a misplaced or misnamed test is not a style nit — it moves lines
# across a measured boundary, and every way of getting it wrong stays green without this check.
echo "→ test placement (sibling *_tests.rs behind #[cfg(test)]; server/src + shared/src)"
placement=0

# 5a · An inline `mod tests { … }` puts test lines in a production file, where coverage counts
# them as covered production code.
inline=""
while IFS= read -r -d '' file; do
  hit=$(awk '
    /^[[:space:]]*#\[cfg\(test\)\]/           { p = 1; next }
    p && /^[[:space:]]*#\[/                   { next }
    p && /^[[:space:]]*(pub )?mod [a-z_]+ \{/ { print FNR ": " $0; p = 0; next }
    p                                         { p = 0 }
  ' "$file")
  [[ -n "$hit" ]] && inline+=$(sed "s|^|    ${file}:|" <<<"$hit")$'\n'
done < <(find server/src shared/src -name "*.rs" -print0 2>/dev/null)
if [[ -n "$inline" ]]; then
  echo "  ✗ inline test blocks — these lines count as COVERED PRODUCTION code and eat the file cap:"
  printf '%s' "$inline"
  echo "    move each to a sibling *_tests.rs: #[cfg(test)] #[path = \"x_tests.rs\"] mod tests;"
  placement=1
fi

# 5b · The exclusion regex is `_tests\.rs$`. A singular `_test.rs` misses it, and the file is
# measured as production.
singular=$(find server/src shared/src -name "*_test.rs" 2>/dev/null || true)
if [[ -n "$singular" ]]; then
  echo "  ✗ singular *_test.rs — coverage.sh excludes *_tests.rs only, so these read as production:"
  sed 's|^|    |' <<<"$singular"
  placement=1
fi

# 5c · A `*_tests.rs` is test code only because something declares it one. A bare `mod x_tests;`
# compiles it unconditionally AND drops it from coverage; a file nothing declares is not compiled
# at all. Both are silent — the suite still passes, with fewer tests in it.
orphan=""; unguarded=""
while IFS= read -r -d '' t; do
  base=$(basename "$t"); stem=${base%.rs}; dir=$(dirname "$t")
  decl=$(grep -rn -E "#\[path = \"${base}\"\]|^[[:space:]]*mod ${stem};" "$dir" --include="*.rs" || true)
  if [[ -z "$decl" ]]; then orphan+="    $t"$'\n'; continue; fi
  while IFS= read -r line; do
    df=${line%%:*}; rest=${line#*:}; dn=${rest%%:*}
    start=$((dn > 2 ? dn - 2 : 1))
    sed -n "${start},$((dn - 1))p" "$df" | grep -q '#\[cfg(test)\]' || unguarded+="    ${df}:${dn}"$'\n'
  done <<<"$decl"
done < <(find server/src shared/src -name "*_tests.rs" -print0 2>/dev/null)
if [[ -n "$orphan" ]]; then
  echo "  ✗ *_tests.rs declared nowhere — not compiled, so its tests silently do not run:"
  printf '%s' "$orphan"
  placement=1
fi
if [[ -n "$unguarded" ]]; then
  echo "  ✗ test module declared without #[cfg(test)] above it — ships in the binary, and if the"
  echo "    file is production code its name still hides it from the coverage gate:"
  printf '%s' "$unguarded"
  placement=1
fi

if ((placement == 0)); then echo "  ok"; else fail=1; fi

exit $fail
