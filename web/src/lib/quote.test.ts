import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { afterEach, describe, expect, it, vi } from "vitest";

import {
  currentQuote,
  fallbackFor,
  feedUrl,
  parseFeed,
  pickForSlot,
  resetQuoteCache,
  slotKey,
  timezone,
  type Quote,
} from "./quote";
import { FALLBACK_QUOTES } from "./quotes.fallback";

// ─────────────────────────────────────────────────────────────────────────────
// THE HEADER QUOTE
// Three properties carry the feature, and each is a way it could quietly break:
// the slot turns over at 06:00 and 18:00 in the site's timezone (DST included),
// the parser survives what the publishers actually send, and MORNING AND EVENING
// ARE NEVER THE SAME LINE — on the live feed and on the bundled pool alike.
// ─────────────────────────────────────────────────────────────────────────────

const fixture = (name: string) =>
  readFileSync(fileURLToPath(new URL(`./quote.fixtures/${name}`, import.meta.url)), "utf8");

const BERLIN = "Europe/Berlin";

afterEach(() => {
  resetQuoteCache();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

// ── the slot clock ───────────────────────────────────────────────────────────

describe("slotKey", () => {
  // Instants in UTC, annotated with the Berlin wall clock they land on. Berlin is UTC+2 in
  // September and March-after-the-switch, UTC+1 from late October — which is exactly why the
  // 06:00 boundary sits at a DIFFERENT instant either side of a DST change.
  it.each([
    ["2026-09-04T03:59:00Z", "05:59 CEST", "2026-09-03:pm"],
    ["2026-09-04T04:00:00Z", "06:00 CEST", "2026-09-04:am"],
    ["2026-09-04T15:59:00Z", "17:59 CEST", "2026-09-04:am"],
    ["2026-09-04T16:00:00Z", "18:00 CEST", "2026-09-04:pm"],
    ["2026-09-03T22:30:00Z", "00:30 CEST", "2026-09-03:pm"],
    ["2026-03-29T03:59:00Z", "05:59 CEST, DST forward", "2026-03-28:pm"],
    ["2026-03-29T04:00:00Z", "06:00 CEST, DST forward", "2026-03-29:am"],
    ["2026-10-25T04:59:00Z", "05:59 CET, DST back", "2026-10-24:pm"],
    ["2026-10-25T05:00:00Z", "06:00 CET, DST back", "2026-10-25:am"],
    ["2026-09-01T03:00:00Z", "05:00 CEST, month boundary", "2026-08-31:pm"],
  ])("%s (%s) is %s", (iso, _wall, expected) => {
    expect(slotKey(new Date(iso), BERLIN)).toBe(expected);
  });

  it("holds the evening quote through the small hours rather than turning over at midnight", () => {
    const evening = slotKey(new Date("2026-09-04T16:00:00Z"), BERLIN); // 18:00
    const midnight = slotKey(new Date("2026-09-04T22:00:00Z"), BERLIN); // 00:00 next day
    const beforeSix = slotKey(new Date("2026-09-05T03:59:00Z"), BERLIN); // 05:59 next day
    expect(midnight).toBe(evening);
    expect(beforeSix).toBe(evening);
  });

  it("reads the requested zone, not the machine's", () => {
    const instant = new Date("2026-09-04T20:00:00Z"); // 22:00 Berlin, 13:00 Los Angeles
    expect(slotKey(instant, BERLIN)).toBe("2026-09-04:pm");
    expect(slotKey(instant, "America/Los_Angeles")).toBe("2026-09-04:am");
  });
});

// ── the parser ───────────────────────────────────────────────────────────────

describe("parseFeed", () => {
  it("reads the four items BrainyQuote actually serves", () => {
    const items = parseFeed(fixture("brainyquote.rss"));
    expect(items).toHaveLength(4);
    // `<title>` is the AUTHOR and `<description>` the QUOTE — the opposite of the tag names.
    expect(items[0]).toEqual({
      text: "To err is human; to forgive, divine.",
      author: "Alexander Pope",
      href: "https://www.brainyquote.com/authors/alexander-pope-quotes",
    });
    expect(items[1]?.author).toBe("Jimmy Dean");
    expect(items[3]?.author).toBe("Helen Keller");
    // The channel's own <title>/<link>/<description> sit outside every <item> and must not leak in.
    expect(items.map((q) => q.author)).not.toContain("Today's Quote");
  });

  it("strips the quotation marks the feed wraps every description in", () => {
    for (const item of parseFeed(fixture("brainyquote.rss"))) {
      expect(item.text.startsWith('"')).toBe(false);
      expect(item.text.endsWith('"')).toBe(false);
    }
  });

  it("parses an indented, multi-line feed body the same way", () => {
    const items = parseFeed(fixture("azquotes.rss"));
    expect(items).toHaveLength(4);
    expect(items[0]).toEqual({
      text: "I like the night. Without the dark, we'd never see the stars.",
      author: "Stephenie Meyer",
      href: "http://www.azquotes.com/quote/403630",
    });
  });

  it("unwraps CDATA and decodes entities, leaving &amp; for last", () => {
    const items = parseFeed(`<rss><channel>
      <item><title><![CDATA[Ada Lovelace]]></title>
            <description>&quot;Bits &amp; pieces &#8212; &amp;lt;not markup&amp;gt; &#x2014; add up.&quot;</description>
            <link>https://example.test/ada</link></item>
    </channel></rss>`);
    expect(items[0]).toEqual({
      text: "Bits & pieces — &lt;not markup&gt; — add up.",
      author: "Ada Lovelace",
      href: "https://example.test/ada",
    });
  });

  it("omits the href when the item carries no usable link", () => {
    const items = parseFeed("<item><title>Anon</title><description>A line.</description></item>");
    expect(items[0]).toEqual({ text: "A line.", author: "Anon" });
  });

  it.each([
    ["an empty body", ""],
    ["a feed with no items", "<rss><channel><title>Empty</title></channel></rss>"],
    ["a body truncated mid-item", '<rss><channel><item><title>Cut</title><descrip'],
    ["an item missing its author", "<item><description>Orphaned.</description></item>"],
    ["an item missing its text", "<item><title>Nobody</title></item>"],
    ["not XML at all", "<html><body>502 Bad Gateway</body></html>"],
  ])("returns an empty list for %s rather than throwing", (_label, body) => {
    expect(parseFeed(body)).toEqual([]);
  });

  it("drops an item too long to be a header line instead of truncating it", () => {
    const essay = "word ".repeat(60).trim(); // ~300 characters
    const items = parseFeed(
      `<item><title>Long</title><description>${essay}</description></item>` +
        "<item><title>Short</title><description>Brief.</description></item>",
    );
    expect(items).toEqual([{ text: "Brief.", author: "Short" }]);
  });
});

// ── morning is never evening ─────────────────────────────────────────────────

const quotes = (...texts: string[]): Quote[] => texts.map((text, i) => ({ text, author: `A${i}` }));

describe("pickForSlot", () => {
  it("takes the first item in the morning and the second in the evening", () => {
    const items = quotes("one", "two", "three", "four");
    expect(pickForSlot(items, "am")?.text).toBe("one");
    expect(pickForSlot(items, "pm")?.text).toBe("two");
  });

  it("walks past a repeated item so the evening is never the morning again", () => {
    const items = quotes("one", "one", "three", "four");
    expect(pickForSlot(items, "am")?.text).toBe("one");
    expect(pickForSlot(items, "pm")?.text).toBe("three");
  });

  it("falls back to the only quote there is when the feed serves one distinct item", () => {
    expect(pickForSlot(quotes("only"), "pm")?.text).toBe("only");
    expect(pickForSlot(quotes("same", "same"), "pm")?.text).toBe("same");
  });

  it("has nothing to offer for an empty feed", () => {
    expect(pickForSlot([], "am")).toBeUndefined();
    expect(pickForSlot([], "pm")).toBeUndefined();
  });

  it("gives the real feed two different quotes", () => {
    const items = parseFeed(fixture("brainyquote.rss"));
    expect(pickForSlot(items, "am")?.text).not.toBe(pickForSlot(items, "pm")?.text);
  });
});

describe("fallbackFor", () => {
  it("never serves the same quote morning and evening, on any day of a full year", () => {
    for (let day = new Date("2026-01-01T00:00:00Z"); day < new Date("2027-01-01T00:00:00Z"); day.setUTCDate(day.getUTCDate() + 1)) {
      const key = day.toISOString().slice(0, 10);
      expect(fallbackFor(`${key}:am`).text, key).not.toBe(fallbackFor(`${key}:pm`).text);
    }
  });

  it("rotates from one day to the next", () => {
    const week = ["2026-09-01", "2026-09-02", "2026-09-03", "2026-09-04", "2026-09-05"].map(
      (d) => fallbackFor(`${d}:am`).text,
    );
    expect(new Set(week).size).toBeGreaterThan(1);
  });

  it("is deterministic and always answers from the pool", () => {
    expect(fallbackFor("2026-09-04:am")).toEqual(fallbackFor("2026-09-04:am"));
    expect(FALLBACK_QUOTES).toContainEqual(fallbackFor("2026-09-04:pm"));
  });
});

// ── configuration ────────────────────────────────────────────────────────────

describe("configuration", () => {
  it("reads BrainyQuote and Berlin by default", () => {
    expect(feedUrl()).toBe("https://www.brainyquote.com/link/quotebr.rss");
    expect(timezone()).toBe(BERLIN);
  });

  it.each(["off", "OFF", "", "   "])("treats %o as the kill switch", (value) => {
    vi.stubEnv("SYNAPSE_QUOTE_FEED_URL", value);
    expect(feedUrl()).toBeNull();
  });

  it("takes an override for both", () => {
    vi.stubEnv("SYNAPSE_QUOTE_FEED_URL", "https://example.test/feed.rss");
    vi.stubEnv("SYNAPSE_QUOTE_TZ", "Asia/Kolkata");
    expect(feedUrl()).toBe("https://example.test/feed.rss");
    expect(timezone()).toBe("Asia/Kolkata");
  });
});

describe("currentQuote", () => {
  it("answers from the bundled pool without touching the network when the feed is off", () => {
    vi.stubEnv("SYNAPSE_QUOTE_FEED_URL", "off");
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);

    const quote = currentQuote(new Date("2026-09-04T04:00:00Z"));

    expect(FALLBACK_QUOTES).toContainEqual(quote);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("still changes between the morning and the evening with no feed at all", () => {
    vi.stubEnv("SYNAPSE_QUOTE_FEED_URL", "off");
    vi.stubGlobal("fetch", vi.fn());

    const morning = currentQuote(new Date("2026-09-04T04:00:00Z"));
    const evening = currentQuote(new Date("2026-09-04T16:00:00Z"));

    expect(morning.text).not.toBe(evening.text);
  });

  it("serves a cold render from the pool and leaves one refresh in flight", async () => {
    vi.stubEnv("SYNAPSE_QUOTE_FEED_URL", "https://example.test/feed.rss");
    const fetchSpy = vi.fn(async () => new Response(fixture("brainyquote.rss"), { status: 200 }));
    vi.stubGlobal("fetch", fetchSpy);

    // The first render must not wait for the network — it answers from the pool immediately.
    expect(FALLBACK_QUOTES).toContainEqual(currentQuote(new Date("2026-09-04T04:00:00Z")));
    // Concurrent renders share the one in-flight request rather than each starting their own.
    currentQuote(new Date("2026-09-04T04:00:01Z"));
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    await vi.waitFor(() => {
      expect(currentQuote(new Date("2026-09-04T04:00:02Z")).author).toBe("Alexander Pope");
    });
    // The evening comes out of the SAME fetch — a slot change inside a cached day costs nothing.
    expect(currentQuote(new Date("2026-09-04T16:00:00Z")).author).toBe("Jimmy Dean");
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("keeps serving the pool, and stops hammering the host, when the feed fails", async () => {
    vi.stubEnv("SYNAPSE_QUOTE_FEED_URL", "https://example.test/feed.rss");
    const fetchSpy = vi.fn(async () => new Response("nope", { status: 503 }));
    vi.stubGlobal("fetch", fetchSpy);

    const at = new Date("2026-09-04T04:00:00Z");
    expect(FALLBACK_QUOTES).toContainEqual(currentQuote(at));
    // Let the rejected request settle all the way through its `finally`, so the retry window —
    // and not merely the in-flight guard — is what the renders below run into.
    await vi.waitFor(() => expect(fetchSpy).toHaveBeenCalledTimes(1));
    await new Promise((resolve) => setTimeout(resolve, 5));

    for (let i = 0; i < 5; i += 1) expect(FALLBACK_QUOTES).toContainEqual(currentQuote(at));
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("treats a feed it cannot parse as a failure rather than an empty header", async () => {
    vi.stubEnv("SYNAPSE_QUOTE_FEED_URL", "https://example.test/feed.rss");
    vi.stubGlobal("fetch", vi.fn(async () => new Response("<html>maintenance</html>", { status: 200 })));

    const quote = currentQuote(new Date("2026-09-04T04:00:00Z"));
    expect(quote.text.length).toBeGreaterThan(0);
    expect(FALLBACK_QUOTES).toContainEqual(quote);
  });
});
