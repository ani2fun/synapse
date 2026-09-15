/**
 * `/admin` → Content repositories → Readers: who may read ONE private repository.
 *
 * Opened per row rather than shown for every source at once, because the list is the whole
 * meaning of "private" — a maintainer granting a name should be looking at exactly the book they
 * are granting it for. The list is kept on a public source too (the API keeps it; flipping the
 * source private must not start from nothing), which is why this panel still opens there and
 * says so.
 *
 * Classes are the shared `admin__*` / `account-page__*` (web/styles/account.css), the same shape
 * as AllowlistSection because a grant is a name and a verb here as well.
 */
import { useEffect, useState } from "preact/hooks";
import { ApiFailure, contentReaderGrant, contentReaderRevoke, contentReaders } from "../../lib/api/client";
import type { ContentReader, ContentSource } from "../../lib/api/client";
import * as log from "../../lib/log";

type ActionStatus =
  | { kind: "idle" }
  | { kind: "busy"; message: string }
  | { kind: "ok"; message: string }
  | { kind: "error"; message: string };

type Rows = { kind: "loading" } | { kind: "loaded"; rows: ContentReader[] } | { kind: "failed"; message: string };

function failureMessage(error: unknown): string {
  return error instanceof ApiFailure ? error.message : error instanceof Error ? error.message : String(error);
}

function whenGranted(iso: string): string {
  const [date] = iso.split("T");
  return date ?? iso;
}

export function ContentReadersSection({ source, close }: { source: ContentSource | null; close: () => void }) {
  const [status, setStatus] = useState<ActionStatus>({ kind: "idle" });
  const [rows, setRows] = useState<Rows>({ kind: "loading" });
  const [username, setUsername] = useState("");
  const [note, setNote] = useState("");
  const id = source?.id ?? null;

  const reload = () => {
    if (id === null) return;
    void (async () => {
      try {
        const readers = await contentReaders(id);
        setRows({ kind: "loaded", rows: readers });
        log.debug(`content readers: ${id} has ${readers.length}`);
      } catch (error) {
        setRows({ kind: "failed", message: failureMessage(error) });
      }
    })();
  };

  useEffect(() => {
    log.info(`content readers section mounted for ${id ?? "(no source)"}`);
    setRows({ kind: "loading" });
    setStatus({ kind: "idle" });
    reload();
  }, [id]);

  if (source === null || id === null) return null;

  const grant = () => {
    const name = username.trim();
    if (name === "") {
      setStatus({ kind: "error", message: "A reader is a sign-in username" });
      return;
    }
    setStatus({ kind: "busy", message: `Granting ${name}…` });
    void (async () => {
      try {
        const stored = await contentReaderGrant(id, { username: name, note: note.trim() === "" ? null : note.trim() });
        setStatus({ kind: "ok", message: `${stored.username} may read ${source.repo} — live within a few seconds.` });
        setUsername("");
        setNote("");
        reload();
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };

  const revoke = (reader: ContentReader) => {
    setStatus({ kind: "busy", message: `Revoking ${reader.username}…` });
    void (async () => {
      try {
        await contentReaderRevoke(id, reader.username);
        setStatus({ kind: "ok", message: `${reader.username} no longer reads ${source.repo}.` });
        reload();
      } catch (error) {
        setStatus({ kind: "error", message: failureMessage(error) });
      }
    })();
  };

  return (
    <section class="admin__section admin__section--nested">
      <h3 class="admin__section-title">
        Readers of {source.repo}
        <button class="admin__revoke" type="button" onClick={close}>
          Close
        </button>
      </h3>
      <p class="account-page__meta">
        {source.visibility === "private"
          ? "This repository is private: only these usernames can open its lessons, find them in search, or see the book in the library. Nobody else knows it is there beyond its URL."
          : "This repository is public, so the list is inert — it is kept so that making the repository private later starts with the readers you already named."}
      </p>
      <StatusBanner status={status} />
      <form
        class="admin__grant"
        onSubmit={(event) => {
          event.preventDefault();
          grant();
        }}
      >
        <input
          class="admin__input"
          placeholder="username (their sign-in name)"
          value={username}
          onInput={(event) => setUsername((event.target as HTMLInputElement).value)}
        />
        <input
          class="admin__input admin__input--note"
          placeholder="note (optional)"
          value={note}
          onInput={(event) => setNote((event.target as HTMLInputElement).value)}
        />
        <button class="admin__grant-btn" type="submit">
          Grant
        </button>
      </form>
      <ReadersTable rows={rows} revoke={revoke} />
    </section>
  );
}

function ReadersTable({ rows, revoke }: { rows: Rows; revoke: (reader: ContentReader) => void }) {
  if (rows.kind === "loading") return <p class="account-page__loading">Loading readers…</p>;
  if (rows.kind === "failed") return <p class="account-page__status account-page__status--error">{rows.message}</p>;
  if (rows.rows.length === 0) return <p class="account-page__meta">No readers yet — a private book with no readers is readable by nobody.</p>;
  return (
    <table class="admin__table">
      <thead>
        <tr>
          <th>Username</th>
          <th>Note</th>
          <th>Granted</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {rows.rows.map((reader) => (
          <tr key={reader.username}>
            <td class="admin__cell-user">{reader.username}</td>
            <td>{reader.note ?? ""}</td>
            <td>{whenGranted(reader.grantedAt)}</td>
            <td>
              <button class="admin__revoke" onClick={() => revoke(reader)}>
                Revoke
              </button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
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
