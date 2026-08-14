// ──────────────────────────────────────────────────────────────────
// HOW TO ASK FOR ACCESS
// ──────────────────────────────────────────────────────────────────
// Two capabilities on this deployment are granted rather than assumed: SAVING code attempts, which
// spends shared compute and storage, and EDITING content, which opens pull requests against the
// content repositories. They are separate grants, and a reader can hold one without the other.
//
// The sentence is the same shape for both — what the grant is, how to ask, and the honest caveat —
// because a reader who meets one gate and later meets the other should recognise the second as the
// same kind of answer rather than reading it as a different rule. Spelled once here, so a fourth
// gate cannot arrive with a fifth wording.
//
// The server states the submit gate's version too, in `server/src/submission/http/dto.rs`: a 403
// travels to API consumers who never load this bundle. That copy is a deliberate second one, held
// honest by the assertion in `submission/http/dto_tests.rs`.

/** Where access requests go. */
export const CONTACT_EMAIL = "synapse.kakde.eu@gmail.com";

/** The closing clause both gates share — the promise is a reply, not a grant. */
const CAVEAT = "access may or may not be granted. Thanks for understanding.";

/**
 * The full sentence for a gate, as one string.
 *
 * `what` names the grant and reads on from "…". Keep it a noun phrase with its own verb, so the
 * result is one sentence rather than two glued together.
 */
export function accessRequest(what: string): string {
  return `${what} To request one, email your GitHub username to ${CONTACT_EMAIL} — ${CAVEAT}`;
}

/** Editing prose and diagrams: the content-editor list. */
export const EDIT_ACCESS_TEXT = accessRequest("Editing needs a place on the content-editor list.");

/** Saving code attempts: the submit list. Mirrors the server's 403 hint. */
export const SUBMIT_ACCESS_TEXT = accessRequest(
  "Saving attempts needs a place on the submit list, because it uses shared compute and storage.",
);
