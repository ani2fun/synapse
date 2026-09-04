// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot
// failure names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * The site header: the twice-daily quote in the middle, and a control row whose four members are
 * ONE size.
 *
 * The suite runs with `SYNAPSE_QUOTE_FEED_URL=off` (set by `dev-tools/e2e`), so the quote comes
 * from the bundled pool and never from BrainyQuote — a spec that fetched the live feed would
 * assert against a third party's uptime and against text that changes at 06:00 and 18:00.
 *
 * The height assertion is the point of this file. "The controls look unbalanced" is otherwise a
 * matter of taste that no test can hold, and it regressed once already by nobody doing anything
 * wrong: each control simply grew its own padding.
 */

/** Every author in `web/src/lib/quotes.fallback.ts`. Seeing one proves the FALLBACK rendered. */
const POOL_AUTHORS = [
  "Paul Halmos",
  "Aristotle",
  "Nelson Mandela",
  "Edsger W. Dijkstra",
  "Marie Curie",
  "Antoine de Saint-Exupéry",
  "Robert Collier",
  "Confucius",
];

/**
 * The controls that must agree, in the order they sit in the row.
 *
 * The chip is matched by ANY of its three states. It renders `__quiet` while check-sso is
 * outstanding, and which state a run catches is a race it is not this spec's business to win —
 * the header must not reflow when that race resolves, so all three carry the row's height.
 */
const CHIP = ".account-chip__signin, .account-chip__user, .account-chip__quiet";
const CONTROLS = [".header__search", ".header__link", ".header__icon-btn", CHIP];

test("the header carries a quote and keeps to one row", async ({ page }) => {
  await page.goto("/");

  const quote = page.locator(".header-quote");
  await expect(quote).toBeVisible();

  const text = (await page.locator(".header-quote__text").innerText()).trim();
  const author = (await page.locator(".header-quote__by").innerText()).replace(/^—\s*/, "").trim();
  expect(text.length).toBeGreaterThan(0);
  expect(POOL_AUTHORS, `unexpected author ${author} — did the feed kill switch not apply?`).toContain(author);

  // The full line stays reachable on hover even where the text truncates.
  await expect(quote).toHaveAttribute("title", `${text} — ${author}`);

  // The quote must never grow the bar. A 95px header breaks every fixed-header offset below it.
  const header = await page.locator(".header").boundingBox();
  expect(header?.height ?? 0).toBeLessThan(64);
});

test("search leads the control row instead of the middle", async ({ page }) => {
  await page.goto("/");

  // It lives in the actions now — and it is the first thing in them.
  await expect(page.locator(".header__actions > .header__search")).toBeVisible();
  await expect(page.locator(".header__actions > :first-child")).toHaveClass(/header__search/);
  await expect(page.locator(".header__mid .header__search")).toHaveCount(0);

  // islands/palette.ts binds it by class, so the move must cost the ⌘K palette nothing.
  await page.locator(".header__search").click();
  await expect(page.locator(".cmdk__input")).toBeVisible();
});

test("every control in the row is the same height", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(CHIP).first()).toBeVisible();

  const heights: Record<string, number> = {};
  for (const selector of CONTROLS) {
    const box = await page.locator(selector).first().boundingBox();
    expect(box, `${selector} has no box`).not.toBeNull();
    heights[selector] = Math.round(box?.height ?? 0);
  }

  const distinct = new Set(Object.values(heights));
  expect(distinct.size, `controls disagree on height: ${JSON.stringify(heights)}`).toBe(1);

  // And they are vertically centred on each other, not merely equal in size.
  const tops = new Set<number>();
  for (const selector of CONTROLS) {
    const box = await page.locator(selector).first().boundingBox();
    tops.add(Math.round(box?.y ?? 0));
  }
  expect(tops.size, "controls are the same height but not on the same line").toBe(1);
});

test("the quote stands down on a phone, and the row still fits", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto("/");
  await expect(page.locator(CHIP).first()).toBeVisible();

  // No room for it, so it is absent rather than an ellipsis with an author attached.
  await expect(page.locator(".header-quote")).toBeHidden();

  // The search collapses to a square and the bar stays ONE row.
  const search = await page.locator(".header__search").boundingBox();
  expect(Math.round(search?.width ?? 0)).toBe(Math.round(search?.height ?? 0));
  const header = await page.locator(".header").boundingBox();
  expect(header?.height ?? 0).toBeLessThan(64);
});

