// The one piece of state C4Embed.tsx (many possible embeds, click-to-select) and C4DocsPanel.tsx
// (one panel, page-wide) share. A module-level singleton store — not a signal, not context —
// because these are two independently-mounted Preact trees with no ancestor in common.
import { Store } from "../../lib/store";

export const c4Selected = new Store<string | null>(null);
