import { expect, test } from "@playwright/test";

/**
 * The ```simulator fence, end to end over a SATELLITE bundle: the marker hydrates into a
 * same-origin iframe over /simulators/<name>/, the bundle's own script runs inside it (the
 * suite runs behind axum's real security headers, so this is also the CSP regression net),
 * and the Enlarge modal opens a second live instance. The bundle lives in
 * e2e/fixture-java-guide/_simulators/hello-sim — proving the route probes the mounted set,
 * not just the primary checkout.
 */

const LESSON = "/synapse/programming-languages/java/first-steps/what-java-is";
const FRAME = 'iframe[src="/simulators/hello-sim/"]';

test("the fence hydrates into a sandboxed iframe whose bundle runs", async ({ page }) => {
  await page.goto(LESSON);
  const frame = page.locator(FRAME);
  await expect(frame).toBeVisible();
  await expect(frame).toHaveAttribute("title", "Hello Simulator");
  await expect(frame).toHaveAttribute("sandbox", /allow-scripts/);
  expect(await frame.evaluate((el) => el.style.height)).toBe("320px");
  // The bundle's module script wrote this — script, stylesheet and document all served
  // with types nosniff accepts, under the page CSP.
  await expect(page.frameLocator(FRAME).locator("#root")).toHaveText("hello from the simulator");
});

test("Enlarge opens a second live instance and Esc closes it", async ({ page }) => {
  await page.goto(LESSON);
  await expect(page.frameLocator(FRAME).locator("#root")).toBeVisible();
  await page.locator(".sim-embed__zoom").click();
  await expect(page.locator(".diagram-zoom-scrim")).toBeVisible();
  await expect(
    page.frameLocator(".diagram-zoom__iframe").locator("#root"),
  ).toHaveText("hello from the simulator");
  await page.keyboard.press("Escape");
  await expect(page.locator(".diagram-zoom-scrim")).toHaveCount(0);
});

test("the route redirects the bare directory and types the assets", async ({ request }) => {
  const redirect = await request.get("/simulators/hello-sim", { maxRedirects: 0 });
  expect(redirect.status()).toBe(301);
  expect(redirect.headers()["location"]).toBe("/simulators/hello-sim/");
  const js = await request.get("/simulators/hello-sim/assets/hello.js");
  expect(js.headers()["content-type"]).toBe("text/javascript");
  const html = await request.get("/simulators/hello-sim/");
  expect(html.headers()["cache-control"]).toBe("public, max-age=60");
});
