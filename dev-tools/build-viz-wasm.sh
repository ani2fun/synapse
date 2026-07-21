#!/usr/bin/env bash
# ── VIZ WASM BUILD (dev | release) ────────────────────────────────────────────
# The standalone `viz-wasm` crate's build pipeline: cargo build → wasm-bindgen → (release only)
# wasm-opt. The output lands in the Astro app at web/src/lib/viz-wasm/pkg, where the lazy
# `islands/viz.ts` loader dynamic-imports the glue from (gitignored — build output, rebuilt by
# CI/Docker).
#
# Needs: the wasm32-unknown-unknown target and a wasm-bindgen-cli version matching Cargo.lock
# (checked below). Release wants wasm-opt (binaryen).
#
# Usage: build-viz-wasm.sh [dev|release]   (default: dev)
set -euo pipefail
cd "$(dirname "$0")/.."

profile="${1:-dev}"
locked="$(grep -A1 'name = "wasm-bindgen"$' Cargo.lock | grep version | cut -d'"' -f2)"
have="$(wasm-bindgen --version | awk '{print $2}')"
if [[ "$locked" != "$have" ]]; then
  echo "✗ wasm-bindgen-cli $have ≠ Cargo.lock's $locked — run: cargo install wasm-bindgen-cli --version $locked"
  exit 1
fi

if [[ "$profile" == "release" ]]; then
  # Size-first codegen plus type-erased Leptos views: this bundle is all view code, so erasing
  # view-tree monomorphization is the single biggest lever on its size.
  RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg erase_components" \
    cargo build -p viz-wasm --target wasm32-unknown-unknown --profile wasm-release
  wasm_file="target/wasm32-unknown-unknown/wasm-release/viz_wasm.wasm"
else
  cargo build -p viz-wasm --target wasm32-unknown-unknown
  wasm_file="target/wasm32-unknown-unknown/debug/viz_wasm.wasm"
fi

out="web/src/lib/viz-wasm/pkg"
wasm-bindgen "$wasm_file" --target web --out-dir "$out" --out-name viz_wasm

if [[ "$profile" == "release" ]]; then
  if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz "$out/viz_wasm_bg.wasm" -o "$out/viz_wasm_bg.wasm"
    echo "→ wasm-opt -Oz applied"
  else
    echo "⚠ wasm-opt not found — shipping unoptimized wasm (brew install binaryen)"
  fi
fi
echo "→ $out ready ($profile)"
