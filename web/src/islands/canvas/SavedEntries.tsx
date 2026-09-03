/**
 * The Saved view: every canvas this account has kept for this problem, newest first, with the
 * three verbs the reader actually wants — read it again, take it away as JSON, throw it out.
 *
 * Everything in a row except the timestamp is DERIVED from the body the server sent (title, areas
 * filled, best complexity). Nothing is denormalised into the row, so a row cannot describe an
 * entry differently from the form that opens it.
 */
import type { CanvasEntry } from "../../lib/api/client";
import { bestComplexity, entryTitle, filledCount, normalizeBody, TOTAL_AREAS } from "./model";

export type EntriesState =
  | { kind: "anonymous" }
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "ok"; list: CanvasEntry[] };

function whenLabel(iso: string): string {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return iso;
  return at.toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

export function SavedEntries({
  state,
  onView,
  onExport,
  onDelete,
}: {
  state: EntriesState;
  onView: (entry: CanvasEntry) => void;
  onExport: (entry: CanvasEntry) => void;
  onDelete: (entry: CanvasEntry) => void;
}) {
  if (state.kind === "anonymous") {
    return <p class="pcanvas__note">Sign in to keep entries — they're private to you, and they follow your account.</p>;
  }
  if (state.kind === "loading") {
    return <p class="pcanvas__note">Loading your saved canvases…</p>;
  }
  if (state.kind === "error") {
    return <p class="pcanvas__note pcanvas__note--error">Couldn't load your entries — {state.message}</p>;
  }
  if (state.list.length === 0) {
    return (
      <div class="pcanvas__empty">
        <p class="pcanvas__empty-label">No saved entries yet</p>
        <p class="pcanvas__empty-body">
          Fill the canvas and hit <strong>Save entry</strong> — each save is a timestamped snapshot you can re-read,
          export, or delete.
        </p>
      </div>
    );
  }

  const total = state.list.length;
  return (
    <table class="pcanvas__table">
      <thead>
        <tr>
          <th>#</th>
          <th>Entry</th>
          <th>Areas</th>
          <th>Best</th>
          <th>When</th>
          <th class="pcanvas__th--end">Actions</th>
        </tr>
      </thead>
      <tbody>
        {state.list.map((entry, index) => {
          const body = normalizeBody(entry.body);
          return (
            <tr key={entry.id}>
              <td class="pcanvas__td--no">{total - index}</td>
              <td>{entryTitle(body)}</td>
              <td class="pcanvas__td--mono">
                {filledCount(body)} / {TOTAL_AREAS}
              </td>
              <td class="pcanvas__td--mono">{bestComplexity(body)}</td>
              <td class="pcanvas__td--when">{whenLabel(entry.createdAt)}</td>
              <td>
                <div class="pcanvas__actions">
                  <button
                    class="pcanvas__icon"
                    type="button"
                    title="View this entry"
                    aria-label="View entry"
                    onClick={() => onView(entry)}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                      <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z" />
                      <circle cx="12" cy="12" r="3" />
                    </svg>
                  </button>
                  <button
                    class="pcanvas__icon"
                    type="button"
                    title="Export this entry as JSON"
                    aria-label="Export entry"
                    onClick={() => onExport(entry)}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                      <path d="M7 10l5 5 5-5 M12 15V3" />
                    </svg>
                  </button>
                  <button
                    class="pcanvas__icon pcanvas__icon--danger"
                    type="button"
                    title="Delete this entry"
                    aria-label="Delete entry"
                    onClick={() => onDelete(entry)}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                      <path d="M3 6h18 M8 6V4h8v2 M19 6l-1 14H6L5 6" />
                    </svg>
                  </button>
                </div>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
