/**
 * "Add to a lesson…" — a diagram drafted at `/d2` or `/mermaid` becomes a pull request against
 * whichever repo owns the lesson it is going into.
 *
 * There is NO new endpoint behind this. The authoring pipeline already takes a whole file
 * (`GET /api/edits/source/{path}` → `POST /api/edits`), so adding a fence is a string splice
 * between two calls that exist, and it inherits the fingerprint check, the allowlist, the rate
 * limit and the dry-run mode for free.
 *
 * Two of the server's behaviours are surfaced rather than fought:
 *   · a second proposal for the same lesson while its PR is open becomes another COMMIT on that
 *     PR, not a second one — so the success copy says which happened;
 *   · `local-only/` content is never editable, so those lessons are filtered out of the picker
 *     rather than 404-ing at submit.
 */
import { h } from "preact";
import { useEffect, useMemo, useState } from "preact/hooks";

import { Icon } from "./icons";
import { CONTACT_EMAIL, EDIT_ACCESS_TEXT } from "../../lib/contact";
import { type DiagramLang, fencesOfLang } from "./lang";
import type { Subject } from "./subject";
import * as api from "../../lib/api/client";
import type { EditRequest, EditSource } from "../../lib/api/client";
import { type SearchEntry, entries as flattenIndex, search as rankEntries } from "../../lib/search";
import { hasBlocker, lint } from "../authoring/lint";
import { useAuthState } from "../auth/Chip";
import { signIn } from "../auth/store";

type Phase =
  | { kind: "picking" }
  | { kind: "placing"; lessonPath: string; source: EditSource }
  | { kind: "sending" }
  | { kind: "done"; request: EditRequest }
  | { kind: "denied"; reason: "anonymous" | "not-allowed" | "off" }
  | { kind: "error"; message: string };

/** A lesson's path, as the picker and the API both spell it. */
const pathOf = (entry: SearchEntry): string | null =>
  entry.page.kind === "lesson" ? entry.page.path.join("/") : null;

export function AddToLesson({
  lang,
  fence,
  subject,
  published,
  onClose,
}: {
  /** Which fence list `at` indexes — the splice reads the wrong diagram if this disagrees with
   *  the pill that opened the editor. */
  lang: DiagramLang;
  fence: string;
  /** The diagram this page was opened on, when it was. Its presence is what turns this dialog
   *  from "choose a lesson and add" into "put this back where it came from". */
  subject?: Subject | null;
  /** The source the lesson held when it was loaded — checked before anything is overwritten. */
  published?: string | null;
  onClose: () => void;
}) {
  const auth = useAuthState();
  const updating = subject != null;
  const [phase, setPhase] = useState<Phase>({ kind: "picking" });
  const [query, setQuery] = useState("");
  const [all, setAll] = useState<SearchEntry[]>([]);
  const [summary, setSummary] = useState("");
  const [anchor, setAnchor] = useState("");

  // The gate is re-checked on every auth change, not just at mount: a reader can sign in while
  // this dialog is open, and a stale "sign in first" is worse than no gate at all.
  useEffect(() => {
    if (auth.kind === "loading") return;
    if (auth.kind === "anonymous") {
      setPhase({ kind: "denied", reason: "anonymous" });
      return;
    }
    void api.editConfig().then(
      (config) => {
        if (!config.enabled) setPhase({ kind: "denied", reason: "off" });
        else if (!config.canEdit) setPhase({ kind: "denied", reason: "not-allowed" });
        else setPhase((was) => (was.kind === "denied" ? { kind: "picking" } : was));
      },
      () => setPhase({ kind: "denied", reason: "off" }), // 404 = the routes are not mounted
    );
  }, [auth.kind]);

  // Lessons only — a diagram goes in a lesson, so the blog half of the flattener is left empty.
  // Skipped entirely for an update: that lesson is already known, and offering a choice would
  // invite putting the diagram somewhere it did not come from.
  useEffect(() => {
    if (updating) return;
    void api.fetchIndex().then(
      (index) => setAll(flattenIndex(index, [])),
      () => setAll([]),
    );
  }, [updating]);

  // …so an update goes straight to the confirm step once the gate has let it through.
  useEffect(() => {
    if (!updating || phase.kind !== "picking") return;
    let live = true;
    void api.editSource(subject.lessonPath).then(
      (source) => {
        if (live) setPhase({ kind: "placing", lessonPath: subject.lessonPath, source });
      },
      (error: unknown) => {
        if (live) setPhase({ kind: "error", message: messageOf(error) });
      },
    );
    return () => {
      live = false;
    };
  }, [updating, phase.kind]);

  const results = useMemo(() => {
    const lessons = all.filter((entry) => {
      const path = pathOf(entry);
      // Local-only content is unconditionally non-editable (ADR-RS002); offering it would only
      // produce a 404 at submit.
      return path != null && !path.startsWith("local-only");
    });
    return (query.trim() === "" ? lessons : rankEntries(query, lessons)).slice(0, 20);
  }, [all, query]);

  const pick = async (entry: SearchEntry) => {
    const lessonPath = pathOf(entry);
    if (lessonPath == null) return;
    try {
      const source = await api.editSource(lessonPath);
      setPhase({ kind: "placing", lessonPath, source });
    } catch (error) {
      setPhase({ kind: "error", message: messageOf(error) });
    }
  };

  const propose = async () => {
    if (phase.kind !== "placing") return;
    let spliced: string;
    try {
      spliced = updating
        ? // Back where it came from, and only if it is still the diagram that was opened.
          replaceFence(phase.source.source, lang, subject.at, subject.count, fence, published ?? "")
        : insertFence(phase.source.source, fence, anchor);
    } catch (error) {
      setPhase({ kind: "error", message: messageOf(error) });
      return;
    }
    const findings = lint(phase.source.source, spliced);
    if (hasBlocker(findings)) {
      setPhase({ kind: "error", message: findings.find((f) => f.severity === "error")!.message });
      return;
    }
    setPhase({ kind: "sending" });
    try {
      const request = await api.proposeEdit({
        lessonPath: phase.lessonPath,
        source: spliced,
        baseFingerprint: phase.source.fingerprint,
        summary: summary.trim() === "" ? null : summary,
      });
      setPhase({ kind: "done", request });
    } catch (error) {
      setPhase({ kind: "error", message: messageOf(error) });
    }
  };

  return (
    <div
      class="mdl"
      role="dialog"
      aria-modal="true"
      aria-label={updating ? "Update this diagram" : "Add this diagram to a lesson"}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div class="mdl__p">
        <header class="mdl__hd">
          <div>
            <span class="mdl__eyebrow">Propose an edit</span>
            <h2 class="mdl__t">{updating ? "Update the diagram" : "Add to a lesson"}</h2>
          </div>
          <button class="pane-hd__btn" aria-label="Close" onClick={onClose}>
            <Icon name="x" size={16} />
          </button>
        </header>

        {phase.kind === "denied" && <Denied reason={phase.reason} />}

        {phase.kind === "error" && (
          <>
            <p class="mdl__bad">{phase.message}</p>
            <footer class="mdl__ft">
              <button class="mdl__ghost" onClick={onClose}>
                Cancel
              </button>
              <button class="lab-primary" onClick={() => setPhase({ kind: "picking" })}>
                Try again
              </button>
            </footer>
          </>
        )}

        {phase.kind === "picking" && (
          <>
            <div class="mdl__find">
              <span class="mdl__findic">
                <Icon name="search" size={15} />
              </span>
              <input
                autoFocus
                placeholder="Search lessons…"
                value={query}
                onInput={(event) => setQuery((event.currentTarget as HTMLInputElement).value)}
              />
            </div>
            <ul class="mdl__list">
              {results.map((entry) => (
                <li key={entryKey(entry)}>
                  <button class="mdl__row" onClick={() => void pick(entry)}>
                    <span class="mdl__rowic">
                      <Icon name="book" size={15} />
                    </span>
                    <span class="mdl__rowt">
                      {entry.label}
                      <em>{entry.sublabel}</em>
                    </span>
                  </button>
                </li>
              ))}
              {results.length === 0 && <li class="mdl__empty">No lesson matches that.</li>}
            </ul>
          </>
        )}

        {phase.kind === "placing" && (
          <>
            {/* Only the lesson and the repo: the change writes a fence and nothing else. Every
                figure is drawn on demand and content-addressed, so there is no artifact path to
                promise here. */}
            <p class="mdl__where">
              {updating ? "Replacing the diagram in " : "Adding to "}
              <code>{phase.lessonPath}</code>
              {" in "}
              <code>{phase.source.repo}</code>
            </p>
            {!updating && (
              <label class="mdl__field">
                <span>Put it</span>
                <select
                  value={anchor}
                  onChange={(event) => setAnchor((event.currentTarget as HTMLSelectElement).value)}
                >
                  <option value="">At the end</option>
                  {headingsOf(phase.source.source).map((heading) => (
                    <option key={heading} value={heading}>{`Before “${heading}”`}</option>
                  ))}
                </select>
              </label>
            )}
            <label class="mdl__field">
              <span>Summary</span>
              <input
                value={summary}
                placeholder="Why this diagram helps"
                onInput={(event) => setSummary((event.currentTarget as HTMLInputElement).value)}
              />
            </label>
            <footer class="mdl__ft">
              <button class="mdl__ghost" onClick={onClose}>
                Cancel
              </button>
              <button class="lab-primary" onClick={() => void propose()}>
                <Icon name="check" size={15} />
                {updating ? "Propose the update" : "Propose edit"}
              </button>
            </footer>
          </>
        )}

        {phase.kind === "sending" && <p class="mdl__where">Opening the change request…</p>}

        {phase.kind === "done" && (
          <>
            <p class="mdl__where">
              {phase.request.reused
                ? "Added as another commit on the change request you already have open for this lesson."
                : "Change request opened."}
            </p>
            <footer class="mdl__ft">
              <button class="mdl__ghost" onClick={onClose}>
                Close
              </button>
              {phase.request.prUrl == null ? (
                <span class="mdl__where">{`${phase.request.mode} mode — nothing left the server.`}</span>
              ) : (
                <a
                  class="lab-primary"
                  href={phase.request.prUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  View the pull request
                </a>
              )}
            </footer>
          </>
        )}
      </div>
    </div>
  );
}

function Denied({ reason }: { reason: "anonymous" | "not-allowed" | "off" }) {
  if (reason === "anonymous") {
    return (
      <div class="edit-gate">
        <p class="edit-gate__body">Sign in to propose a change.</p>
        <button class="edit-gate__btn" onClick={() => void signIn()}>
          Sign in
        </button>
      </div>
    );
  }
  if (reason === "off") {
    return <p class="edit-gate__body">Editing is not enabled on this deployment.</p>;
  }
  // The same sentence the lesson page and /edit give, so a reader who has met one of those gates
  // recognises this as the same answer rather than a different rule.
  return (
    <div class="edit-gate">
      <p class="edit-gate__body">{EDIT_ACCESS_TEXT}</p>
      <a class="edit-gate__mail" href={`mailto:${CONTACT_EMAIL}`}>
        Email {CONTACT_EMAIL}
      </a>
    </div>
  );
}

// ── SPLICING ─────────────────────────────────────────────────────────────────

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

const entryKey = (entry: SearchEntry): string => `${entry.label}|${entry.sublabel}`;

/**
 * The file split at its frontmatter fence, keeping the fence's text BYTE-FOR-BYTE.
 *
 * `lib/markdown/frontmatter` parses the fence into fields and throws the raw text away, which is
 * right for rendering and wrong here: this file is going back to the server as a commit, and the
 * server rejects a proposal that lost its frontmatter. So the split is done on the raw string and
 * nothing between the `---` markers is ever rewritten.
 */
export function splitHead(file: string): [head: string, body: string] {
  const lines = file.split("\n");
  if (lines[0]?.trimEnd() !== "---") return ["", file];
  const end = lines.findIndex((line, i) => i >= 1 && line.trimEnd() === "---");
  if (end === -1) return ["", file];
  // The head keeps the newline that ends its closing `---`, so `head + body` is the file again.
  // Without it the two halves join one line short and every proposal carries an unrelated
  // whitespace change into its diff.
  return [`${lines.slice(0, end + 1).join("\n")}\n`, lines.slice(end + 1).join("\n")];
}

const HEADING = /^#{2,3} \S/;
const titleOfHeading = (line: string): string => line.replace(/^#+\s*/, "").trim();

/** The `##`/`###` headings a fence can be placed before. */
export function headingsOf(file: string): string[] {
  const [, body] = splitHead(file);
  return body.split("\n").filter((line) => HEADING.test(line)).map(titleOfHeading);
}

/**
 * The lesson's file with `fence` inserted — before `anchor`'s heading, or at the end.
 *
 * Blank lines around the fence are what keep it a block in the markdown rather than glued to the
 * paragraph it lands beside. An anchor that no longer exists falls back to the end rather than
 * failing: the lesson may have changed since the picker read it.
 */
/** What went wrong when a diagram could not be put back where it came from. */
export class FenceMoved extends Error {}

/**
 * The lesson's file with the `lang` fence at `at` (and the `count - 1` after it) replaced by
 * `fence`.
 *
 * `at` indexes the fences of ONE language, so the mermaid diagram at position 2 is found without
 * counting the d2 fences between it and position 1 — the same ordinal the figure's `data-fence-at`
 * carries.
 *
 * `expect` is the source that was loaded into the editor. It is checked against what is actually
 * at that position now, and a mismatch throws rather than writing: `baseFingerprint` would also
 * catch a file that moved, but only after the whole splice is built and sent, and its message
 * says the page changed rather than which diagram this would have overwritten. Someone editing a
 * diagram should never silently replace a different one.
 */
export function replaceFence(
  file: string,
  lang: DiagramLang,
  at: number,
  count: number,
  fence: string,
  expect: string,
): string {
  const [head, body] = splitHead(file);
  const found = fencesOfLang(body, lang);
  const target = found[at];
  if (target == null) {
    throw new FenceMoved(`This lesson no longer has a diagram at position ${at + 1}.`);
  }
  if (target.source !== expect) {
    throw new FenceMoved(
      "This diagram has changed in the lesson since it was opened. Reload the page and edit it again.",
    );
  }
  // A run of adjacent fences is one figure, so editing it replaces the whole run.
  const last = found[Math.min(at + count, found.length) - 1] ?? target;
  // Cut on the BLOCK's offsets, not the source's: the info string and the backtick runs go with
  // it, which is where a walkthrough's `boards name=…` lives.
  return `${head}${body.slice(0, target.start)}${fence.replace(/\n+$/, "")}${body.slice(last.end)}`;
}

export function insertFence(file: string, fence: string, anchor: string): string {
  const [head, body] = splitHead(file);
  const block = fence.replace(/\n+$/, "");
  const atEnd = () => `${head}${body.replace(/\s*$/, "")}\n\n${block}\n`;
  if (anchor === "") return atEnd();

  const lines = body.split("\n");
  const at = lines.findIndex((line) => HEADING.test(line) && titleOfHeading(line) === anchor);
  if (at < 0) return atEnd();
  return `${head}${[...lines.slice(0, at), block, "", ...lines.slice(at)].join("\n")}`;
}
