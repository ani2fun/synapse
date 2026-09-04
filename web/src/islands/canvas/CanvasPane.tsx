/**
 * The Think pane — the Algorithm Design Canvas over a problem's workbench.
 *
 * It holds ONE live document (the draft, autosaved to this browser) and a list of SAVED entries
 * (the account's, in Postgres, the way submissions are). The two are deliberately different kinds
 * of thing: a draft is what you are thinking now and needs no account, an entry is a snapshot you
 * chose to keep and follows you between machines.
 *
 * Re-render discipline: the form's fields are uncontrolled and the body is a mutable ref, so
 * typing does NOT re-render. The parent re-renders only when the derived readout changes — an area
 * crossing empty ↔ non-empty, which is exactly when the meter moves. `formKey` bumps when a
 * DIFFERENT document is loaded (a saved entry, or Clear), remounting every field.
 */
import { useEffect, useRef, useState } from "preact/hooks";

import * as api from "../../lib/api/client";
import type { CanvasEntry } from "../../lib/api/client";
import * as log from "../../lib/log";
import { AUTH_CHANGED, currentUser, isAuthed } from "../workbench/contracts";
import { CanvasForm } from "./CanvasForm";
import { InfoModal } from "./InfoModal";
import { SavedEntries } from "./SavedEntries";
import type { EntriesState } from "./SavedEntries";
import * as draftStore from "./draft";
import { GUIDANCE } from "./guidance";
import type { Area, CanvasBody } from "./model";
import {
  blankBody,
  entryTitle,
  exportEnvelope,
  fileStem,
  filledCount,
  isBlank,
  newIdeaId,
  normalizeBody,
  TOTAL_AREAS,
  toWire,
} from "./model";

const AUTOSAVE_MS = 400;
const TOAST_MS = 2200;

/** Hand a JSON file to the reader. A real anchor click, because that is the only download a page
 *  can start for itself; the object URL is revoked once the browser has taken it. */
function download(name: string, payload: unknown): void {
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export function CanvasPane({ path, title }: { path: string[]; title: string }) {
  // The live document. A ref, not state: the form writes into it on every keystroke and must not
  // drag a render with it.
  const body = useRef<CanvasBody>(blankBody());
  const filled = useRef(0);
  const saveTimer = useRef<number | undefined>(undefined);
  const toastTimer = useRef<number | undefined>(undefined);

  const [formKey, setFormKey] = useState("k0");
  const [meter, setMeter] = useState(0);
  const [view, setView] = useState<"canvas" | "saved">("canvas");
  const [entries, setEntries] = useState<EntriesState>({ kind: "loading" });
  const [viewing, setViewing] = useState<CanvasEntry | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [, setAuthTick] = useState(0);

  const draftKey = () => draftStore.keyFor(currentUser(), path);

  const say = (message: string) => {
    setToast(message);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), TOAST_MS);
  };

  /** Re-read the derived count; render only if it moved. */
  const syncMeter = () => {
    const next = filledCount(body.current);
    if (next !== filled.current) {
      filled.current = next;
      setMeter(next);
    }
  };

  const queueSave = () => {
    if (viewing) return; // reading a saved entry — nothing to autosave
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => draftStore.save(draftKey(), body.current), AUTOSAVE_MS);
  };

  /** Write NOW, cancelling the pending debounce. The debounce exists so typing does not hit storage
   *  per keystroke, but it also means the last few hundred milliseconds of writing are unsaved at
   *  any moment — and a page being hidden or torn down is exactly when that matters. A pane that
   *  promises "draft autosaves" must not lose the last word to a closed tab. */
  const flushSave = () => {
    if (viewing) return;
    window.clearTimeout(saveTimer.current);
    draftStore.save(draftKey(), body.current);
  };

  /** Replace the whole document — the only path that remounts the fields. */
  const loadBody = (next: CanvasBody) => {
    body.current = next;
    filled.current = filledCount(next);
    setMeter(filled.current);
    setFormKey(`k${Date.now()}`);
  };

  const loadEntries = () => {
    if (!isAuthed()) {
      setEntries({ kind: "anonymous" });
      return;
    }
    setEntries({ kind: "loading" });
    api.canvasEntriesFor(path).then(
      (list) => {
        log.info(`canvas: ${list.length} saved entr(ies) for /${path.join("/")}`);
        setEntries({ kind: "ok", list });
      },
      (error: unknown) =>
        setEntries({ kind: "error", message: error instanceof Error ? error.message : String(error) }),
    );
  };

  useEffect(() => {
    log.info(`canvas pane mounted — /${path.join("/")}`);
    const saved = draftStore.load(draftKey());
    if (saved) {
      log.debug("canvas: restored the local draft");
      loadBody(saved);
    }
    loadEntries();
    // A sign-in mid-session changes both what the Saved view may show AND which draft key is
    // ours, so the pane re-reads both rather than waiting for a reload.
    const onAuth = () => {
      setAuthTick((n) => n + 1);
      loadEntries();
    };
    window.addEventListener(AUTH_CHANGED, onAuth);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setExpanded(false);
    };
    window.addEventListener("keydown", onKey);
    // `pagehide` fires where `beforeunload` is unreliable (bfcache, mobile Safari), and
    // `visibilitychange` catches a tab switched away from and never returned to.
    const onLeave = () => flushSave();
    const onHide = () => {
      if (document.visibilityState === "hidden") flushSave();
    };
    window.addEventListener("pagehide", onLeave);
    document.addEventListener("visibilitychange", onHide);
    return () => {
      window.removeEventListener(AUTH_CHANGED, onAuth);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pagehide", onLeave);
      document.removeEventListener("visibilitychange", onHide);
      window.clearTimeout(saveTimer.current);
      window.clearTimeout(toastTimer.current);
    };
  }, []);

  // ── editing ───────────────────────────────────────────────────────────────────────────────

  const onArea = (area: Area, value: string) => {
    body.current[area] = value;
    syncMeter();
    queueSave();
  };

  const onIdea = (id: string, prop: "name" | "desc" | "time" | "space", value: string) => {
    const idea = body.current.ideas.find((candidate) => candidate.id === id);
    if (!idea) return;
    idea[prop] = value;
    syncMeter();
    queueSave();
  };

  const onAddIdea = () => {
    // Named on arrival, the way the two starters are. The name field carries no placeholder (the
    // ℹ️ covers what an idea is), so an unnamed row would be a bare box saying nothing at all.
    const name = `Idea ${body.current.ideas.length + 1}`;
    body.current.ideas = [...body.current.ideas, { id: newIdeaId(), name, desc: "", time: "", space: "" }];
    setFormKey(`k${Date.now()}`);
    queueSave();
  };

  const onRemoveIdea = (id: string) => {
    if (body.current.ideas.length <= 1) return;
    body.current.ideas = body.current.ideas.filter((idea) => idea.id !== id);
    setFormKey(`k${Date.now()}`);
    syncMeter();
    queueSave();
  };

  // ── entries ───────────────────────────────────────────────────────────────────────────────

  const saveEntry = () => {
    if (viewing) return;
    if (!isAuthed()) {
      say("Sign in to save an entry");
      setView("saved");
      return;
    }
    if (isBlank(body.current)) {
      say("Nothing to save yet");
      return;
    }
    api.saveCanvasEntry({ path, body: toWire(body.current) }).then(
      (entry) => {
        log.info(`canvas entry saved — ${entry.id}`);
        setEntries((current) =>
          current.kind === "ok" ? { kind: "ok", list: [entry, ...current.list] } : current,
        );
        say("Entry saved");
      },
      (error: unknown) => say(error instanceof Error ? error.message : "Save failed"),
    );
  };

  const viewEntry = (entry: CanvasEntry) => {
    loadBody(normalizeBody(entry.body));
    setViewing(entry);
    setView("canvas");
  };

  const closeViewing = () => {
    setViewing(null);
    loadBody(draftStore.load(draftKey()) ?? blankBody());
  };

  const deleteEntry = (entry: CanvasEntry) => {
    api.deleteCanvasEntry(entry.id).then(
      () => {
        setEntries((current) =>
          current.kind === "ok"
            ? { kind: "ok", list: current.list.filter((row) => row.id !== entry.id) }
            : current,
        );
        if (viewing?.id === entry.id) closeViewing();
        say("Entry deleted");
      },
      (error: unknown) => say(error instanceof Error ? error.message : "Delete failed"),
    );
  };

  const clearDraft = () => {
    if (viewing) return;
    draftStore.clear(draftKey());
    loadBody(blankBody());
    say("Canvas cleared");
  };

  // ── export ────────────────────────────────────────────────────────────────────────────────

  const stem = fileStem(path);

  const exportDraft = () => {
    download(`${stem}-canvas-draft.json`, exportEnvelope(title, path, { draft: body.current }));
    say("Draft exported");
  };

  const exportAll = () => {
    if (entries.kind !== "ok" || entries.list.length === 0) {
      say("No saved entries to export");
      return;
    }
    download(`${stem}-canvas-entries.json`, exportEnvelope(title, path, { entries: entries.list }));
    say(`Exported ${entries.list.length} entr${entries.list.length === 1 ? "y" : "ies"}`);
  };

  const exportOne = (entry: CanvasEntry) => {
    download(`${stem}-canvas-${entry.id.slice(0, 8)}.json`, exportEnvelope(title, path, { entries: [entry] }));
    say("Entry exported");
  };

  // ── render ────────────────────────────────────────────────────────────────────────────────

  const count = entries.kind === "ok" ? entries.list.length : 0;
  const guidance = info ? GUIDANCE[info] : undefined;

  return (
    <div class={`pcanvas${expanded ? " pcanvas--expanded" : ""}`}>
      <div class="pcanvas__bar">
        <span class="pcanvas__bar-title">Design canvas</span>
        <span class="pcanvas__meter" title={`${meter} of ${TOTAL_AREAS} areas filled`}>
          <span class="pcanvas__meter-fill" style={`width:${Math.round((meter / TOTAL_AREAS) * 100)}%`} />
        </span>
        <span class="pcanvas__meter-label">
          {meter} / {TOTAL_AREAS}
        </span>
        <span class="pcanvas__bar-spacer" />
        <span class="pcanvas__seg">
          <button
            class={`pcanvas__seg-btn${view === "canvas" ? " pcanvas__seg-btn--on" : ""}`}
            type="button"
            aria-pressed={view === "canvas"}
            onClick={() => setView("canvas")}
          >
            Canvas
          </button>
          <button
            class={`pcanvas__seg-btn${view === "saved" ? " pcanvas__seg-btn--on" : ""}`}
            type="button"
            aria-pressed={view === "saved"}
            onClick={() => setView("saved")}
          >
            Saved · {count}
          </button>
        </span>
        <button class="pcanvas__btn" type="button" title="Export the current unsaved draft as JSON" onClick={exportDraft}>
          Export draft
        </button>
        <button class="pcanvas__btn" type="button" title="Export every saved entry as JSON" onClick={exportAll}>
          JSON
        </button>
        <button class="pcanvas__btn pcanvas__btn--primary" type="button" onClick={saveEntry} disabled={viewing !== null}>
          Save entry
        </button>
        <button
          class="pcanvas__btn pcanvas__btn--icon"
          type="button"
          title={expanded ? "Exit full screen (Esc)" : "Expand the canvas to full screen"}
          aria-label={expanded ? "Exit full screen" : "Expand the canvas to full screen"}
          aria-pressed={expanded}
          onClick={() => setExpanded((on) => !on)}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M15 3h6v6 M9 21H3v-6 M21 3l-7 7 M3 21l7-7" />
          </svg>
        </button>
      </div>

      {viewing && (
        <div class="pcanvas__viewing">
          <span class="pcanvas__viewing-label">Viewing saved entry</span>
          <span class="pcanvas__viewing-title">{entryTitle(normalizeBody(viewing.body))}</span>
          <span class="pcanvas__bar-spacer" />
          <button class="pcanvas__btn" type="button" onClick={closeViewing}>
            Back to draft
          </button>
        </div>
      )}

      <div class="pcanvas__body">
        {view === "canvas" ? (
          <>
            <CanvasForm
              body={body.current}
              formKey={formKey}
              onArea={onArea}
              onIdea={onIdea}
              onAddIdea={onAddIdea}
              onRemoveIdea={onRemoveIdea}
              onInfo={setInfo}
              readOnly={viewing !== null}
            />
            {!viewing && (
              <div class="pcanvas__foot">
                <span class="pcanvas__foot-note">Draft autosaves on this device</span>
                <span class="pcanvas__bar-spacer" />
                <button class="pcanvas__btn pcanvas__btn--danger" type="button" onClick={clearDraft}>
                  Clear
                </button>
              </div>
            )}
          </>
        ) : (
          <SavedEntries state={entries} onView={viewEntry} onExport={exportOne} onDelete={deleteEntry} />
        )}
      </div>

      {guidance && <InfoModal info={guidance} onClose={() => setInfo(null)} />}
      {toast && <div class="pcanvas__toast">{toast}</div>}
    </div>
  );
}
