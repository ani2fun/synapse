// The submit gate: three steps, Preview → Changes → Details, with Submit only on the last. No
// edit can be proposed without its author having LOOKED at the rendered page — that is the whole
// point of the feature, so the preview is step one and it is unskippable.
//
// A blocking lint error (a lost frontmatter fence, a page with no title) disables Submit and
// links back to the offending line; warnings are shown and do not block. So the client never
// lets a contributor submit something the server will refuse, and the reviewer's time is spent on
// prose, not on formatting a non-git contributor could not see was broken.

import { useEffect, useRef, useState } from "preact/hooks";

import { hydratePreview, renderPreview } from "./preview";
import type { PreviewHtml } from "./preview";
import { diffLines } from "./diff";
import type { Finding } from "./lint";
import { hasBlocker } from "./lint";

type Step = "preview" | "changes" | "details";
const STEPS: Step[] = ["preview", "changes", "details"];
const STEP_LABEL: Record<Step, string> = { preview: "Preview", changes: "Changes", details: "Details" };

export interface ReviewDialogProps {
  readonly original: string;
  readonly source: string;
  readonly lessonPath: string;
  readonly findings: Finding[];
  /** Non-null while a submit is in flight — disables the controls and shows the message. */
  readonly submitting: string | null;
  readonly onSubmit: (summary: string) => void;
  readonly onClose: () => void;
  /** Jump the editor to a line (a blocking finding's "fix it" link). */
  readonly onGotoLine: (line: number) => void;
}

export function ReviewDialog(props: ReviewDialogProps) {
  const [step, setStep] = useState<Step>("preview");
  const [summary, setSummary] = useState("");
  const blocked = hasBlocker(props.findings);
  const stepIndex = STEPS.indexOf(step);

  return (
    <div class="edit-review" role="dialog" aria-modal="true" aria-label="Review your change">
      <div class="edit-review__scrim" onClick={props.submitting ? undefined : props.onClose} />
      <div class="edit-review__panel">
        <header class="edit-review__head">
          <ol class="edit-review__steps">
            {STEPS.map((s, i) => (
              <li
                key={s}
                class={`edit-review__step${s === step ? " edit-review__step--active" : ""}${
                  i < stepIndex ? " edit-review__step--done" : ""
                }`}
              >
                <span class="edit-review__step-num">{i + 1}</span>
                {STEP_LABEL[s]}
              </li>
            ))}
          </ol>
          <button class="edit-review__close" aria-label="Close" onClick={props.onClose} disabled={!!props.submitting}>
            ✕
          </button>
        </header>

        <div class="edit-review__body">
          {step === "preview" && <PreviewStep source={props.source} lessonPath={props.lessonPath} findings={props.findings} onGotoLine={(l) => { props.onClose(); props.onGotoLine(l); }} />}
          {step === "changes" && <ChangesStep original={props.original} source={props.source} />}
          {step === "details" && <DetailsStep summary={summary} setSummary={setSummary} />}
        </div>

        <footer class="edit-review__foot">
          <button class="edit-review__back" onClick={() => setStep(STEPS[Math.max(0, stepIndex - 1)])} disabled={stepIndex === 0 || !!props.submitting}>
            Back
          </button>
          {props.submitting && <span class="edit-review__status">{props.submitting}</span>}
          {step === "details" ? (
            <button
              class="edit-review__submit"
              onClick={() => props.onSubmit(summary)}
              disabled={blocked || !!props.submitting}
              title={blocked ? "Fix the highlighted problems first" : undefined}
            >
              {props.submitting ? "Submitting…" : "Submit change request"}
            </button>
          ) : (
            <button class="edit-review__next" onClick={() => setStep(STEPS[stepIndex + 1])} disabled={!!props.submitting}>
              Next
            </button>
          )}
        </footer>
      </div>
    </div>
  );
}

/** Step 1 — the rendered page, reader chrome and all, plus any blocking lint problems. */
function PreviewStep({
  source,
  lessonPath,
  findings,
  onGotoLine,
}: {
  source: string;
  lessonPath: string;
  findings: Finding[];
  onGotoLine: (line: number) => void;
}) {
  const bodyRef = useRef<HTMLDivElement>(null);
  // The rendered HTML is handed to Preact via dangerouslySetInnerHTML (opaque to reconciliation)
  // rather than injected with `body.innerHTML =` — Preact would wipe the latter on its next
  // re-render, leaving a blank preview (the bug the signed-in e2e caught). `null` = still rendering.
  const [html, setHtml] = useState<PreviewHtml | null>(null);

  useEffect(() => {
    let live = true;
    setHtml(null);
    void (async () => {
      const rendered = await renderPreview(source);
      if (live) setHtml(rendered);
    })();
    return () => {
      live = false;
    };
  }, [source]);

  // Hydrate the reader's islands into the body only AFTER Preact has committed its HTML.
  useEffect(() => {
    if (html !== null && bodyRef.current) void hydratePreview(bodyRef.current);
  }, [html]);

  const blockers = findings.filter((f) => f.severity === "error");
  return (
    <div class="edit-review__preview">
      {blockers.length > 0 && (
        <div class="edit-review__blockers" role="alert">
          <p class="edit-review__blockers-head">Fix these before submitting:</p>
          <ul>
            {blockers.map((f, i) => (
              <li key={i}>
                {f.message}{" "}
                {f.line > 0 && (
                  <button class="edit-review__goto" onClick={() => onGotoLine(f.line)}>
                    go to line {f.line}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      <p class="edit-review__hint">This is how the page will look. Read it through — a reviewer sees the same thing.</p>
      {html === null && <p class="edit-review__rendering">Rendering…</p>}
      <div class="edit-review__page synapse-prose">
        <header class="lesson-header" dangerouslySetInnerHTML={{ __html: html?.headerHtml ?? "" }} />
        <div class="lesson-body" ref={bodyRef} dangerouslySetInnerHTML={{ __html: html?.bodyHtml ?? "" }} />
      </div>
    </div>
  );
}

/** Step 2 — the line diff against the base source. */
function ChangesStep({ original, source }: { original: string; source: string }) {
  const diff = diffLines(original, source);
  return (
    <div class="edit-review__changes">
      <p class="edit-review__hint">
        <span class="edit-diff__added-count">+{diff.added}</span> <span class="edit-diff__removed-count">−{diff.removed}</span>{" "}
        line{diff.added + diff.removed === 1 ? "" : "s"} changed.
      </p>
      <div class="edit-diff">
        {diff.rows.map((row, i) => (
          <div key={i} class={`edit-diff__row edit-diff__row--${row.kind}`}>
            <span class="edit-diff__gutter">{row.kind === "added" ? "+" : row.kind === "removed" ? "−" : " "}</span>
            <span class="edit-diff__text">{row.text || " "}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/** Step 3 — the PR title context and the "what and why" summary. */
function DetailsStep({ summary, setSummary }: { summary: string; setSummary: (v: string) => void }) {
  return (
    <div class="edit-review__details">
      <label class="edit-review__label" for="edit-summary">
        What did you change, and why?
      </label>
      <p class="edit-review__sub">
        This becomes the message on the pull request. A sentence is plenty — it helps the reviewer say yes faster.
      </p>
      <textarea
        id="edit-summary"
        class="edit-review__summary"
        rows={5}
        placeholder="e.g. Fixed a typo in the second paragraph and clarified the CAP example."
        value={summary}
        onInput={(e) => setSummary((e.target as HTMLTextAreaElement).value)}
      />
    </div>
  );
}
