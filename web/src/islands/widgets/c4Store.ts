// The one piece of state C4Embed.tsx (many possible embeds, click-to-select) and C4DocsPanel.tsx
// (one panel, page-wide) share (oracle: reader.rs's `c4_selected` RwSignal, threaded through both
// `hydrate_c4_embeds` and `<C4DocsPanel>`). A module-level singleton store — not a signal, not
// context — because these are two independently-mounted Preact trees with no ancestor in common.
import { Store } from "../../lib/store";

export const c4Selected = new Store<string | null>(null);
