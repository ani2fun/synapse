/**
 * `/admin` → Content repositories: which repositories feed the library, and what the merge had to
 * resolve between them.
 *
 * Not an `AllowlistSection` with different copy. A grant is a name and a verb; a source is a
 * placement plus a sync state that changes on its own — the row has to say what the fetch loop
 * last did, because "the book is not there" and "the token cannot see the repository" look
 * identical from the outside and only the row can tell them apart.
 *
 * The warnings below the table are the reason this screen is worth having during a migration:
 * while a book exists in the spine AND in its own repository, `duplicateBookSlug` names the copy
 * that is actually serving. That decides whether it is safe to delete the other one.
 *
 * Classes are the shared `admin__*` / `account-page__*` (web/styles/account.css).
 */
import { useEffect, useState } from "preact/hooks";

import { ApiFailure } from "../../lib/api/client";
import type { CatalogWarning, ContentSource } from "../../lib/api/client";
import {
  contentSourceRegister,
  contentSourceRemove,
  contentSources,
  contentSourcesSync,
  contentWarnings,
} from "../../lib/api/client";
import * as log from "../../lib/log";
import { ContentReadersSection } from "./ContentReadersSection";

type ActionStatus =
  | { kind: "idle" }
  | { kind: "busy"; message: string }
  | { kind: "ok"; message: string }
  | { kind: "error"; message: string };

type Rows =
  | { kind: "loading" }
  | { kind: "loaded"; rows: ContentSource[] }
  | { kind: "failed"; message: string };

function failureMessage(error: unknown): string {
  return error instanceof ApiFailure ? error.message : error instanceof Error ? error.message : String(error);
}

/**
 * Accept what a maintainer actually has in the clipboard — the repository's page URL — as well as
 * the `owner/name` the API wants. Everything else is passed through untouched so the server's
 * validation stays the single authority on what a repository is; this only removes the parts of a
 * GitHub URL that are unambiguously not the name.
 */
export function normaliseRepo(input: string): string {
  const trimmed = input.trim().replace(/\s+/g, "");
  if (trimmed === "") return "";
  const withoutScheme = trimmed.replace(/^(https?:\/\/)?(www\.)?github\.com\//i, "");
  const wasGitHubUrl = withoutScheme !== trimmed;
  const withoutSuffix = withoutScheme.replace(/\.git$/i, "").replace(/\/+$/, "");
  // Truncating to two segments is only safe for something we KNOW was a GitHub URL — a pasted
  // deep link (…/tree/main/chapter) still names the repository first. Doing it to anything else
  // would turn a wrong paste into a well-formed owner/name the server happily stores and then
  // cannot fetch; left whole, it is refused at the door with the rule quoted back.
  if (!wasGitHubUrl) return withoutSuffix;
  const segments = withoutSuffix.split("/").filter((s) => s !== "");
  return segments.length >= 2 ? `${segments[0]}/${segments[1]}` : withoutSuffix;
}

/** `2026-07-28T09:14:02Z` → `2026-07-28 09:14`, because the seconds are noise on a 60s loop. */
function whenSynced(iso: string | null | undefined): string {
  if (!iso) return "never";
  const [date, time] = iso.split("T");
  return date && time ? `${date} ${time.slice(0, 5)}` : iso;
}

export function ContentSourcesSection() {
  const [status, setStatus] = useState<ActionStatus>({ kind: "idle" });
  const [rows, setRows] = useState<Rows>({ kind: "loading" });
  const [warnings, setWarnings] = useState<CatalogWarning[]>([]);
  const [repo, setRepo] = useState("");
  const [grouping, setGrouping] = useState("");
  const [order, setOrder] = useState("");
  const [visibility, setVisibility] = useState<"public" | "private">("public");
  /** The source whose reader list is open below the table, by id. */
  const [readersOf, setReadersOf] = useState<string | null>(null);

  const reload = () => {
    void (async () => {
      try {
        const sources = await contentSources();
        setRows({ kind: "loaded", rows: sources });
        log.debug(`content sources: loaded ${sources.length}`);
      } catch (error) {
        setRows({ kind: "failed", message: failureMessage(error) });
      }
      try {
        setWarnings(await contentWarnings());
      } catch (error) {
        // A warnings failure must not blank the table — the table is the part you act on.
        log.debug(`content warnings unavailable: ${failureMessage(error)}`);
      }
    })();
  };

  useEffect(() => {
    log.info("content repositories section mounted");
    reload();
  }, []);

  const register = () => {
    const normalised = normaliseRepo(repo);
    if (normalised === "") {
      setStatus({ kind: "error", message: "A repository is owner/name, or its GitHub URL" });
      return;
    }
    // The row is the ONLY thing that positions a registered book — its `book.json` order is not
    // read. Leaving this blank therefore does not inherit anything, it means "unpositioned", and
    // the book lands at the end of its level. Asked for rather than defaulted, because a silent
    // default is exactly how a library reorders itself without anyone deciding to.
    const parsed = Number.parseInt(order, 10);
    if (Number.isNaN(parsed)) {
      setStatus({
        kind: "error",
        message: "An order is required — it is what positions this book; book.json's is not read.",
      });
      return;
    }
    setStatus({ kind: "busy", message: `Registering ${normalised}…` });
    void (async () => {
      try {
        const stored = await contentSourceRegister({
          repo: normalised,
          grouping: grouping.trim() === "" ? null : grouping.trim(),
          order: parsed,
          visibility,
        });
        setStatus({
          kind: "ok",
          message:
            visibility === "private"
              ? `Registered ${stored.repo} as private — nobody can read it until you add a reader.`
              : `Registered ${stored.repo}. It appears once the next fetch lands it.`,
        });
        setRepo("");
        setGrouping("");
        setOrder("");
        setVisibility("public");
        if (visibility === "private") setReadersOf(stored.id);
        reload();
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };

  /**
   * Enable/disable and public/private are re-registrations: the row is an upsert keyed on the
   * derived id, and EVERY field travels — a re-registration that forgot `visibility` would quietly
   * publish a private book, which is the one mistake this screen must make impossible.
   */
  const reregister = (
    row: ContentSource,
    change: Partial<Pick<ContentSource, "enabled" | "visibility">>,
    verb: string,
  ) => {
    setStatus({ kind: "busy", message: `${verb} ${row.repo}…` });
    void (async () => {
      try {
        await contentSourceRegister({
          repo: row.repo,
          branch: row.branch,
          grouping: row.grouping === "" ? null : row.grouping,
          order: row.order ?? null,
          enabled: change.enabled ?? row.enabled,
          visibility: change.visibility ?? row.visibility,
        });
        setStatus({ kind: "ok", message: `${row.repo}: done.` });
        reload();
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };
  const setEnabled = (row: ContentSource, enabled: boolean) =>
    reregister(row, { enabled }, enabled ? "Enabling" : "Disabling");
  const setVisibilityOf = (row: ContentSource, next: "public" | "private") =>
    reregister(row, { visibility: next }, next === "private" ? "Making private" : "Making public");

  const remove = (row: ContentSource) => {
    setStatus({ kind: "busy", message: `Removing ${row.repo}…` });
    void (async () => {
      try {
        await contentSourceRemove(row.id);
        setStatus({ kind: "ok", message: `Removed ${row.repo}.` });
        reload();
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };

  const syncNow = () => {
    setStatus({ kind: "busy", message: "Reconciling…" });
    void (async () => {
      try {
        await contentSourcesSync();
        setStatus({ kind: "ok", message: "Reconcile requested — refresh in a moment." });
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };

  return (
    <section class="admin__section">
      <h2 class="admin__section-title">Content repositories</h2>
      <p class="account-page__meta">
        Books served from their own repositories, fetched on a 60-second loop. The row is the only
        thing that decides placement — which grouping, in what order. An <code>order</code> left in
        the repository's <code>book.json</code> is not read at all; <code>book.json</code> decides
        the slug, because the slug is the URL. The primary checkout is not listed: it arrives by
        git-sync, is always mounted and is always first.
      </p>
      <StatusBanner status={status} />
      <form
        class="admin__grant"
        onSubmit={(event) => {
          event.preventDefault();
          register();
        }}
      >
        <input
          class="admin__input"
          placeholder="ani2fun/java-guide or its GitHub URL"
          value={repo}
          onInput={(event) => setRepo((event.target as HTMLInputElement).value)}
        />
        <input
          class="admin__input admin__input--note"
          placeholder="grouping (blank = top level)"
          value={grouping}
          onInput={(event) => setGrouping((event.target as HTMLInputElement).value)}
        />
        <input
          class="admin__input admin__input--order"
          placeholder="order (required)"
          inputMode="numeric"
          value={order}
          onInput={(event) => setOrder((event.target as HTMLInputElement).value)}
        />
        <select
          class="admin__input admin__input--order"
          aria-label="visibility"
          value={visibility}
          onChange={(event) =>
            setVisibility((event.target as HTMLSelectElement).value === "private" ? "private" : "public")
          }
        >
          <option value="public">public</option>
          <option value="private">private</option>
        </select>
        <button class="admin__grant-btn" type="submit">
          Register
        </button>
        <button class="admin__revoke" type="button" onClick={syncNow}>
          Sync now
        </button>
      </form>
      <SourcesTable
        rows={rows}
        setEnabled={setEnabled}
        setVisibility={setVisibilityOf}
        remove={remove}
        readersOf={readersOf}
        toggleReaders={(id) => setReadersOf((open) => (open === id ? null : id))}
      />
      {readersOf !== null && rows.kind === "loaded" && (
        <ContentReadersSection
          source={rows.rows.find((row) => row.id === readersOf) ?? null}
          close={() => setReadersOf(null)}
        />
      )}
      <WarningsList warnings={warnings} />
    </section>
  );
}

function SourcesTable({
  rows,
  setEnabled,
  setVisibility,
  remove,
  readersOf,
  toggleReaders,
}: {
  rows: Rows;
  setEnabled: (row: ContentSource, enabled: boolean) => void;
  setVisibility: (row: ContentSource, next: "public" | "private") => void;
  remove: (row: ContentSource) => void;
  readersOf: string | null;
  toggleReaders: (id: string) => void;
}) {
  if (rows.kind === "loading") return <p class="account-page__loading">Loading repositories…</p>;
  if (rows.kind === "failed")
    return <p class="account-page__status account-page__status--error">{rows.message}</p>;
  if (rows.rows.length === 0)
    return <p class="account-page__meta">No repositories registered — the spine is the whole library.</p>;
  return (
    <table class="admin__table">
      <thead>
        <tr>
          <th>Repository</th>
          <th>Placement</th>
          <th>Last sync</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {rows.rows.map((row) => (
          <tr key={row.id}>
            <td class="admin__cell-user">
              {row.visibility === "private" ? "🔒 " : ""}
              {row.repo}
              {row.branch === "main" ? "" : `#${row.branch}`}
              {row.enabled ? "" : " (disabled)"}
            </td>
            <td>
              {row.grouping === "" ? "top level" : row.grouping}
              {row.order === null || row.order === undefined ? "" : ` · ${row.order}`}
            </td>
            <td>
              {row.lastError ? (
                <span class="account-page__status--error">{row.lastError}</span>
              ) : (
                `${row.lastSha ? row.lastSha.slice(0, 7) : "—"} · ${whenSynced(row.lastSyncedAt)}`
              )}
            </td>
            <td>
              {/* A 2×2 grid, one verb per cell: enable · visibility / readers · remove. A public
                  row has no reader list to manage, so its cell stays empty rather than the
                  buttons reflowing. */}
              <div class="admin__actions">
                <button class="admin__revoke" onClick={() => setEnabled(row, !row.enabled)}>
                  {row.enabled ? "Disable" : "Enable"}
                </button>
                <button
                  class="admin__revoke"
                  onClick={() => setVisibility(row, row.visibility === "private" ? "public" : "private")}
                >
                  {row.visibility === "private" ? "Make public" : "Make private"}
                </button>
                {row.visibility === "private" ? (
                  <button class="admin__revoke" aria-expanded={readersOf === row.id} onClick={() => toggleReaders(row.id)}>
                    Readers
                  </button>
                ) : (
                  <span aria-hidden="true" />
                )}
                <button class="admin__revoke" onClick={() => remove(row)}>
                  Remove
                </button>
              </div>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/**
 * Empty is the normal state and says so, because "no warnings" and "warnings not loaded" must not
 * look the same on the screen you consult before deleting a book from the spine.
 */
function WarningsList({ warnings }: { warnings: CatalogWarning[] }) {
  if (warnings.length === 0)
    return <p class="account-page__meta">No cross-repository conflicts.</p>;
  return (
    <ul class="admin__warnings">
      {warnings.map((warning) => (
        <li key={`${warning.kind}:${warning.slug ?? ""}:${warning.sources.join(",")}`}>
          <span class="account-page__status-icon">!</span> {warning.detail}
        </li>
      ))}
    </ul>
  );
}

function StatusBanner({ status }: { status: ActionStatus }) {
  if (status.kind === "idle") return null;
  const cls =
    status.kind === "busy"
      ? "account-page__status account-page__status--busy"
      : status.kind === "ok"
        ? "account-page__status account-page__status--ok"
        : "account-page__status account-page__status--error";
  const icon = status.kind === "busy" ? "…" : status.kind === "ok" ? "✓" : "✗";
  return (
    <p class={cls}>
      <span class="account-page__status-icon">{icon}</span> {status.message}
    </p>
  );
}
