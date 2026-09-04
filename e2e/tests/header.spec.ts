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
 * The chip is pinned to `__signin`, not matched across all three of its states: every auth failure
 * path lands on `anonymous`, so an anonymous page load reaches the Sign in button whether or not
 * Keycloak is reachable. All three states do carry the row's height — the header must not reflow
 * when check-sso answers — but only the settled one belongs in a symmetry assertion.
 */
const CHIP = ".account-chip__signin";
const CONTROLS = [".header__search", ".header__link", ".header__icon-btn", CHIP];

test("the header carries a quote and keeps to one row", async ({ page }) => {
  await page.goto("/");

  const quote = page.locator(".header-quote");
  await expect(quote).toBeVisible();

  const text = (await page.locator(".header-quote__text").innerText()).trim();
  const author = (await page.locator(".header-quote__by").innerText()).replace(/^—\s*/, "").trim();
  expect(text.length).toBeGreaterThan(0);
  expect(POOL_AUTHORS, `unexpected author ${author} — did the feed kill switch not apply?`).toContain(author);

  // The full line stays reachable on hover even where the text truncates, and the block names
  // itself: without it a screen reader meets a bare blockquote in the nav with no context. The
  // name is the LABEL alone — the quote is the figure's content, and putting it in both would
  // have it announced twice.
  await expect(quote).toHaveAttribute("title", `Featured quote: ${text} — ${author}`);
  await expect(quote).toHaveAttribute("aria-label", "Featured quote");

  // The quote must never grow the bar. A 95px header breaks every fixed-header offset below it.
  const header = await page.locator(".header").boundingBox();
  expect(header?.height ?? 0).toBeLessThan(64);
});

test("a long quote takes the room beside it, not a third of the bar", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".header-quote")).toBeVisible();

  // Longer than any bar can hold, so what shows is decided purely by the layout.
  await page.locator(".header-quote__text").evaluate((el) => {
    el.textContent =
      "You gotta try your luck at least once a day, because you could be going around lucky all day and not even know it.";
  });

  const quote = (await page.locator(".header-quote").boundingBox())!;
  const brand = (await page.locator(".header__brand").boundingBox())!;
  const actions = (await page.locator(".header__actions").boundingBox())!;
  const width = page.viewportSize()!.width;

  // Three equal columns centred the quote but rationed it to a third of the bar, ellipsising a
  // long one while the brand's identical share sat four-fifths empty. `1fr auto 1fr` sizes the
  // middle to the quote and splits what is left, so the spare room goes where the text is. Half
  // the bar is the discriminator: a third can never reach it.
  expect(quote.width).toBeGreaterThan(width * 0.5);

  // Taking the spare room must never mean taking someone else's. Both neighbours stay clear.
  expect(Math.round(quote.x)).toBeGreaterThanOrEqual(Math.round(brand.x + brand.width));
  expect(Math.round(quote.x + quote.width)).toBeLessThanOrEqual(Math.round(actions.x));

  // And it is still one line on one row.
  expect((await page.locator(".header").boundingBox())?.height ?? 0).toBeLessThan(64);
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

test("the account chip settles instead of sitting on its placeholder", async ({ page }) => {
  await page.goto("/");

  // The store starts `loading` and every failure path lands on `anonymous`, so this resolves with
  // or without Keycloak. It once did not, and not visibly: `useAuthState` read the state at mount
  // and only subscribed after first paint, so a check-sso that answered inside that window was
  // delivered to no listener at all. The store logged `anonymous`, the header showed `…`, and it
  // stayed that way for the rest of the session. `useSyncExternalStore` re-reads after
  // subscribing, which is the whole reason it is the primitive for this.
  await expect(page.locator(".account-chip__signin")).toBeVisible();
  await expect(page.locator(".account-chip__quiet")).toHaveCount(0);
});
