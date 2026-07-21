# ── Port reclamation, shared by `dev` and `e2e` ────────────────────────────────
# Sourced, not executed. Both scripts bind FIXED ports by design — Vite is
# --strictPort because the Keycloak dev realm whitelists specific origins, and a
# silent bump 403s the silent-SSO iframe. So a port being held always means a
# leftover process, never a neighbour to negotiate with.
#
# Without this, a stale server left on :8280 makes the new bind die with
# "Address already in use (os error 48)" while Vite carries on and proxies to the
# OLD binary — a page that looks alive and serves pre-edit code.

# reclaim <port> <label>
reclaim() {
  local port="$1" label="$2" pids pid
  pids=$(lsof -ti "tcp:$port" -sTCP:LISTEN 2>/dev/null || true)
  [[ -z "$pids" ]] && return 0
  echo "→ :$port ($label) is held — reclaiming"
  while read -r pid; do
    [[ -z "$pid" ]] && continue
    # Name it: if this is ever something unexpected, that must be visible rather
    # than silently reaped.
    echo "    kill $pid  $(ps -p "$pid" -o comm= 2>/dev/null || echo '?')"
    kill "$pid" 2>/dev/null || true
  done <<<"$pids"
  for _ in $(seq 1 20); do
    lsof -ti "tcp:$port" -sTCP:LISTEN >/dev/null 2>&1 || return 0
    sleep 0.25
  done
  echo "    still held after SIGTERM — sending SIGKILL"
  lsof -ti "tcp:$port" -sTCP:LISTEN 2>/dev/null | xargs kill -9 2>/dev/null || true
}
