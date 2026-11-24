/**
 * The code labs' door — `/code` and `/viz` open only for a signed-in account that passes the
 * submit gate (`GET /api/submissions/access`).
 *
 * The server refuses an anonymous run on its own, so this is not what keeps a stranger out of the
 * sandbox. It is what keeps the lab from offering a stranger an editor, a Run button and a 5 MB
 * wasm bundle that can only answer "sign in". Nothing behind the gate is mounted until it opens:
 * the children render only in the `open` state, so the wasm and Monaco are never fetched for a
 * caller who cannot use them.
 *
 * It follows the session live (`auth/store.subscribe`): signing in on another tab, or the header
 * chip finishing its silent check-sso after first paint, re-asks the question.
 */
import type { ComponentChildren } from "preact";
import { useEffect, useState } from "preact/hooks";

import { submitAccess } from "../../lib/api/client";
import * as log from "../../lib/log";
import { getState, signIn, subscribe } from "../auth/store";

export type GateState =
  | { readonly kind: "checking" }
  | { readonly kind: "anonymous" }
  | { readonly kind: "refused"; readonly username: string }
  | { readonly kind: "failed"; readonly reason: string }
  | { readonly kind: "open" };

interface Props {
  /** What the lab is called on the wall, e.g. "The code lab". */
  title: string;
  /** One sentence on what the lab does, under the title. */
  blurb: string;
  children: ComponentChildren;
}

export function LabGate({ title, blurb, children }: Props) {
  const [gate, setGate] = useState<GateState>({ kind: "checking" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let current = 0;
    const evaluate = () => {
      const auth = getState();
      const ticket = ++current;
      if (auth.kind === "loading") {
        setGate({ kind: "checking" });
        return;
      }
      if (auth.kind === "anonymous") {
        log.info("lab gate: anonymous");
        setGate({ kind: "anonymous" });
        return;
      }
      const username = auth.me.username;
      setGate({ kind: "checking" });
      submitAccess()
        .then(({ canSubmit }) => {
          // A later evaluation (a sign-out mid-request) owns the state now.
          if (ticket !== current) return;
          log.info(`lab gate: ${canSubmit ? "open" : "refused"} for ${username}`);
          setGate(canSubmit ? { kind: "open" } : { kind: "refused", username });
        })
        .catch((error: unknown) => {
          if (ticket !== current) return;
          const reason = error instanceof Error ? error.message : String(error);
          log.warn(`lab gate: access check failed: ${reason}`);
          setGate({ kind: "failed", reason });
        });
    };
    // Subscribe first, then read: the store is notify-only, so a flip between the two would
    // otherwise be missed.
    const unsubscribe = subscribe(evaluate);
    evaluate();
    return () => {
      current = -1;
      unsubscribe();
    };
  }, [attempt]);

  if (gate.kind === "open") return <>{children}</>;
  return (
    <div class="lab-gate" data-lab-gate={gate.kind}>
      <div class="lab-gate__card">
        <span class="pane-hd__eyebrow">{title}</span>
        <h1 class="lab-gate__title">{headline(gate)}</h1>
        <p class="lab-gate__blurb">{blurb}</p>
        <p class="lab-gate__why">{explanation(gate)}</p>
        {gate.kind === "anonymous" && (
          <button type="button" class="lab-primary" onClick={() => signIn()}>
            Sign in
          </button>
        )}
        {gate.kind === "failed" && (
          <button type="button" class="lab-primary" onClick={() => setAttempt((n) => n + 1)}>
            Try again
          </button>
        )}
        {gate.kind === "checking" && <span class="st st--busy"><span class="st__dot"></span>Checking access</span>}
      </div>
    </div>
  );
}

function headline(gate: GateState): string {
  switch (gate.kind) {
    case "anonymous":
      return "Sign in to use the lab";
    case "refused":
      return "This account has no lab access";
    case "failed":
      return "Could not check your access";
    default:
      return "One moment";
  }
}

function explanation(gate: GateState): string {
  switch (gate.kind) {
    case "anonymous":
      return "The lab runs real programs in a shared sandbox, so it needs an account.";
    case "refused":
      return `Signed in as ${gate.username}, which is not on this site's access list. Ask the operator to add it.`;
    case "failed":
      return gate.reason;
    default:
      return "Checking whether this account can run code here.";
  }
}
