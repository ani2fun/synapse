/**
 * The problem page's Submissions tab. The caller's OWN submissions for this lesson, newest
 * first: verdict badges, a revealed code card, "Copy to editor" into the right pane's matching
 * language tab, and "Use this test case" pulling a rejection's failing input into the tests
 * panel. Anonymous readers get the sign-in note.
 *
 * Cross-pane wiring is by event, not signal (the `workbench/contracts.ts` contract): "Copy to
 * editor" dispatches LOAD_CODE and "Use this test case" dispatches USE_CASE ON the workbench
 * root; a completed submit bubbles SUBMITTED up from that root, and this feed refetches on it.
 * The workbench root is handed in as a getter because it is minted after this component first
 * renders (the right pane mounts its Workbench independently).
 */
import { useEffect, useRef, useState } from "preact/hooks";

import * as api from "../lib/api/client";
import type { Submission } from "../lib/api/client";
import { canReproduce } from "../lib/execution/blocks";
import type { TestSpec } from "../lib/execution/judge";
import { highlightCode } from "../lib/markdown/render";
import { isAuthed, LOAD_CODE, SUBMITTED, USE_CASE } from "./workbench/contracts";
import type { LoadCode, UseCase } from "./workbench/contracts";
import * as log from "../lib/log";

interface FeedProps {
  path: string[];
  /** The extracted workbench's suite, read only for the reproducibility guard. */
  spec: TestSpec | null;
  /** The workbench root element (event target), or null until the right pane mounts. */
  workbenchRoot: () => HTMLElement | null;
}

type Rows = { kind: "loading" } | { kind: "error"; message: string } | { kind: "ok"; list: Submission[] };

// ─────────────────────────────────────────────────────────────────────────────
// VERDICT BADGE + ROW HELPERS
// ─────────────────────────────────────────────────────────────────────────────

function badgeFor(verdict: string | null | undefined): [cls: string, text: string] {
  switch (verdict) {
    case "accepted":
      return ["subs__status subs__status--ok", "Accepted"];
    case "rejected":
      return ["subs__status subs__status--fail", "Wrong answer"];
    case "judge-failed":
      return ["subs__status subs__status--warn", "Judge failed"];
    default:
      return ["subs__status", "pending"];
  }
}

function casesLabel(dto: Submission): string | null {
  return dto.passed != null && dto.total != null ? `${dto.passed}/${dto.total} cases` : null;
}

function timeLabel(createdAt: string): string {
  return createdAt.slice(0, 19).replace("T", " ");
}

// ─────────────────────────────────────────────────────────────────────────────
// USE-THIS-TEST-CASE — only a rejection has a revealed input to pull
// ─────────────────────────────────────────────────────────────────────────────

function UseCaseButton({
  dto,
  spec,
  workbenchRoot,
}: {
  dto: Submission;
  spec: TestSpec | null;
  workbenchRoot: () => HTMLElement | null;
}) {
  const failure = dto.firstFailure;
  if (!failure) return null;
  const reproducible = spec != null && canReproduce(spec, failure.args);
  const tip = reproducible
    ? "Adds this input as a new case below. It reproduces the input, not necessarily the judge's exact stdin."
    : "This problem is judged against a larger hidden suite whose inputs don't line up with the fields below.";
  return (
    <button
      class="wb__use-case"
      disabled={!reproducible}
      data-tip={tip}
      onClick={() => {
        const root = workbenchRoot();
        if (!root) return;
        log.debug("failing input dispatched to the tests panel");
        const detail: UseCase = { args: failure.args, expected: failure.expected ?? null };
        root.dispatchEvent(new CustomEvent(USE_CASE, { detail }));
      }}
    >
      Use this test case
    </button>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE TABLE
// ─────────────────────────────────────────────────────────────────────────────

function SubsTable({
  list,
  selected,
  onSelect,
  spec,
  workbenchRoot,
}: {
  list: Submission[];
  selected: string | null;
  onSelect: (id: string | null) => void;
  spec: TestSpec | null;
  workbenchRoot: () => HTMLElement | null;
}) {
  return (
    <table class="subs__table">
      <thead>
        <tr>
          <th>No.</th>
          <th>Status</th>
          <th>Language</th>
          <th>Code</th>
        </tr>
      </thead>
      <tbody>
        {list.map((dto, i) => {
          const [badgeClass, badgeText] = badgeFor(dto.verdict);
          const cases = casesLabel(dto);
          const on = selected === dto.id;
          return (
            <tr class="subs__row" key={dto.id}>
              <td class="subs__cell subs__cell--no">{i + 1}</td>
              <td class="subs__cell">
                <span class={badgeClass}>{badgeText}</span>
                {cases && <span class="subs__meta">{cases}</span>}
                <span class="subs__time">{timeLabel(dto.createdAt)}</span>
              </td>
              <td class="subs__cell">
                <span class="subs__lang">{dto.language}</span>
              </td>
              <td class="subs__cell subs__cell--action">
                <button
                  class={`subs__icon-btn${on ? " subs__icon-btn--on" : ""}`}
                  title="View the code"
                  onClick={() => onSelect(on ? null : dto.id)}
                >
                  {"\u{1F441}"}
                </button>
                <UseCaseButton dto={dto} spec={spec} workbenchRoot={workbenchRoot} />
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE REVEALED CODE CARD — shiki-highlighted, loadable into the right pane
// ─────────────────────────────────────────────────────────────────────────────

function CodeCard({
  dto,
  onClose,
  workbenchRoot,
}: {
  dto: Submission;
  onClose: () => void;
  workbenchRoot: () => HTMLElement | null;
}) {
  const preRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let live = true;
    void highlightCode(dto.source, dto.language).then(
      (html) => {
        if (live && preRef.current) preRef.current.innerHTML = html;
      },
      (error: unknown) => {
        log.warn(`submission highlight failed: ${String(error)}`);
        if (live && preRef.current) preRef.current.textContent = dto.source;
      },
    );
    return () => {
      live = false;
    };
  }, [dto.id]);
  const title = `Submission ${dto.id.slice(0, 8)} · ${dto.language}`;
  return (
    <div class="subs__code">
      <div class="subs__code-head">
        <span class="subs__code-title">{title}</span>
        <span class="subs__code-actions">
          <button
            class="wb__ghost"
            title="Load this submission into its language tab on the right"
            onClick={() => {
              const root = workbenchRoot();
              if (!root) return;
              log.debug(`submission source copied into the ${dto.language} tab`);
              const detail: LoadCode = { language: dto.language, code: dto.source };
              root.dispatchEvent(new CustomEvent(LOAD_CODE, { detail }));
            }}
          >
            Copy to editor
          </button>
          <button class="subs__code-close" aria-label="Close" onClick={onClose}>
            {"×"}
          </button>
        </span>
      </div>
      <div class="subs__pre" ref={preRef}></div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// THE FEED
// ─────────────────────────────────────────────────────────────────────────────

export function SubmissionsFeed({ path, spec, workbenchRoot }: FeedProps) {
  const [rows, setRows] = useState<Rows>({ kind: "loading" });
  const [selected, setSelected] = useState<string | null>(null);

  const load = () => {
    if (!isAuthed()) return;
    setRows({ kind: "loading" });
    api.submissionsFor(path).then(
      (list) => {
        log.info(`submissions loaded — ${list.length} for /${path.join("/")}`);
        setRows({ kind: "ok", list });
      },
      (error: unknown) => setRows({ kind: "error", message: error instanceof Error ? error.message : String(error) }),
    );
  };

  useEffect(() => {
    load();
    // A completed submit bubbles SUBMITTED from the workbench root to the document — refetch on it.
    const onSubmitted = () => {
      log.debug("submitted — refetching the submissions feed");
      load();
    };
    document.addEventListener(SUBMITTED, onSubmitted);
    return () => document.removeEventListener(SUBMITTED, onSubmitted);
  }, []);

  if (!isAuthed()) {
    return (
      <div class="psub not-prose">
        <p class="psub__note">Sign in to see your submissions — they're private to you.</p>
      </div>
    );
  }

  if (rows.kind === "loading") {
    return (
      <div class="psub not-prose">
        <p class="psub__note">Loading your submissions…</p>
      </div>
    );
  }
  if (rows.kind === "error") {
    return (
      <div class="psub not-prose">
        <p class="psub__note psub__note--error">Couldn't load submissions — {rows.message}</p>
      </div>
    );
  }
  if (rows.list.length === 0) {
    return (
      <div class="psub not-prose">
        <p class="psub__note">No submissions yet — solve it and hit Submit.</p>
      </div>
    );
  }

  const current = rows.list.slice(0, 1);
  const code = selected != null ? rows.list.find((d) => d.id === selected) : undefined;
  return (
    <div class="psub not-prose">
      <h3 class="psub__section">Current submission</h3>
      <SubsTable list={current} selected={selected} onSelect={setSelected} spec={spec} workbenchRoot={workbenchRoot} />
      <h3 class="psub__section">All submissions</h3>
      <SubsTable list={rows.list} selected={selected} onSelect={setSelected} spec={spec} workbenchRoot={workbenchRoot} />
      <p class="psub__note psub__count">Showing {rows.list.length} submission(s)</p>
      {code && <CodeCard dto={code} onClose={() => setSelected(null)} workbenchRoot={workbenchRoot} />}
    </div>
  );
}
