#!/usr/bin/env bash
# ── GENERATE WIRE TYPES ────────────────────────────────────────────────────────
# The server's OpenAPI document (utoipa, code-first — server/src/lib.rs's `ApiDoc`) is the one
# source of truth for what web/ decodes. This re-derives web/src/lib/api/schema.gen.ts from it:
# build dump_openapi, run it for the document, feed the document to openapi-typescript, write
# the result with a header saying how to regenerate.
#
# CI runs this exact script and diffs the result (the `web` job's "generated types are
# current" step) — a schema that drifted from a hand edit, or a server change nobody
# regenerated for, fails the build instead of waiting for a reviewer to notice.
#
# Run from the repo root (like every other dev-tools script) or anywhere — it cd's home first.
set -euo pipefail
cd "$(dirname "$0")/.."

OUT="web/src/lib/api/schema.gen.ts"
mkdir -p "$(dirname "$OUT")"

echo "→ building dump_openapi"
cargo build --quiet -p synapse-server --bin dump_openapi

echo "→ openapi-typescript (run from web/, so it shares web's npm/npx cache)"
{
  cat <<'HEADER'
// GENERATED FILE — do not edit by hand.
// Source: server/src/lib.rs's `ApiDoc` (utoipa) -> dump_openapi -> openapi-typescript.
// Regenerate: dev-tools/gen-api-types.sh
HEADER
  ./target/debug/dump_openapi | (cd web && npx --yes openapi-typescript@latest)
} >"$OUT"

echo "→ wrote $OUT ($(wc -l <"$OUT" | tr -d ' ') lines)"
