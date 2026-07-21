// The typed API client (oracle chain: Rust `client/src/api/mod.rs`, itself modelled on
// `ApiClient.scala`) — fetches decoding the wire DTOs the server publishes as OpenAPI, generated
// into `./schema.gen.ts` (never hand-typed — see the A02 chapter for the incident that argues
// for this). Errors surface as an `ApiFailure`; its `.message` matches the Rust client's
// `decode` format exactly (`error: detail`, `error` alone with no detail, `HTTP n` when the body
// was not the envelope at all), so error copy shown to a reader is unchanged mid-migration.

import * as log from "../log";
import type { components } from "./schema.gen";

// ── wire type aliases ───────────────────────────────────────────────────────────────────────
// The `Dto` suffix is the wire struct's Rust name, not vocabulary the rest of `web/` should
// have to spell — these are what every other module imports.

export type ApiErrorBody = components["schemas"]["ApiError"];
export type SynapseIndex = components["schemas"]["SynapseIndexDto"];
export type CatalogEntry = components["schemas"]["CatalogEntryDto"];
export type LessonPayload = components["schemas"]["LessonPayloadDto"];
export type ComponentDoc = components["schemas"]["ComponentDocDto"];
export type RunRequest = components["schemas"]["RunRequest"];
export type RunResult = components["schemas"]["RunResult"];
export type SubmitRequest = components["schemas"]["SubmitRequestDto"];
export type SubmissionAccepted = components["schemas"]["SubmissionAcceptedDto"];
export type Submission = components["schemas"]["SubmissionDto"];
export type DeleteResult = components["schemas"]["DeleteResultDto"];
export type Me = components["schemas"]["MeDto"];
export type AuthConfig = components["schemas"]["AuthConfigDto"];
export type BlogSummary = components["schemas"]["BlogSummaryDto"];
export type BlogPost = components["schemas"]["BlogPostDto"];
export type AllowlistEntry = components["schemas"]["AllowlistEntryDto"];
export type GrantRequest = components["schemas"]["GrantRequestDto"];
export type TutorConfig = components["schemas"]["TutorConfigDto"];
export type ChatMessage = components["schemas"]["ChatMessage"];
export type TutorChatRequest = components["schemas"]["TutorChatRequestDto"];
export type TutorChatResponse = components["schemas"]["TutorChatResponseDto"];

/**
 * `DELETE /api/me`'s 200 body has no OpenAPI schema — `identity::http::delete_me` answers
 * `Json(serde_json::json!({ "deleted": true }))` ad hoc (see server/src/identity/http/mod.rs).
 * There is nothing to generate this from, so it is spelled by hand, once, here.
 */
export interface DeleteMeResult {
  deleted: boolean;
}

// ── the origin seam ─────────────────────────────────────────────────────────────────────────

/**
 * In the browser every fetch is same-origin, so the base is `""` — identical to the old
 * client's relative paths. SSR has no browser origin to be relative to: that fetch is a real
 * network hop to the axum process, and `SYNAPSE_API_URL` names where. `import.meta.env` first
 * (Astro/Vite's own env loading, what every other server-side module in `web/` reads);
 * `process.env` as a plain-Node fallback for code paths that run outside Vite's pipeline.
 */

/** One line per wire call (the Rust chokepoint's localhost-only flow trace). */
async function loggedFetch(input: string, init?: RequestInit): Promise<Response> {
  log.debug(`${init?.method ?? "GET"} ${input}`);
  return fetch(input, init);
}

export function apiBase(): string {
  if (typeof window !== "undefined") return "";
  return import.meta.env.SYNAPSE_API_URL ?? process.env.SYNAPSE_API_URL ?? "http://localhost:8280";
}

// ── the bearer seam (oracle: the Rust client's `set_token_provider` thread_local) ─────────────

let tokenProvider: () => string | null = () => null;

/**
 * Identity installs this once, at startup; every call below reads it through `bearerHeaders`.
 * The default stays anonymous — this module never imports identity, the same direction-of-
 * dependency the Rust client's thread_local enforces (`api` stays feature-agnostic).
 */
export function installTokenProvider(provider: () => string | null): void {
  tokenProvider = provider;
}

/** Every call's headers run through this — an absent token means an anonymous request, not an
 *  error; the server decides what anonymous is allowed to do. */
export function bearerHeaders(): Record<string, string> {
  const token = tokenProvider();
  return token ? { Authorization: `Bearer ${token}` } : {};
}

// ── the error shape ─────────────────────────────────────────────────────────────────────────

/**
 * Thrown by every call below on a non-2xx response. `status` and `envelope` are there for
 * callers that want to branch (a 404 reads differently from a 503); `.message` is already the
 * human string a reader can be shown, formatted exactly like the Rust client's `decode`:
 * `error: detail` when the server sent a detail, `error` alone otherwise, `HTTP n` when the body
 * was not the envelope shape at all (a proxy 502, a body an intermediary stripped).
 */
export class ApiFailure extends Error {
  readonly status: number;
  readonly envelope: ApiErrorBody | null;

  constructor(status: number, envelope: ApiErrorBody | null) {
    super(ApiFailure.format(status, envelope));
    this.name = "ApiFailure";
    this.status = status;
    this.envelope = envelope;
  }

  private static format(status: number, envelope: ApiErrorBody | null): string {
    if (!envelope) return `HTTP ${status}`;
    return envelope.detail ? `${envelope.error}: ${envelope.detail}` : envelope.error;
  }
}

// ── the transport chokepoint ────────────────────────────────────────────────────────────────

async function errorBody(response: Response): Promise<ApiErrorBody | null> {
  try {
    return (await response.json()) as ApiErrorBody;
  } catch {
    return null;
  }
}

/**
 * The one place a `Response` becomes either a decoded value or a thrown `ApiFailure` — every
 * call below funnels through it, so the error-message format lives in exactly one place. A 204
 * (only `allowlistRevoke` today) has no body to parse; callers instantiate `T = void` there, and
 * `undefined` is a legal `void`, so this stays the single decode path rather than the Rust
 * client's bespoke `allowlist_revoke` (hand-rolled there because Rust has no `void` to unify
 * around — TypeScript's does the job instead).
 */
async function decode<T>(response: Response): Promise<T> {
  if (!response.ok) {
    throw new ApiFailure(response.status, await errorBody(response));
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

async function get<T>(path: string): Promise<T> {
  const response = await loggedFetch(`${apiBase()}${path}`, { headers: bearerHeaders() });
  return decode<T>(response);
}

async function post<T>(path: string, body: unknown): Promise<T> {
  const response = await loggedFetch(`${apiBase()}${path}`, {
    method: "POST",
    headers: { ...bearerHeaders(), "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  return decode<T>(response);
}

async function del<T>(path: string): Promise<T> {
  const response = await loggedFetch(`${apiBase()}${path}`, { method: "DELETE", headers: bearerHeaders() });
  return decode<T>(response);
}

// ── the 18 endpoints (oracle: client/src/api/mod.rs, one function per Rust fn) ────────────────

/** The browsable library index. */
export function fetchIndex(): Promise<SynapseIndex> {
  return get<SynapseIndex>("/api/synapse/index");
}

/** A lesson by its full directory-mirror path. */
export function lesson(path: string[]): Promise<LessonPayload> {
  return get<LessonPayload>(`/api/synapse/${path.join("/")}`);
}

/** A LikeC4 component's tutorial doc, co-located next to the given lesson. */
export function c4Doc(elementId: string, lesson: string[]): Promise<ComponentDoc> {
  return get<ComponentDoc>(`/api/synapse/c4-doc/${elementId}?lesson=${lesson.join("/")}`);
}

/** Run one snippet in the sandbox — a badly-running program is still a resolved `RunResult`. */
export function run(request: RunRequest): Promise<RunResult> {
  return post<RunResult>("/api/run", request);
}

/** Submit a solution — the 202 hands back the id the poll loop watches. */
export function submit(request: SubmitRequest): Promise<SubmissionAccepted> {
  return post<SubmissionAccepted>("/api/submissions", request);
}

/** The caller's OWN submissions for a lesson, newest first (anonymous → `[]`). */
export function submissionsFor(path: string[]): Promise<Submission[]> {
  return get<Submission[]>(`/api/submissions?path=${path.join("/")}`);
}

/** One poll tick. */
export function submission(id: string): Promise<Submission> {
  return get<Submission>(`/api/submissions/${id}`);
}

/** The blog listing, newest first. */
export function blogList(): Promise<BlogSummary[]> {
  return get<BlogSummary[]>("/api/blog");
}

/** One post with body + neighbours. */
export function blogPost(slug: string): Promise<BlogPost> {
  return get<BlogPost>(`/api/blog/${slug}`);
}

/** The SPA's Keycloak coordinates. */
export function authConfig(): Promise<AuthConfig> {
  return get<AuthConfig>("/api/auth/config");
}

/** The verified caller — the bearer seam supplies the token. */
export function me(): Promise<Me> {
  return get<Me>("/api/me");
}

/** Erase every submission of the caller ("reset my data"). */
export function eraseSubmissions(): Promise<DeleteResult> {
  return del<DeleteResult>("/api/submissions");
}

/** Remove the caller's sign-in (the Keycloak account). App data is the separate verb above —
 *  the account page orchestrates erase → delete. */
export function deleteMe(): Promise<DeleteMeResult> {
  return del<DeleteMeResult>("/api/me");
}

/** The coach's coordinates — always answers; `enabled: false` is the off state. */
export function tutorConfig(): Promise<TutorConfig> {
  return get<TutorConfig>("/api/tutor/config");
}

/** One coaching turn — the full transcript up, one reply back (a 404 means the coach is off). */
export function tutorChat(request: TutorChatRequest): Promise<TutorChatResponse> {
  return post<TutorChatResponse>("/api/tutor/chat", request);
}

/** The admin allowlist (the server re-checks admin per call — these 401/403 for everyone else). */
export function allowlist(): Promise<AllowlistEntry[]> {
  return get<AllowlistEntry[]>("/api/admin/allowlist");
}

/** Grant (upsert) — the stored row comes back. */
export function allowlistGrant(request: GrantRequest): Promise<AllowlistEntry> {
  return post<AllowlistEntry>("/api/admin/allowlist", request);
}

/** `undefined` on 204; a 404 surfaces as an `ApiFailure`. */
export function allowlistRevoke(username: string): Promise<void> {
  return del<void>(`/api/admin/allowlist/${username}`);
}
