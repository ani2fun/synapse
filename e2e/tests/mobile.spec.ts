// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot
// failure names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * Phone width. This project has repeatedly shipped mobile-layout bugs that desktop checks
 * could not see, including the nav drawer sitting UNDER the fixed header so its close button
 * was unclickable on every phone — a bug that survived multiple releases because every
 * verification pass was done on a desktop viewport.
 */

async function firstLessonPath(request: { get: (u: string) => Promise<{ text: () => Promise<string> }> }) {
  const xml = await (await request.get("/sitemap.xml")).text();
  const match = xml.match(/<loc>[^<]*(\/synapse\/[^<]+)<\/loc>/);
  if (!match) throw new Error("no lesson in the sitemap — is the content root mounted?");
  return match[1];
}

test("the reader fits the screen", async ({ page, request }) => {
  await page.goto(await firstLessonPath(request));
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - window.innerWidth,
  );
  expect(overflow, "horizontal overflow — step 46 all over again").toBeLessThanOrEqual(1);
});

test("the nav drawer opens and its close button is actually clickable", async ({ page, request }) => {
  await page.goto(await firstLessonPath(request));

  const fab = page.locator(".reader-nav-fab").first();
  await expect(fab).toBeVisible();
  await fab.click();

  // `.reader-nav` is the SIDEBAR container and is correctly hidden on a phone —
  // matching it was how the first run failed. The drawer is `.reader-nav-drawer`.
  const drawer = page.locator(".reader-nav-drawer").first();
  await expect(drawer).toBeVisible();

  // The failure mode this guards against: the drawer rendered, looked fine, and
  // `elementFromPoint` at the ✕ returned `header__mid` because the drawer sat under the fixed
  // header. Visible was true; clickable was not. Assert what is actually on top at that point.
  const close = page.locator(".reader-nav-drawer__close").first();
  await expect(close).toBeVisible();
  const box = await close.boundingBox();
  expect(box).not.toBeNull();
  if (box) {
    const topmost = await page.evaluate(
      ([x, y]) => {
        const el = document.elementFromPoint(x, y);
        return el ? `${el.tagName}.${el.className}` : "none";
      },
      [box.x + box.width / 2, box.y + box.height / 2] as [number, number],
    );
    expect(topmost, "something is covering the drawer's close button").not.toMatch(/header/i);
  }

  await close.click();
  await expect(drawer).toBeHidden();
});

/**
 * Inline citations — the same class of bug as the drawer above, and found the same way.
 *
 * Lessons carry page cites as native `title` attributes. A `title` opens on HOVER and nothing
 * else, so on a phone the `[i]` marker was visible and permanently unopenable: there was no
 * gesture that would ever reveal it. Desktop verification could not see this, because desktop
 * has a mouse.
 */
const LESSON_WITH_CITE = "/synapse/learn/smoke/intro";

test("a citation opens on tap, and closes again", async ({ page }) => {
  await page.goto(LESSON_WITH_CITE);

  const marker = page.locator("abbr[data-cite]").first();
  await expect(marker, "the citation island never hydrated").toBeVisible();

  // The native tooltip must be GONE, not merely supplemented: leaving `title` in place would
  // give a desktop reader two tooltips at two offsets.
  await expect(marker).not.toHaveAttribute("title", /.*/);

  const bubble = page.locator("#synapse-cite-pop");
  await expect(bubble).toBeHidden();

  await marker.click();
  await expect(bubble).toBeVisible();
  await expect(bubble).toHaveText("[p. 42]");
  await expect(marker).toHaveAttribute("aria-expanded", "true");

  // Tapping the same marker again puts it away — on a phone there is not always convenient
  // empty space to tap instead.
  await marker.click();
  await expect(bubble).toBeHidden();
});

test("a citation's tap target clears the 24px minimum", async ({ page }) => {
  await page.goto(LESSON_WITH_CITE);
  const marker = page.locator("abbr[data-cite]").first();
  await expect(marker).toBeVisible();

  // WCAG 2.5.8. `[i]` is a ~10px glyph; the padding that lifts it to 24 must come out of the
  // leading rather than the line box, so the surrounding prose keeps its rhythm.
  const box = await marker.boundingBox();
  expect(box).not.toBeNull();
  if (box) {
    expect(box.width, "citation tap target is too narrow to hit").toBeGreaterThanOrEqual(24);
    expect(box.height, "citation tap target is too short to hit").toBeGreaterThanOrEqual(24);
  }
});

test("a citation survives the scroll that focusing it causes", async ({ page }) => {
  await page.goto(LESSON_WITH_CITE);
  const marker = page.locator("abbr[data-cite]").first();
  await expect(marker).toBeVisible();

  // Mid-viewport first, and INSTANTLY. The reader scrolls smoothly, so a default
  // `scrollIntoView` is still animating when the assertions below run — the marker is genuinely
  // off-screen for a few hundred milliseconds, the bubble correctly gives up, and the spec fails
  // for a reason that has nothing to do with the behaviour under test. Measured: the marker sat
  // at y=958 in a 727px viewport a full frame after `focus()`, reaching centre only ~400ms later.
  await marker.evaluate((el) => el.scrollIntoView({ block: "center", behavior: "instant" }));

  // The regression this pins: the bubble used to close on ANY scroll. Focusing a marker makes
  // the browser scroll it into view, and that scroll lands a frame AFTER `focusin` — so the
  // keyboard path opened a bubble and then immediately shut it, every time.
  await marker.focus();
  const bubble = page.locator("#synapse-cite-pop");
  await expect(bubble).toBeVisible();

  await page.mouse.wheel(0, 40);
  await expect(bubble, "a small scroll dismissed the bubble instead of moving it").toBeVisible();

  // Scrolled clear of the marker, it should give up rather than float over unrelated prose.
  await page.mouse.wheel(0, 4000);
  await expect(bubble).toBeHidden();
});
