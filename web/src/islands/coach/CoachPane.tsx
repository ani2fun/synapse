/**
 * The Coach pane (port of client/src/tutoring/mod.rs — oracle: `CoachPane`, step 20 /
 * ADR-S025): a flat thin feature, the problem page's 4th tab. The transcript is EPHEMERAL (page
 * state, gone on navigation); config is fetched on mount and ANY failure falls to Off — never a
 * chat box that 404s. Off copy: "The coach is off / This feature is coming soon."
 *
 * `code_ctx` mirrored the workbench editor's `(source, language)` in the Rust, read at SEND time
 * only (an untracked read, never a subscription). Islands cannot share signals (A06's rule), so
 * this listens for the workbench root's bubbling `synapse:code-changed` and keeps the latest
 * snapshot in a ref — read only when `send()` fires, the same "snapshot, not a subscription"
 * semantics.
 */
import { useEffect, useRef, useState } from "preact/hooks";

import { tutorChat, tutorConfig } from "../../lib/api/client";
import type { ChatMessage } from "../../lib/api/client";
import * as log from "../../lib/log";
import { CODE_CHANGED } from "../workbench/contracts";
import type { CodeSnapshot } from "../workbench/contracts";

type ConfigState = { kind: "loading" } | { kind: "off" } | { kind: "on"; model: string };
type SendState = { kind: "idle" } | { kind: "sending" } | { kind: "failed"; message: string };

export interface CoachPaneProps {
  problem: string;
  /** The right pane's workbench root — its event target for `CODE_CHANGED`. */
  workbenchRoot: () => HTMLElement | null;
}

export function CoachPane({ problem, workbenchRoot }: CoachPaneProps) {
  const [cfg, setCfg] = useState<ConfigState>({ kind: "loading" });
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [sendState, setSendState] = useState<SendState>({ kind: "idle" });
  const codeCtx = useRef<CodeSnapshot>({ source: "", language: "" });
  const messagesRef = useRef<ChatMessage[]>(messages);
  messagesRef.current = messages;
  const sendingRef = useRef(false);

  useEffect(() => {
    void (async () => {
      try {
        const config = await tutorConfig();
        setCfg(config.enabled ? { kind: "on", model: config.model } : { kind: "off" });
      } catch {
        setCfg({ kind: "off" });
      }
    })();
  }, []);

  useEffect(() => {
    const root = workbenchRoot();
    if (!root) return;
    const onCodeChanged = (event: Event) => {
      codeCtx.current = (event as CustomEvent<CodeSnapshot>).detail;
    };
    root.addEventListener(CODE_CHANGED, onCodeChanged);
    return () => root.removeEventListener(CODE_CHANGED, onCodeChanged);
  }, [workbenchRoot]);

  const send = (): void => {
    const text = draft.trim();
    if (text === "" || sendingRef.current) return;
    const next = [...messagesRef.current, { role: "user", content: text }];
    setMessages(next);
    setDraft("");
    sendingRef.current = true;
    setSendState({ kind: "sending" });
    const { source, language } = codeCtx.current;
    log.info(`tutor: sending turn (${next.length} message(s))`);
    void (async () => {
      try {
        const reply = await tutorChat({
          problemPath: problem,
          code: source !== "" ? source : null,
          language: language !== "" ? language : null,
          messages: next,
        });
        log.debug("tutor: reply received");
        setMessages((m) => [...m, { role: "assistant", content: reply.content }]);
        setSendState({ kind: "idle" });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        log.error(`tutor: chat failed — ${message}`);
        setSendState({ kind: "failed", message });
      } finally {
        sendingRef.current = false;
      }
    })();
  };

  return (
    <div class="coach not-prose">
      {cfg.kind === "loading" && <p class="coach__checking">Checking the coach…</p>}
      {cfg.kind === "off" && (
        <div class="coach__off">
          <span class="coach__off-title">The coach is off</span>
          <p class="coach__off-note">This feature is coming soon.</p>
        </div>
      )}
      {cfg.kind === "on" && (
        <ChatUi model={cfg.model} messages={messages} draft={draft} setDraft={setDraft} sendState={sendState} send={send} />
      )}
    </div>
  );
}

function ChatUi({
  model,
  messages,
  draft,
  setDraft,
  sendState,
  send,
}: {
  model: string;
  messages: ChatMessage[];
  draft: string;
  setDraft: (value: string) => void;
  sendState: SendState;
  send: () => void;
}) {
  return (
    <div class="coach__chat">
      <div class="coach__head">
        <span class="wb__eyebrow">
          <span class="wb__prompt">{">_"}</span> COACH
        </span>
        <span class="coach__model">{model}</span>
      </div>
      <div class="coach__log">
        {messages.map((message) => (
          <div class={`coach__bubble coach__bubble--${message.role}`}>
            <p>{message.content}</p>
          </div>
        ))}
        {sendState.kind === "sending" && <div class="coach__bubble coach__bubble--assistant coach__typing">…</div>}
        {sendState.kind === "failed" && <div class="coach__error">Couldn't reach the coach — {sendState.message}</div>}
      </div>
      <div class="coach__composer">
        <textarea
          class="coach__input"
          placeholder="Ask for a hint…"
          rows={2}
          value={draft}
          onInput={(event) => setDraft((event.target as HTMLTextAreaElement).value)}
          onKeyDown={(event) => {
            // Enter sends; Shift+Enter stays a newline.
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              send();
            }
          }}
        ></textarea>
        <button class="wb__submit" disabled={sendState.kind === "sending"} onClick={send}>
          Send
        </button>
      </div>
    </div>
  );
}
