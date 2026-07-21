#!/bin/bash
# ──────────────────────────────────────────────────────────────────────────────
# THE PRODUCTION IMAGE ENTRYPOINT — the axum server fronting the Astro SSR
# sidecar, two processes in one container.
#
# bash (not the image's /bin/sh, which is dash) is REQUIRED for `wait -n`: dash's
# builtin has no -n, and -n is the whole trick — it wakes on the FIRST child to
# die so either half going down takes the container with it.
# ──────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# ── API-ONLY MODE ─────────────────────────────────────────────────────────────
# SYNAPSE_ASTRO_URL empty/unset ⇒ one process, the API alone (pages answer with a
# plain-text pointer) — a degraded ops mode, not a serve anyone should run long.
# UNSET (not just empty) so the server's figment config sees it ABSENT — an empty
# string is `Some("")`, which enables the proxy pointed at nowhere and 502s every
# page. `exec` so the server is PID 1 and gets signals directly — no wrapper to reap.
if [[ -z "${SYNAPSE_ASTRO_URL:-}" ]]; then
  unset SYNAPSE_ASTRO_URL
  exec /app/synapse-server
fi

# ── ASTRO TOPOLOGY (the default) ──────────────────────────────────────────────
# The @astrojs/node SSR sidecar behind the axum server. The sidecar's own render
# fetches are a real network hop back to axum; SYNAPSE_API_URL names it. SYNAPSE_PORT
# is the single source of truth for the server's port — do not spell 8080 a second time.
export SYNAPSE_API_URL="http://127.0.0.1:${SYNAPSE_PORT}"

# The sidecar listens where SYNAPSE_ASTRO_URL points (:4321 by the image's ENV
# default); the two MUST agree or the proxy talks to no one.
HOST=127.0.0.1 PORT=4321 node /app/web/server/entry.mjs &
SIDECAR_PID=$!
/app/synapse-server &
SERVER_PID=$!

# Half-alive is the worst state: one process gone and every page is a 502 forever,
# while the orchestrator only restarts a container that EXITS. So EITHER death ends
# the whole container (`wait -n`), and the trap reaps whichever half is still up.
# The TERM/INT trap turns an orchestrator stop into a clean exit so the EXIT trap runs.
trap 'kill "$SIDECAR_PID" "$SERVER_PID" 2>/dev/null || true' EXIT
trap 'exit 0' TERM INT
wait -n
