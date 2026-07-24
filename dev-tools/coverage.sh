#!/usr/bin/env bash
# ── COVERAGE ───────────────────────────────────────────────────────────────────
# Line/region coverage for the Rust workspace and the web island tests, in one place.
#
# The gated number is SERVER + SHARED production code — the two crates whose behaviour
# the tests actually pin. Deliberately excluded from the gate (the `IGNORE` regex):
#   · viz-wasm/*      — the engine is covered by `cargo test -p viz-wasm`, but its DOM /
#                       renderer layer is browser-only and cannot run natively, so it would
#                       drag the number down for a reason no test can fix.
#   · server/src/main.rs — the composition root; exercised by running the binary, not by tests.
#   · *_tests.rs, service_fakes.rs, tests/*, dump_openapi.rs — test code and the openapi dumper,
#                       which are not the production surface being measured.
#
# Usage:
#   dev-tools/coverage.sh            report + write lcov/HTML under target/coverage/
#   dev-tools/coverage.sh --check    the above, then FAIL if server+shared lines < FAIL_UNDER
#
# Set POSTGRES_IT=1 (with the dev database up on :5532) for the honest number — the Postgres
# adapters answer 503/skip without it, so their lines read as uncovered. CI runs it with a
# Postgres service. FAIL_UNDER overrides the floor (default 88).
set -euo pipefail
cd "$(dirname "$0")/.."

CHECK=0
[[ "${1:-}" == "--check" ]] && CHECK=1
FAIL_UNDER="${FAIL_UNDER:-88}"
OUT="target/coverage"
mkdir -p "$OUT"

# `_tests\.rs` / `service_fakes\.rs` / `/tests/` cover the test code; `/main\.rs` the wiring point.
IGNORE='(^|/)(viz-wasm)/|/main\.rs$|_tests\.rs$|/tests/|service_fakes\.rs$|dump_openapi\.rs$'

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  echo "✗ cargo-llvm-cov not installed — run: cargo install cargo-llvm-cov --locked"
  echo "  (and: rustup component add llvm-tools-preview)"
  exit 1
fi

echo "→ Rust coverage (server + shared; viz-wasm + main.rs + test code excluded from the gate)"
[[ -n "${POSTGRES_IT:-}" ]] || echo "  note: POSTGRES_IT unset — Postgres adapters will read as uncovered"

# One instrumented run produces every artifact: the machine-readable lcov, the browsable HTML,
# and the printed summary. `--no-clean` between them would re-use the profile; here each flag is
# a separate cheap re-report over the same run.
cargo llvm-cov --workspace --ignore-filename-regex "$IGNORE" --lcov --output-path "$OUT/rust.lcov" >/dev/null 2>&1
cargo llvm-cov report --ignore-filename-regex "$IGNORE" --html --output-dir "$OUT/rust-html" >/dev/null 2>&1
cargo llvm-cov report --ignore-filename-regex "$IGNORE" --summary-only 2>/dev/null | tail -1 | sed 's/^/  /'
echo "  lcov: $OUT/rust.lcov   html: $OUT/rust-html/index.html"

echo "→ web coverage (island logic — lint, diff, frontmatter, markdown, execution)"
if [[ -d web/node_modules ]]; then
  (cd web && npx vitest run --coverage --coverage.reportsDirectory="../$OUT/web-html" 2>&1 | tail -20 | sed 's/^/  /')
else
  echo "  skipped — web/node_modules absent (run: cd web && npm ci)"
fi

if ((CHECK)); then
  echo "→ gate: server + shared lines ≥ ${FAIL_UNDER}%"
  cargo llvm-cov report --ignore-filename-regex "$IGNORE" --fail-under-lines "$FAIL_UNDER" 2>/dev/null >/dev/null \
    && echo "  ok" \
    || { echo "  ✗ below ${FAIL_UNDER}% — add tests, or lower FAIL_UNDER with a reason"; exit 1; }
fi
