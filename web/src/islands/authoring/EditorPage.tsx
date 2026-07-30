// The content editor page (/edit/<path>). A signed-in, allow-listed contributor edits a lesson's
// markdown here, sees it rendered exactly as a reader will, and submits — the server opens a pull
// request for them. The lesson page never pays for any of this: it is a separate route, and
// Monaco + the markdown pipeline arrive through the same lazy chunks the reader uses.
//
// State machine: loading → (denied | notFound | editing) → reviewing → submitting → done | failed.

import { render, h } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import * as api from "../../lib/api/client";
import type { EditRequest, EditSource } from "../../lib/api/client";
import { ApiFailure } from "../../lib/api/client";
import * as log from "../../lib/log";
import { lessonPathFromUrl } from "../../lib/catalog/path";
import type { EditorHandle } from "../../lib/islands/editor/monaco";
import { mountMarkdownEditor } from "../../lib/islands/editor/loader";
import { useAuthState } from "../auth/Chip";
import { signIn } from "../auth/store";
import { lint } from "./lint";
import type { Finding } from "./lint";
import { ReviewDialog } from "./ReviewDialog";
import { clearDraft, loadDraft, saveDraft, savedAgo } from "./draft";

type Phase =
  | { kind: "loading" }
  | { kind: "denied"; reason: "anonymous" | "not-allowed" | "off" }
  | { kind: "notFound"; message: string }
  | { kind: "error"; message: string }
  | { kind: "editing" }
  | { kind: "done"; request: EditRequest };

const DRAFT_DEBOUNCE_MS = 800;

export function EditorPage() {
  const auth = useAuthState();
  const lessonPath = lessonPathFromUrl().join("/");
  const [phase, setPhase] = useState<Phase>({ kind: "loading" });
  const [source, setSource] = useState<EditSource | null>(null);
  const [restored, setRestored] = useState<number | null>(null);

  // The auth store starts `loading`; wait for it to resolve before deciding anything, so a signed-in
  // reload never flashes "sign in".
  useEffect(() => {
    if (auth.kind === "loading") return;
    if (auth.kind === "anonymous") {
      setPhase({ kind: "denied", reason: "anonymous" });
      return;
    }
    let live = true;
    void (async () => {
      try {
        const loaded = await api.editSource(lessonPath);
        if (!live) return;
        setSource(loaded);
        setPhase({ kind: "editing" });
        const draft = loadDraft(auth.me.username, lessonPath);
        if (draft && draft.baseFingerprint === loaded.fingerprint && draft.source !== loaded.source) {
          setRestored(draft.savedAt);
        } else if (draft) {
          clearDraft(auth.me.username, lessonPath); // stale — the page moved under it
        }
      } catch (error) {
        if (!live) return;
        setPhase(phaseForError(error));
      }
    })();
    return () => {
      live = false;
    };
  }, [auth.kind, lessonPath]);

  if (phase.kind === "loading") return <CenteredMessage title="Loading the editor…" />;
  if (phase.kind === "denied") return <Denied reason={phase.reason} />;
  if (phase.kind === "notFound") return <CenteredMessage title="Not an editable page" body={phase.message} backHome />;
  if (phase.kind === "error") return <CenteredMessage title="The editor could not load" body={phase.message} backHome />;
  if (phase.kind === "done") return <Done request={phase.request} lessonPath={lessonPath} />;

  if (!source || auth.kind !== "authed") return <CenteredMessage title="Loading the editor…" />;
  return (
    <Editor
      username={auth.me.username}
      lessonPath={lessonPath}
      source={source}
      restoredAt={restored}
      onRestoreDismiss={() => setRestored(null)}
      onDone={(request) => setPhase({ kind: "done", request })}
    />
  );
}

/**
 * What a failed submit says. `.message` is the diagnosis (`error: detail`, formatted for parity
 * with the Rust client), and the envelope's `hint` is the remedy — the server sends a different
 * one per broken rule, and dropping it would leave a contributor told what is wrong but not what
 * to do. Kept here rather than folded into `.message`, which is a cross-client parity contract.
 */
function submitFailureText(err: unknown): string {
  const message = err instanceof Error ? err.message : String(err);
  const hint = err instanceof ApiFailure ? err.envelope?.hint : null;
  return hint ? `${message} — ${hint}` : message;
}

function phaseForError(error: unknown): Phase {
  if (error instanceof ApiFailure) {
    if (error.status === 401) return { kind: "denied", reason: "anonymous" };
    if (error.status === 403) return { kind: "denied", reason: "not-allowed" };
    if (error.status === 404) return { kind: "notFound", message: error.message };
  }
  return { kind: "error", message: error instanceof Error ? error.message : String(error) };
}

// ── the editor proper ──────────────────────────────────────────────────────────

interface EditorProps {
  username: string;
  lessonPath: string;
  source: EditSource;
  restoredAt: number | null;
  onRestoreDismiss: () => void;
  onDone: (request: EditRequest) => void;
}

function Editor(props: EditorProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const handleRef = useRef<EditorHandle | null>(null);
  const [text, setText] = useState(props.source.source);
  const [reviewing, setReviewing] = useState(false);
  const [submitting, setSubmitting] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const draftTimer = useRef<number | undefined>(undefined);

  const findings = lint(props.source.source, text);
  const dirty = text !== props.source.source;

  // Mount Monaco once. `dark` is read from <html>.dark, the same signal the reader's editors use.
  useEffect(() => {
    let disposed = false;
    void (async () => {
      if (!hostRef.current) return;
      const dark = document.documentElement.classList.contains("dark");
      const handle = await mountMarkdownEditor(hostRef.current, props.source.source, dark, (value) => setText(value));
      if (disposed) {
        handle.dispose();
        return;
      }
      handleRef.current = handle;
      log.info(`edit: Monaco mounted for ${props.lessonPath}`);
    })();
    return () => {
      disposed = true;
      handleRef.current?.dispose();
      handleRef.current = null;
    };
  }, [props.lessonPath]);

  // Autosave the draft, debounced. Cleared on a successful submit.
  useEffect(() => {
    if (!dirty) return;
    window.clearTimeout(draftTimer.current);
    draftTimer.current = window.setTimeout(() => {
      saveDraft(props.username, props.lessonPath, text, props.source.fingerprint);
      log.debug(`edit: draft saved (${text.length} bytes)`);
    }, DRAFT_DEBOUNCE_MS);
    return () => window.clearTimeout(draftTimer.current);
  }, [text, dirty, props.username, props.lessonPath, props.source.fingerprint]);

  const restore = () => {
    const draft = loadDraft(props.username, props.lessonPath);
    if (draft) {
      handleRef.current?.setValue(draft.source);
      setText(draft.source);
    }
    props.onRestoreDismiss();
  };

  const submit = async (summary: string) => {
    setSubmitting("Opening your change request…");
    setError(null);
    try {
      const request = await api.proposeEdit({
        lessonPath: props.lessonPath,
        source: text,
        baseFingerprint: props.source.fingerprint,
        summary: summary.trim() === "" ? null : summary,
      });
      clearDraft(props.username, props.lessonPath);
      log.info(`edit: submitted ${request.branch} (${request.mode})`);
      props.onDone(request);
    } catch (err) {
      const message = submitFailureText(err);
      setSubmitting(null);
      setReviewing(false);
      setError(message);
      log.warn(`edit: submit failed — ${message}`);
    }
  };

  return (
    <div class="edit-page">
      <EditorHeader
        lessonPath={props.lessonPath}
        filePath={props.source.filePath}
        dirty={dirty}
        findings={findings}
        onReview={() => setReviewing(true)}
      />
      {props.restoredAt !== null && (
        <div class="edit-page__draft" role="status">
          You have an unsaved draft from {savedAgo(props.restoredAt)}.
          <button class="edit-page__draft-btn" onClick={restore}>
            Restore it
          </button>
          <button class="edit-page__draft-btn edit-page__draft-btn--ghost" onClick={props.onRestoreDismiss}>
            Discard
          </button>
        </div>
      )}
      {error && (
        <div class="edit-page__error" role="alert">
          {error}
        </div>
      )}
      <div class="edit-page__editor" ref={hostRef} />
      <LintStrip findings={findings} onGotoLine={(line) => handleRef.current?.setLineHighlights(line, null)} />

      {reviewing && (
        <ReviewDialog
          original={props.source.source}
          source={text}
          lessonPath={props.lessonPath}
          repo={props.source.repo}
          baseBranch={props.source.baseBranch}
          findings={findings}
          submitting={submitting}
          onSubmit={(summary) => void submit(summary)}
          onClose={() => (submitting ? undefined : setReviewing(false))}
          onGotoLine={(line) => handleRef.current?.setLineHighlights(line, null)}
        />
      )}
    </div>
  );
}

function EditorHeader({
  lessonPath,
  filePath,
  dirty,
  findings,
  onReview,
}: {
  lessonPath: string;
  filePath: string;
  dirty: boolean;
  findings: Finding[];
  onReview: () => void;
}) {
  const errors = findings.filter((f) => f.severity === "error").length;
  return (
    <header class="edit-page__head">
      <div class="edit-page__crumbs">
        <a class="edit-page__crumb" href={`/synapse/${lessonPath}`}>
          ← Back to the page
        </a>
        <code class="edit-page__file">{filePath}</code>
      </div>
      <div class="edit-page__actions">
        <span class="edit-page__saved">{dirty ? "Draft saved in this browser" : "No changes yet"}</span>
        <button class="edit-page__review" onClick={onReview} disabled={!dirty}>
          Review &amp; submit{errors > 0 ? ` (${errors} to fix)` : ""}
        </button>
      </div>
    </header>
  );
}

function LintStrip({ findings, onGotoLine }: { findings: Finding[]; onGotoLine: (line: number) => void }) {
  if (findings.length === 0) return null;
  return (
    <ul class="edit-lint">
      {findings.map((f, i) => (
        <li key={i} class={`edit-lint__item edit-lint__item--${f.severity}`}>
          <span class="edit-lint__badge">{f.severity === "error" ? "Fix" : "Note"}</span>
          <span class="edit-lint__msg">{f.message}</span>
          {f.line > 0 && (
            <button class="edit-lint__goto" onClick={() => onGotoLine(f.line)}>
              line {f.line}
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}

// ── the terminal + gate states ───────────────────────────────────────────────

function Denied({ reason }: { reason: "anonymous" | "not-allowed" | "off" }) {
  if (reason === "anonymous") {
    return (
      <div class="edit-gate">
        <h1 class="edit-gate__title">Sign in to suggest an edit</h1>
        <p class="edit-gate__body">Editing a page needs a signed-in account on the content-editor list.</p>
        <button class="edit-gate__btn" onClick={() => signIn()}>
          Sign in
        </button>
      </div>
    );
  }
  if (reason === "off") {
    return (
      <div class="edit-gate">
        <h1 class="edit-gate__title">Editing is not enabled here</h1>
        <p class="edit-gate__body">This deployment does not accept in-app content edits.</p>
        <a class="edit-gate__link" href="/">
          ← Back to the library
        </a>
      </div>
    );
  }
  // Same wording as the lesson page's gated tooltip — someone who lands here directly should get
  // the identical instruction, not a second, subtly different story.
  return (
    <div class="edit-gate">
      <h1 class="edit-gate__title">You are not a content editor yet</h1>
      <p class="edit-gate__body">
        Editing needs a place on the content-editor list. To request one, email your GitHub username to{" "}
        <a class="edit-gate__mail" href="mailto:a.r.kakde@gmail.com">
          a.r.kakde@gmail.com
        </a>{" "}
        — access may or may not be granted. Thanks for understanding.
      </p>
      <p class="edit-gate__body">Editing is a separate grant from running code.</p>
      <a class="edit-gate__link" href="/">
        ← Back to the library
      </a>
    </div>
  );
}

function Done({ request, lessonPath }: { request: EditRequest; lessonPath: string }) {
  const dryRun = request.mode !== "github" || !request.prUrl;
  return (
    <div class="edit-gate edit-gate--done">
      <h1 class="edit-gate__title">{dryRun ? "Change recorded" : "Change request opened"}</h1>
      {dryRun ? (
        <p class="edit-gate__body">
          Your change was validated and recorded on branch <code>{request.branch}</code>, but this deployment is in
          dry-run mode, so no pull request was opened.
        </p>
      ) : (
        <p class="edit-gate__body">
          Thank you — your suggestion is now {request.reused ? "added to your open" : "a"} pull request for a maintainer
          to review.
        </p>
      )}
      <div class="edit-gate__row">
        {request.prUrl && (
          <a class="edit-gate__btn" href={request.prUrl} target="_blank" rel="noopener noreferrer">
            View the pull request
          </a>
        )}
        <a class="edit-gate__link" href={`/synapse/${lessonPath}`}>
          ← Back to the page
        </a>
      </div>
    </div>
  );
}

function CenteredMessage({ title, body, backHome }: { title: string; body?: string; backHome?: boolean }) {
  return (
    <div class="edit-gate">
      <h1 class="edit-gate__title">{title}</h1>
      {body && <p class="edit-gate__body">{body}</p>}
      {backHome && (
        <a class="edit-gate__link" href="/">
          ← Back to the library
        </a>
      )}
    </div>
  );
}

// ── mount ─────────────────────────────────────────────────────────────────────
const root = document.querySelector<HTMLElement>("[data-edit-root]");
if (root) {
  render(h(EditorPage, {}), root);
  log.info("edit page mounted");
}
