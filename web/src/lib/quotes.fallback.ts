/**
 * The header quote's floor — what renders when the feed is unreachable, when it answers something
 * unparseable, and on every render before the first fetch of a slot lands.
 *
 * It exists so the header NEVER shows an empty middle and never blocks on the network: the reader
 * gets a real quote either way, and only the source differs. It is also what the e2e suite asserts
 * against, because `dev-tools/e2e` pins `SYNAPSE_QUOTE_FEED_URL=off` — a suite that reached
 * BrainyQuote would be a suite whose result depends on a third party's uptime.
 *
 * Every entry is short enough to read on one header line and durably attributed; none carries a
 * `href`, because there is nothing external to credit for these.
 */

import type { Quote } from "./quote";

export const FALLBACK_QUOTES: readonly Quote[] = [
  { text: "The only way to learn mathematics is to do mathematics.", author: "Paul Halmos" },
  { text: "What we have to learn to do, we learn by doing.", author: "Aristotle" },
  { text: "It always seems impossible until it's done.", author: "Nelson Mandela" },
  { text: "Simplicity is prerequisite for reliability.", author: "Edsger W. Dijkstra" },
  { text: "Nothing in life is to be feared, it is only to be understood.", author: "Marie Curie" },
  {
    text: "Perfection is achieved not when there is nothing more to add, but when there is nothing left to take away.",
    author: "Antoine de Saint-Exupéry",
  },
  { text: "Success is the sum of small efforts, repeated day in and day out.", author: "Robert Collier" },
  { text: "It does not matter how slowly you go as long as you do not stop.", author: "Confucius" },
];
