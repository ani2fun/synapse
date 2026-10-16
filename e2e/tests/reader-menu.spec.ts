// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot
// failure names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * The reader menu: one round button at a lesson's bottom-right that opens Contents, Search Book
 * and Themes & Settings, and — while it is open — the vertical scrubber down the right edge.
 *
 * The arrays lesson is the subject because it has sections (h2 + h3) and runs several screens,
 * so the scrubber has ticks to step between and a page to move through. It also mentions the
 * word "satellite", as the Java satellite lesson does, which is what lets Search Book prove its
 * scope: the same word, two books, one of them left out.
 */
const LESSON = "/synapse/learn/smoke/structures/arrays";

async function openMenu(page: import("@playwright/test").Page) {
  await page.goto(LESSON);
  const fab = page.locator(".reader-menu-fab");
  await expect(fab, "the chrome island never mounted the menu").toBeVisible();
  await fab.click();
  await expect(fab).toHaveAttribute("aria-expanded", "true");
  return fab;
}

test("the menu opens three rows and the scrubber, and Escape takes both away", async ({ page }) => {
  const fab = await openMenu(page);

  const rows = page.locator(".reader-menu [role=menuitem]");
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(0)).toContainText("Contents");
  await expect(rows.nth(1)).toContainText("Search Book");
  await expect(rows.nth(2)).toContainText("Themes & Settings");
  await expect(rows.nth(0), "opening focuses the first row").toBeFocused();

  const scrub = page.locator(".reader-scrub");
  await expect(scrub).toBeVisible();
  await expect(scrub).toHaveAttribute("role", "slider");
  // One tick per section the page shows: Indexing, Insertion, Shifting (h3), Trade-offs.
  await expect(scrub.locator(".reader-scrub__tick")).toHaveCount(4);
  await expect(scrub.locator(".reader-scrub__tick--l3")).toHaveCount(1);

  await page.keyboard.press("Escape");
  await expect(page.locator(".reader-menu-pop")).toHaveCount(0);
  await expect(scrub).toHaveCount(0);
  await expect(fab).toHaveAttribute("aria-expanded", "false");
  await expect(fab, "dismissing returns focus to the button").toBeFocused();
});

test("Contents opens the book drawer and closes the menu", async ({ page }) => {
  await openMenu(page);
  await page.locator(".reader-menu [role=menuitem]", { hasText: "Contents" }).click();

  const drawer = page.locator(".reader-nav-drawer");
  await expect(drawer).toBeVisible();
  await expect(drawer.locator(".reader-nav-drawer__title")).toHaveText("Browse");
  await expect(drawer.locator(".reader-sidebar__link--active")).toBeVisible();
  await expect(page.locator(".reader-menu-pop")).toHaveCount(0);
});

test("Search Book searches this book only, and the chip widens it to every book", async ({ page }) => {
  await openMenu(page);
  await page.locator(".reader-menu [role=menuitem]", { hasText: "Search Book" }).click();

  await expect(page.locator(".cmdk__scope-label")).toHaveText("In: Smoke Book");
  const input = page.locator(".cmdk__input");
  await expect(input).toBeFocused();
  await expect(input).toHaveAttribute("placeholder", "Search this book…");
  await input.fill("satellite");

  // `toHaveText` retries, which is what waits out the debounce and the full-text round trip.
  const results = page.locator(".cmdk__result");
  await expect(results.first().locator("mark")).toHaveText(/satellite/i);
  const scoped = await results.evaluateAll((rows) => rows.map((a) => a.getAttribute("href") ?? ""));
  expect(scoped.length).toBeGreaterThan(0);
  expect(scoped.every((href) => href.startsWith("/synapse/learn/smoke/")), scoped.join(", ")).toBe(true);

  // Lifting the scope keeps the query and lets the Java satellite in.
  await page.locator(".cmdk__scope-clear").click();
  await expect(page.locator(".cmdk__scope")).toHaveCount(0);
  await expect(input).toHaveValue("satellite");
  await expect(page.locator('.cmdk__result[href*="/java/"]').first()).toBeVisible();
});

test("settings switch the theme and the reading size, and the header toggle agrees", async ({ page }) => {
  await openMenu(page);
  await page.locator(".reader-menu [role=menuitem]", { hasText: "Themes & Settings" }).click();
  await expect(page.locator(".reader-settings__title")).toHaveText("Themes & Settings");

  await page.locator(".reader-settings__opt", { hasText: "Dark" }).click();
  await expect(page.locator("html")).toHaveClass(/\bdark\b/);
  await expect(page.locator("[data-theme-toggle]")).toHaveAttribute("aria-pressed", "true");
  await page.locator(".reader-settings__opt", { hasText: "Light" }).click();
  await expect(page.locator("html")).not.toHaveClass(/\bdark\b/);
  await expect(page.locator("[data-theme-toggle]")).toHaveAttribute("aria-pressed", "false");

  await page.locator(".reader-settings__opt", { hasText: "Large" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-reader-size", "lg");
  await expect(page.locator(".reader-settings__opt", { hasText: "Large" })).toHaveAttribute("aria-pressed", "true");

  // Escape steps back to the menu before it closes the menu.
  await page.keyboard.press("Escape");
  await expect(page.locator(".reader-menu [role=menuitem]")).toHaveCount(3);
  await expect(page.locator(".reader-menu [role=menuitem]", { hasText: "Themes & Settings" })).toBeFocused();
});

test("the scrubber steps from section to section and follows a drag", async ({ page }) => {
  await openMenu(page);
  const scrub = page.locator(".reader-scrub");
  await expect(scrub).toHaveAttribute("aria-valuenow", "0");
  await expect(scrub).toHaveAttribute("aria-valuetext", "Start of the lesson");

  await scrub.focus();
  await page.keyboard.press("ArrowDown");
  await expect(scrub).toHaveAttribute("aria-valuetext", "Indexing");
  // The jump lands the heading just under the fixed header, where a jump from the outline does.
  const top = await page.locator("h2#indexing").evaluate((el) => el.getBoundingClientRect().top);
  expect(Math.round(top)).toBe(80);
  await page.keyboard.press("ArrowDown");
  await expect(scrub).toHaveAttribute("aria-valuetext", "Insertion");
  await page.keyboard.press("End");
  await expect(scrub).toHaveAttribute("aria-valuenow", "100");
  await page.keyboard.press("Home");
  await expect(scrub).toHaveAttribute("aria-valuenow", "0");

  // A drag moves the page AT ONCE — the reader scrolls smoothly, and a drag that animated would
  // trail the pointer instead of following it.
  const thumb = await scrub.locator(".reader-scrub__thumb").boundingBox();
  const track = await scrub.locator(".reader-scrub__track").boundingBox();
  if (!thumb || !track) throw new Error("the scrubber has no box");
  await page.mouse.move(thumb.x + thumb.width / 2, thumb.y + thumb.height / 2);
  await page.mouse.down();
  await page.mouse.move(thumb.x + thumb.width / 2, track.y + track.height * 0.6, { steps: 5 });
  expect(await page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
  await expect(page.locator(".reader-scrub__bubble")).toBeVisible();
  await page.mouse.up();
  await expect(page.locator(".reader-scrub__bubble")).toBeHidden();
});

test("the scrubber re-measures its sections when a setting reflows the page", async ({ page }) => {
  await openMenu(page);
  const ticks = page.locator(".reader-scrub__tick");
  const before = await ticks.evaluateAll((els) => els.map((el) => (el as HTMLElement).style.top));

  await page.locator(".reader-menu [role=menuitem]", { hasText: "Themes & Settings" }).click();
  // Size, not width: at this viewport the column is already narrower than any width cap, so a
  // width change would reflow nothing. A larger face always does.
  await page.locator(".reader-settings__opt", { hasText: "Large" }).click();

  await expect
    .poll(() => ticks.evaluateAll((els) => els.map((el) => (el as HTMLElement).style.top)))
    .not.toEqual(before);
});
