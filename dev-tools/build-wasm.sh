#!/usr/bin/env bash
# ── WASM BUILD (dev | release) ────────────────────────────────────────────────
# The client's wasm pipeline, spelled out: cargo build → wasm-bindgen → (release)
# wasm-opt. This replaced wasm-pack in step 08 — wasm-pack wraps exactly these
# three tools but kept dying on its own tool-discovery ("invalid type: map,
# expected a string"), and a build we can read is a build we can trust.
#
# Needs: the wasm32-unknown-unknown target (rust-toolchain.toml pins it) and
# wasm-bindgen-cli matching Cargo.lock's wasm-bindgen (0.2.126):
#   cargo install wasm-bindgen-cli --version 0.2.126
# Release also wants wasm-opt (brew install binaryen / apt install binaryen);
# it is skipped with a warning when absent — the bundle-budget gate still
# measures whatever comes out.
#
# Usage: build-wasm.sh [dev|release]   (default: dev; output: client/pkg)
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
  # Size-first profile (opt-level z · fat LTO · one codegen unit · panic=abort) — see
  # [profile.wasm-release] in Cargo.toml. The plain `release` profile stays the server's.
  # `--cfg erase_components` switches Leptos to TYPE-ERASED views: the statically-typed
  # view tree's monomorphization was ~15% of the whole gzipped wasm (686→584 KiB gz,
  # measured at step 39) — dynamic dispatch is invisible next to DOM cost; the bytes are not.
  RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--cfg erase_components" \
    cargo build -p synapse-client --target wasm32-unknown-unknown --profile wasm-release
  wasm_file="target/wasm32-unknown-unknown/wasm-release/synapse_client.wasm"
else
  cargo build -p synapse-client --target wasm32-unknown-unknown
  wasm_file="target/wasm32-unknown-unknown/debug/synapse_client.wasm"
fi

wasm-bindgen "$wasm_file" --target web --out-dir client/pkg --out-name synapse_client

if [[ "$profile" == "release" ]]; then
  if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz client/pkg/synapse_client_bg.wasm -o client/pkg/synapse_client_bg.wasm
    echo "→ wasm-opt -Oz applied"
  else
    echo "⚠ wasm-opt not found — shipping unoptimized wasm (brew install binaryen)"
  fi
fi
echo "→ client/pkg ready ($profile)"
