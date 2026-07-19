// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot
// failure names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * Phone width. Four separate steps shipped mobile-layout bugs that desktop checks could not
 * see (33, 42, 43, 46), and one of them — the nav drawer sitting UNDER the fixed header, so its
 * close button was unclickable on every phone — survived from step 33 to step 42 because every
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

  // The step-42 bug precisely: the drawer rendered, looked fine, and `elementFromPoint` at the
  // ✕ returned `header__mid` because the drawer sat under the fixed header. Visible was true;
  // clickable was not. Assert what is actually on top at that point.
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
