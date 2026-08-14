/**
 * "Edit" on a rendered d2 figure — the diagram half of "Suggest an edit".
 *
 * A lesson's prose has been editable in place for a while; its diagrams have not. This opens
 * `/d2` loaded with the exact fence behind the figure, where it can be changed against a live
 * preview and proposed as a pull request that replaces it where it stands.
 *
 * The figure carries `data-fence-at` — which d2 fence in the lesson it came from — because a
 * server-drawn block ships no source at all (nothing may recompile it) and d2 hashes its salt
 * into its own element ids, so the ordinal is the only thing tying a picture back to its text.
 *
 * Gating follows `editLink.ts` exactly: visible but inert by default, lit when the server says
 * this reader may edit, removed entirely when the deployment does not offer editing. A reader who
 * cannot use it still learns it exists and how to ask — and that default is the safe one, because
 * it is what they get if this never runs.
 */
import { h } from "preact";
import { useEffect, useState } from "preact/hooks";

import { EDIT_ACCESS_TEXT } from "../../lib/contact";
import { Icon } from "../d2lab/icons";
import { lessonPathFromUrl } from "../../lib/catalog/path";
import { onEditGate } from "../../lib/api/editGate";

const ACTIVE_TIP = "Edit this diagram and open a change request";

/** Where `/d2` should open for a given figure. `count` travels so a slideshow run is edited as
 *  the group it is, rather than one slide of it. */
export function editorHref(lessonPath: string, at: number, count: number): string {
  const params = new URLSearchParams({ lesson: lessonPath, at: String(at) });
  if (count > 1) params.set("count", String(count));
  return `/d2?${params.toString()}`;
}

export function DiagramEdit({ at, count }: { at: number; count: number }) {
  const [gate, setGate] = useState<{ enabled: boolean; canEdit: boolean } | null>(null);
  useEffect(() => onEditGate(setGate), []);

  // Absent until the answer arrives, and gone for good when editing is off — an affordance nobody
  // can ever earn is worse than none.
  if (gate == null || !gate.enabled) return null;

  const lessonPath = lessonPathFromUrl().join("/");
  if (lessonPath === "") return null; // not on a lesson: nothing to edit this against

  if (!gate.canEdit) {
    return (
      <span class="diagram__edit diagram__edit--gated modal-btn" role="note" title={EDIT_ACCESS_TEXT}>
        <Icon name="pencil" size={14} />
        <span>Edit</span>
      </span>
    );
  }
  return (
    <a class="diagram__edit modal-btn" href={editorHref(lessonPath, at, count)} title={ACTIVE_TIP}>
      <Icon name="pencil" size={14} />
      <span>Edit</span>
    </a>
  );
}
