// ──────────────────────────────────────────────────────────────────
// LISTEN, LIVE — for a reader on the listen list
// ──────────────────────────────────────────────────────────────────
// Loaded only when the server says this reader may listen (see `./index`). Two ways in:
//
//   · the lesson header's Listen pill (server-rendered gated; made live here) — the whole lesson;
//   · select some prose and the popover offers "Listen" (just the selection) and "From here"
//     (from the selection's paragraph to the end of the lesson).
//
// Whatever is playing, the sentence being spoken is highlighted and kept on screen (`./follow`).

import type { ListenConfig } from "../../lib/api/client";
import * as log from "../../lib/log";
import type { ProseBlock } from "./extract";
import { blockAt, proseFrom, proseInRange } from "./extract";
import type { Follower } from "./follow";
import { follow } from "./follow";
import { createPlayer } from "./player";
import { mountPopover } from "./popover";
import { unitSpans } from "./units";

const LIVE_TIP = "Listen to this lesson — select any text to start from there";

export function mount(config: ListenConfig): () => void {
  const body = document.querySelector<HTMLElement>("article.lesson-body");
  if (!body) return () => undefined;
  const player = createPlayer(config);
  const title = document.querySelector("h1")?.textContent?.trim() ?? document.title;

  let follower: Follower | null = null;
  const listen = (blocks: ProseBlock[]): void => {
    const spans = unitSpans(
      blocks.map((b) => b.text),
      config.maxUnitChars,
    );
    follower?.dispose();
    const current = follow(blocks, spans, (away) => player.setAway(away));
    follower = current;
    void player.play(
      spans.map((s) => s.text),
      title,
      (index) => current.show(index),
      () => current.jump(),
    );
  };

  // ── the header pill: the whole lesson ──
  const header = document.querySelector<HTMLButtonElement>("[data-listen-header]");
  const onHeader = (): void => {
    const first = body.firstElementChild;
    if (first) listen(proseFrom(body, first));
  };
  if (header) {
    header.classList.remove("listen-header--gated");
    header.removeAttribute("aria-disabled");
    header.addEventListener("click", onHeader);
    document.querySelector("[data-listen-tip]")?.setAttribute("data-tip", LIVE_TIP);
  }

  const unmountPopover = mountPopover(body, {
    listen: (range) => listen(proseInRange(body, range)),
    fromHere: (range) => {
      const start = blockAt(body, range.startContainer);
      if (start) listen(proseFrom(body, start));
    },
  });

  log.info("listen: live");
  return () => {
    header?.removeEventListener("click", onHeader);
    unmountPopover();
  };
}
