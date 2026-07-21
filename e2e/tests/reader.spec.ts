// `test` comes from ./fixtures: it fails a spec on any uncaught page error, so a boot
// failure names itself instead of surfacing as "element(s) not found".
import { expect, test } from "./fixtures";

/**
 * The reading path: the lesson renders, its islands hydrate, search navigates, and progress is
 * remembered. Everything here is ANONYMOUS — no sign-in, no sandbox — so it runs on every push.
 *
 * Sandbox-dependent paths (Run, Visualise) live in `sandbox.spec.ts` behind `E2E_SANDBOX`,
 * following the same env-gate convention as `GOJUDGE_IT` and `POSTGRES_IT`.
 */

/** The first lesson in the sitemap — content-agnostic, so this never pins a slug that moves. */
async function firstLessonPath(request: { get: (u: string) => Promise<{ text: () => Promise<string> }> }) {
  const xml = await (await request.get("/sitemap.xml")).text();
  const match = xml.match(/<loc>[^<]*(\/synapse\/[^<]+)<\/loc>/);
  if (!match) throw new Error("no lesson in the sitemap — is the content root mounted?");
  return match[1];
}

/**
 * Wait until the markdown island has actually filled the body. `.lesson-body` exists as an
 * empty shell from first paint and shows "rendering…" until the island finishes, so every
 * assertion about lesson content has to go through here or it races the pipeline.
 */
async function waitForLessonBody(page: import("@playwright/test").Page) {
  await expect(page.locator("h1").first()).toBeVisible();
  await expect(page.locator(".lesson-body")).toBeVisible();
  await expect
    .poll(async () => (await page.locator(".lesson-body").innerText()).length, { timeout: 30_000 })
    .toBeGreaterThan(200);
}

test("the server renders a per-page head, not the placeholder", async ({ page, request }) => {
  const path = await firstLessonPath(request);
  const response = await page.goto(path);
  const html = (await response?.text()) ?? "";

  // Asserted on the RAW response, before any JS runs — this is what a crawler sees, and that
  // is exactly what this assertion exists to prove. A client-side title would pass a naive
  // `page.title()` check.
  expect(html).toContain("<title>");
  expect(html).not.toContain("<title>Synapse</title>");
  expect(html).toMatch(/<meta name="description" content="[^"]+"/);
  expect(html).toMatch(/<link rel="canonical" href="https?:\/\/[^"]+"/);
  expect(html).toContain('property="og:title"');
});

test("the lesson body renders and its prose hydrates", async ({ page, request }) => {
  const path = await firstLessonPath(request);
  await page.goto(path);

  await waitForLessonBody(page);
  expect(await page.title()).not.toBe("Synapse");
});

test("the page does not scroll sideways", async ({ page, request }) => {
  // A 161px horizontal overflow once threw the fixed FAB rail 141px off-screen, because
  // `position: fixed; right: 20px` resolves against a layout viewport the overflow had
  // stretched. One assertion that would have caught it before a phone did.
  await page.goto(await firstLessonPath(request));
  await expect
    .poll(async () =>
      page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth),
    )
    .toBeLessThanOrEqual(1);
});

test("the command palette opens and navigates", async ({ page }) => {
  await page.goto("/");

  // Open via the HEADER BUTTON first, not the shortcut. The ⌘K handler is a window listener
  // attached by an effect, so pressing the moment `.header__search` paints can land before the
  // listener exists — which is exactly how this test failed at first, looking like a broken
  // shortcut rather than a race. Clicking is also the path most people actually take.
  const trigger = page.locator(".header__search");
  await expect(trigger).toBeVisible();
  await trigger.click();

  const input = page.locator(".cmdk__input");
  await expect(input).toBeVisible();

  // The palette once rendered at the page's bottom-left for a whole release because an
  // orphaned declaration block swallowed `.cmdk-scrim`'s `position: fixed`. `toBeVisible` was
  // true the entire time — so assert it is actually placed, not merely present.
  const box = await input.boundingBox();
  const viewport = page.viewportSize();
  expect(box, "the palette input has no box").not.toBeNull();
  if (box && viewport) {
    expect(box.y).toBeLessThan(viewport.height / 2);
    expect(box.x).toBeGreaterThan(0);
  }

  // Now that the app is demonstrably live, the keyboard path is safe to assert. Control, not
  // Meta: the handler accepts `meta_key() || ctrl_key()`, and headless macOS swallows Cmd+K
  // before it reaches the page, so Control means the same thing on every platform.
  await page.keyboard.press("Escape");
  await expect(input).toBeHidden();
  await page.keyboard.press("Control+k");
  await expect(input).toBeVisible();

  await input.fill("a");
  const first = page.locator(".cmdk__result").first();
  await expect(first).toBeVisible();
  await first.click();
  await expect(page).toHaveURL(/\/synapse\/.+/);
});

test("finishing a lesson is remembered across a reload", async ({ page, request }) => {
  const path = await firstLessonPath(request);
  await page.goto(path);
  await waitForLessonBody(page);

  // Auto-complete could not be exercised in the dev preview at all — it reports
  // `innerHeight: 0`, so the window cannot scroll. A real browser can, which is precisely the
  // gap this suite exists to close.
  //
  // Wait for the body to be POPULATED before scrolling: the markdown island fills it after
  // mount, so scrolling to `scrollHeight` too early lands at the bottom of an empty shell and
  // the page then grows underneath — leaving the reader nowhere near the end. (That is exactly
  // how this test failed on its first run.)
  await expect
    .poll(async () => (await page.locator(".lesson-body").innerText()).length)
    .toBeGreaterThan(200);
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await expect
    .poll(async () => page.evaluate(() => localStorage.getItem("reader-progress")))
    .toContain(path.replace("/synapse/", ""));

  await page.reload();
  await expect(page.locator(".reader-sidebar__link--done").first()).toBeVisible();

  // And the resume card is the payoff — a second visit differs from a first.
  await page.goto("/");
  await expect(page.locator(".lib-continue")).toBeVisible();
  await expect(page.locator(".lib-card__progress").first()).toBeVisible();
});
