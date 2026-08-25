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
#   3. FILE-SIZE CAPS: server & shared ≤ 500 lines/file, viz-wasm & web ≤ 800.
#      A file over its cap is doing too much or explaining too much; split it
#      along the layer seams. Rust TEST code is exempt — a suite is a list, it
#      grows with the surface it covers, and splitting one to satisfy a budget
#      invents module boundaries that describe nothing. `*.gen.ts` is exempt
#      too: a generated schema is machine output, not prose to split — the same
#      way dist/pkg/node_modules are not walked at all.
#   4. RENDER-LOCAL-ONLY: the image build never enables the cargo feature that
#      publishes local-only study material (ADR-RS002).
#   5. TEST PLACEMENT: a module's unit tests live at its natural child path —
#      `<module>/tests.rs`, with anything they need under `<module>/tests/`.
#      No inline blocks, no flat `*_tests.rs` siblings, and no `#[path]`, which
#      exists only to defeat the resolution this convention relies on.
#      coverage.sh excludes test code by that PATH, so misplacing a test moves
#      its lines into the production number without failing anything else.
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

echo "→ file-size caps (server/shared ≤ 500 · viz-wasm/web ≤ 800 · tests + *.gen.ts exempt)"
server_ok=0
check_caps 500 find server shared -name "*.rs" -not -path "*/target/*" \
  -not -name "tests.rs" -not -path "*/tests/*" || server_ok=1
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
# A module's tests live at its natural child path: `foo.rs` (or `foo/mod.rs`) is tested by
# `foo/tests.rs`, and whatever that suite needs — fixtures, fakes, topical sub-suites — sits
# under `foo/tests/`. coverage.sh excludes exactly that shape, by PATH, so a misplaced test is
# counted as production code and silently lifts the number the 88% floor is measured against.
# Rust resolves this layout on its own; `#[path]` is what a flat layout needs, and it is banned
# here precisely because reaching for it is how the flat layout comes back.
echo "→ test placement (a module's tests at <module>/tests.rs; server/src + shared/src)"
placement=0

# 5a · An inline `mod tests { … }` leaves test lines inside a production file, where nothing
# can separate them from the code they test.
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
  echo "  ✗ inline test blocks — move each to <module>/tests.rs:"
  printf '%s' "$inline"
  placement=1
fi

# 5b · A flat sibling is the old layout. It needs `#[path]`, and its name is the only thing
# saying it is a test — which is what let a production adapter named `problem_tests.rs` out of
# the coverage gate entirely.
flat=$(find server/src shared/src \( -name "*_tests.rs" -o -name "*_test.rs" \) 2>/dev/null || true)
if [[ -n "$flat" ]]; then
  echo "  ✗ flat test siblings — these belong at <module>/tests.rs or under <module>/tests/:"
  sed 's|^|    |' <<<"$flat"
  placement=1
fi

# 5c · `#[path]` defeats the resolution the layout depends on, and one use pulls in the next:
# a `#[path]`-loaded file resolves ITS submodules beside the parent, so they need `#[path]` too.
paths=$(grep -rn '#\[path' --include="*.rs" server/src shared/src 2>/dev/null || true)
if [[ -n "$paths" ]]; then
  echo "  ✗ #[path] on a module declaration — the natural child path needs no attribute:"
  sed 's|^|    |' <<<"$paths"
  placement=1
fi

# 5d · A tests.rs its owner never declares is not compiled, and a suite that does not run looks
# exactly like a suite that passes.
undeclared=""
while IFS= read -r -d '' t; do
  d=$(dirname "$t")
  if [[ -f "$d/mod.rs" ]]; then owner="$d/mod.rs"; elif [[ -f "$d.rs" ]]; then owner="$d.rs"; else owner=""; fi
  if [[ -z "$owner" ]]; then
    undeclared+="    $t — no owning module (expected $d/mod.rs or $d.rs)"$'\n'
  elif ! grep -A1 '#\[cfg(test)\]' "$owner" | grep -q '^[[:space:]]*mod tests;'; then
    undeclared+="    $t — $owner does not declare it behind #[cfg(test)]"$'\n'
  fi
done < <(find server/src shared/src -name "tests.rs" -print0 2>/dev/null)
if [[ -n "$undeclared" ]]; then
  echo "  ✗ test roots that do not compile into the crate:"
  printf '%s' "$undeclared"
  placement=1
fi

# 5e · Same hazard one level down: a file under tests/ that its tests.rs never names.
orphan=""
while IFS= read -r -d '' c; do
  root="$(dirname "$c").rs"
  stem=$(basename "$c" .rs)
  [[ -f "$root" ]] || continue
  grep -qE "^[[:space:]]*mod ${stem};" "$root" || orphan+="    $c — not declared in $root"$'\n'
done < <(find server/src shared/src -path "*/tests/*" -name "*.rs" -print0 2>/dev/null)
if [[ -n "$orphan" ]]; then
  echo "  ✗ test support files nothing declares:"
  printf '%s' "$orphan"
  placement=1
fi

if ((placement == 0)); then echo "  ok"; else fail=1; fi

exit $fail
