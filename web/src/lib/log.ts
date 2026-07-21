/**
 * The friendly logger (ADR-S009's client half), made ISOMORPHIC: the same call sites run in the
 * browser (islands) and in the Node sidecar (SSR frontmatter).
 *
 * Browser: a colored SYNAPSE badge + per-level emoji, each level mapped to the MATCHING
 * console method so DevTools filtering works; `debug` suppressed off-localhost so production
 * stays quiet.
 *
 * Node (SSR): plain `[SYNAPSE] emoji msg` lines on stdout/stderr — the sidecar's log becomes
 * followable the same way the axum side's tracing is; `debug` gated to dev builds.
 *
 * The point: a dev session is FOLLOWABLE FROM THE LOGS — boot → route → SSR fetch → hydrate →
 * run → result — INFO as the follow-along level, DEBUG filling in internals. Every island logs
 * its lifecycle; every store logs its transitions; the API client logs one line per wire call.
 */

const RESET = "color:inherit";
const badge = (color: string) =>
  `background:${color};color:#fff;border-radius:3px;padding:1px 5px;font-weight:bold`;

const inBrowser = typeof window !== "undefined";

function browserDev(): boolean {
  return inBrowser && window.location.hostname === "localhost";
}

function nodeDev(): boolean {
  return typeof process !== "undefined" && process.env.NODE_ENV !== "production";
}

type Method = "info" | "warn" | "error" | "debug";

function emit(method: Method, color: string, emoji: string, msg: string): void {
  if (inBrowser) {
    // eslint-disable-next-line no-console
    console[method](`%cSYNAPSE%c ${emoji} ${msg}`, badge(color), RESET);
  } else {
    // eslint-disable-next-line no-console
    console[method](`[SYNAPSE] ${emoji} ${msg}`);
  }
}

/** Lifecycle & notable events — the default follow-along level. */
export function info(msg: string): void {
  emit("info", "#2563eb", "ℹ️", msg);
}

/** Degraded but recovered (fallback, retry, stayed anonymous). */
export function warn(msg: string): void {
  emit("warn", "#d97706", "⚠️", msg);
}

/** A real failure needing attention. */
export function error(msg: string): void {
  emit("error", "#dc2626", "❌", msg);
}

/** Detailed internal steps — localhost / dev-build only, so production stays quiet. */
export function debug(msg: string): void {
  if (browserDev() || (!inBrowser && nodeDev())) {
    emit("debug", "#6b7280", "🔍", msg);
  }
}
