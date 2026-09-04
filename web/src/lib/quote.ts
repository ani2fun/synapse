/**
 * The header's twice-daily quote: one at 06:00, a different one at 18:00, every day.
 *
 * SYNCHRONOUS BY CONSTRUCTION. `currentQuote()` is called while the header renders, on every page
 * of the site, so it must never await anything — a slow feed would otherwise become a slow site.
 * It answers from memory and, when that answer is stale, fires a refresh it does not wait for: the
 * render in hand serves the bundled fallback (or what the previous fetch left) and the NEXT render
 * gets the fresh one. Nothing here can make a page slower than a property read.
 *
 * THE FEED IS BRAINYQUOTE'S, and the choice is load-bearing. `quotebr.rss` publishes FOUR items a
 * day, each with a date-stamped `guid` (`…/quote-20260904-0` … `-3`) and a 24-hour `ttl` — so
 * morning and evening come from ONE fetch, at different indices, and the whole set turns over
 * tomorrow. A one-item "quote of the day" feed could not answer the requirement at all.
 *
 * THE CACHE IS KEYED BY SLOT, NOT BY AGE, and that is what keeps the quote still. BrainyQuote
 * rebuilds at 05:00 GMT — 07:00 in Berlin over summer — so a time-to-live cache would swap the
 * quote out from under a reader an hour into their morning. Keyed by slot, the 06:00 quote is
 * fetched once and holds until 18:00.
 *
 * `<title>` is the AUTHOR and `<description>` is the QUOTE, the opposite of what the tag names
 * suggest, and the description arrives wrapped in straight double quotes.
 */

import { fnv1a } from "./hash";
import * as log from "./log";
import { FALLBACK_QUOTES } from "./quotes.fallback";

/** One quote as the header renders it. `href` credits the feed's source; the fallback pool has none. */
export interface Quote {
  text: string;
  author: string;
  href?: string;
}

/** Which half of the day a quote belongs to. */
export type Slot = "am" | "pm";

const DEFAULT_FEED = "https://www.brainyquote.com/link/quotebr.rss";
const DEFAULT_TZ = "Europe/Berlin";
/** The two turnover hours, in the site's timezone. */
const MORNING_HOUR = 6;
const EVENING_HOUR = 18;
/** Hard ceiling on a render-blocking-adjacent fetch. The refresh is not awaited, but a socket held
 *  open forever would keep `inFlight` set and block every later attempt. */
const FETCH_TIMEOUT_MS = 4_000;
/** How long a failed feed is left alone. Without it every page render would retry a dead host. */
const RETRY_AFTER_MS = 10 * 60 * 1_000;
/** Longer than this is not a header line, it is an ellipsis. Such items are dropped, not truncated
 *  — a cut-off quote misattributes half a thought to its author. */
const MAX_TEXT = 220;

const USER_AGENT = "synapse.kakde.eu (+https://synapse.kakde.eu)";

// ── configuration ────────────────────────────────────────────────────────────────────────────
// `import.meta.env` first (Vite inlines it at build time), `process.env` second (the standalone
// Node server reads its environment at runtime) — the same belt-and-braces read `api/client.ts`
// uses for `SYNAPSE_API_URL`, and for the same reason.

function fromEnv(viteValue: string | undefined, name: string): string | undefined {
  if (viteValue !== undefined) return viteValue;
  return typeof process !== "undefined" ? process.env[name] : undefined;
}

/**
 * The feed to read, or `null` for "do not touch the network". THE ADDRESS IS THE SWITCH:
 * `SYNAPSE_QUOTE_FEED_URL=off` (or empty) pins the bundled pool, which is how the e2e suite stays
 * offline and deterministic and how a live instance can drop the dependency without a redeploy.
 */
export function feedUrl(): string | null {
  const raw = (fromEnv(import.meta.env.SYNAPSE_QUOTE_FEED_URL, "SYNAPSE_QUOTE_FEED_URL") ?? DEFAULT_FEED).trim();
  return raw === "" || raw.toLowerCase() === "off" ? null : raw;
}

/** The timezone whose 06:00 and 18:00 the schedule means. Env: `SYNAPSE_QUOTE_TZ`. */
export function timezone(): string {
  const raw = (fromEnv(import.meta.env.SYNAPSE_QUOTE_TZ, "SYNAPSE_QUOTE_TZ") ?? DEFAULT_TZ).trim();
  return raw === "" ? DEFAULT_TZ : raw;
}

// ── the slot clock ───────────────────────────────────────────────────────────────────────────

/**
 * Which quote is current, as `YYYY-MM-DD:am` / `YYYY-MM-DD:pm` in `tz`.
 *
 * Before 06:00 the answer is the PREVIOUS day's evening, not the current day's: the quote turns
 * over at 06:00, so someone reading at 01:00 is still in last night's slot. Reading the wall clock
 * through `Intl` is what makes this correct across Berlin's DST switches without a tz database of
 * our own; `hourCycle: "h23"` pins midnight to `00` rather than ICU's `24`.
 */
export function slotKey(now: Date, tz: string): string {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: tz,
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
  }).formatToParts(now);
  const at = (type: string) => parts.find((p) => p.type === type)?.value ?? "";
  const day = `${at("year")}-${at("month")}-${at("day")}`;
  const hour = Number(at("hour")) % 24;

  if (hour < MORNING_HOUR) return `${dayBefore(day)}:pm`;
  return hour < EVENING_HOUR ? `${day}:am` : `${day}:pm`;
}

/** Calendar arithmetic on the `YYYY-MM-DD` half only — no clock, so no DST to get wrong. */
function dayBefore(day: string): string {
  const d = new Date(`${day}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() - 1);
  return d.toISOString().slice(0, 10);
}

/** Split a slot key back into its halves. */
export function splitSlotKey(key: string): { day: string; slot: Slot } {
  const [day = "", slot = ""] = key.split(":");
  return { day, slot: slot === "am" ? "am" : "pm" };
}

// ── the feed ─────────────────────────────────────────────────────────────────────────────────

const ITEM = /<item\b[^>]*>([\s\S]*?)<\/item>/gi;

function childText(block: string, name: string): string {
  const match = new RegExp(`<${name}\\b[^>]*>([\\s\\S]*?)</${name}>`, "i").exec(block);
  return match?.[1] ?? "";
}

/**
 * XML text to plain text. CDATA first, then the named entities, then numeric ones, and `&amp;`
 * LAST — decoding it earlier would turn `&amp;lt;` into a literal `<` that was never in the feed.
 */
function decodeXml(raw: string): string {
  return raw
    .replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g, "$1")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&#x([0-9a-f]+);/gi, (_, hex: string) => String.fromCodePoint(Number.parseInt(hex, 16)))
    .replace(/&#(\d+);/g, (_, dec: string) => String.fromCodePoint(Number(dec)))
    .replace(/&amp;/g, "&")
    .replace(/\s+/g, " ")
    .trim();
}

/** Strip the quotation marks the feed wraps every description in — the header draws its own. */
function unquote(text: string): string {
  return text
    .replace(/^["“”«]\s*/, "")
    .replace(/\s*["“”»]$/, "")
    .trim();
}

/**
 * The feed's items, in feed order — the order IS the schedule, so nothing here sorts or filters by
 * anything but usability. Never throws: a truncated or empty body is an empty list, which the
 * caller already handles as "serve the fallback".
 */
export function parseFeed(xml: string): Quote[] {
  const items: Quote[] = [];
  for (const match of xml.matchAll(ITEM)) {
    const block = match[1] ?? "";
    const author = decodeXml(childText(block, "title"));
    const text = unquote(decodeXml(childText(block, "description")));
    if (!author || !text || text.length > MAX_TEXT) continue;
    const href = decodeXml(childText(block, "link"));
    items.push(href.startsWith("http") ? { text, author, href } : { text, author });
  }
  return items;
}

/**
 * The item this slot shows. Morning is the first; evening is THE FIRST ONE THAT READS DIFFERENTLY,
 * walking past index 1 if it has to. Indices alone would satisfy the requirement only for as long
 * as the feed keeps shipping four distinct items — and "a different quote in the evening" is the
 * requirement, not "the second element".
 */
export function pickForSlot(items: readonly Quote[], slot: Slot): Quote | undefined {
  const first = items[0];
  if (!first || slot === "am") return first;
  return items.slice(1).find((q) => q.text !== first.text) ?? first;
}

/**
 * The bundled quote for a slot. Morning is drawn from the day; evening is offset by a non-zero
 * amount, so the two can never land on the same entry — the same guarantee `pickForSlot` gives the
 * live feed, kept for the path that does not have one.
 */
export function fallbackFor(key: string): Quote {
  const pool = FALLBACK_QUOTES;
  const { day, slot } = splitSlotKey(key);
  const morning = index(day, pool.length);
  const at = slot === "am" ? morning : (morning + 1 + index(`${day}:pm`, pool.length - 1)) % pool.length;
  // The pool is non-empty (it is a literal in this repo), so this index always resolves.
  return pool[at] ?? pool[0]!;
}

function index(seed: string, mod: number): number {
  return Number.parseInt(fnv1a(seed), 16) % mod;
}

// ── the live cache ───────────────────────────────────────────────────────────────────────────

interface CachedFeed {
  /** The day this was fetched FOR, not the day the feed claims — it answers "do we have today?". */
  day: string;
  items: Quote[];
}

let cached: CachedFeed | null = null;
let inFlight: Promise<void> | null = null;
let failedAt = 0;

/**
 * The quote to render, now. Never throws, never blocks, never reaches the network on the caller's
 * time. A slot change WITHIN a cached day costs nothing at all — the other item is already here,
 * which is the whole reason the parsed feed is cached rather than the chosen quote.
 */
export function currentQuote(now: Date = new Date()): Quote {
  const key = slotKey(now, timezone());
  const { day, slot } = splitSlotKey(key);

  if (cached?.day === day) {
    const picked = pickForSlot(cached.items, slot);
    if (picked) {
      log.debug(`quote: ${slot} of ${day} served from the cached feed`);
      return picked;
    }
  }

  kickRefresh(day);
  return fallbackFor(key);
}

/** Start a refresh unless one is already running, the feed is switched off, or it just failed. */
function kickRefresh(day: string): void {
  const url = feedUrl();
  if (!url) return;
  if (inFlight) return;
  if (failedAt !== 0 && Date.now() - failedAt < RETRY_AFTER_MS) return;
  inFlight = refresh(url, day).finally(() => {
    inFlight = null;
  });
}

async function refresh(url: string, day: string): Promise<void> {
  try {
    const response = await fetch(url, {
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
      headers: {
        accept: "application/rss+xml, application/xml;q=0.9, */*;q=0.8",
        "user-agent": USER_AGENT,
      },
    });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const items = parseFeed(await response.text());
    if (items.length === 0) throw new Error("no usable items");
    cached = { day, items };
    failedAt = 0;
    log.info(`quote: ${items.length} item(s) fetched for ${day}`);
  } catch (error) {
    failedAt = Date.now();
    const reason = error instanceof Error ? error.message : String(error);
    log.warn(`quote: feed unreachable (${reason}) — serving the bundled pool`);
  }
}

/** Drop every cached feed and failure. Exists for tests, which must not inherit each other's state. */
export function resetQuoteCache(): void {
  cached = null;
  inFlight = null;
  failedAt = 0;
}
