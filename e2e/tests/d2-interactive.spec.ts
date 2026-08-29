import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { expect, test } from "@playwright/test";

/**
 * `d2-interactive` — a multi-board .d2 packaged as ONE page a reader drives.
 *
 * The whole promise of the output is that it needs nothing: no server, no network, no build. So
 * this generates the page for real and opens it over `file://`, and the run is offlined first —
 * a page that quietly reached for a CDN would still look right here otherwise.
 */

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const EXAMPLE = join(REPO, "dev-tools/examples/url-shortener.d2");
const GENERATOR = join(REPO, "dev-tools/d2-interactive.mjs");

let pageUrl: string;

test.beforeAll(() => {
  const out = join(mkdtempSync(join(tmpdir(), "d2-interactive-")), "url-shortener.html");
  // Run from `web/`, because the generator resolves `@terrastruct/d2` from the WORKING DIRECTORY
  // — it is meant to be run beside the content it draws, wherever the install happens to be.
  execFileSync("node", [GENERATOR, EXAMPLE, "--output", out, "--title", "URL Shortener"], {
    cwd: join(REPO, "web"),
    encoding: "utf8",
  });
  pageUrl = pathToFileURL(out).href;
});

test.beforeEach(async ({ context }) => {
  // Nothing may load off the network. The assertion is the whole point of a single file.
  await context.route("http://**", (route) => route.abort());
  await context.route("https://**", (route) => route.abort());
});

test("drills Context → Container → Component → Code by clicking", async ({ page }) => {
  await page.goto(pageUrl);
  await expect(page.locator(".trail .here")).toHaveText("URL Shortener");

  // Each level's drill-down node is the only linked one on its board.
  for (const board of ["Container", "Component", "Code"]) {
    await page.locator(".figure a").first().click();
    await expect(page.locator(".trail .here")).toHaveText(board);
  }
  await expect(page).toHaveURL(/#code$/);
});

test("back, forward and home walk the boards", async ({ page }) => {
  await page.goto(pageUrl);
  await page.locator(".figure a").first().click();
  await expect(page.locator(".trail .here")).toHaveText("Container");

  await page.locator("#back").click();
  await expect(page.locator(".trail .here")).toHaveText("URL Shortener");
  await page.locator("#fwd").click();
  await expect(page.locator(".trail .here")).toHaveText("Container");
  await page.locator("#home").click();
  await expect(page.locator(".trail .here")).toHaveText("URL Shortener");

  // Disabled where they would do nothing, so the controls tell the truth about the trail.
  await expect(page.locator("#home")).toBeDisabled();
});

test("the arrows walk the boards before any history exists", async ({ page }) => {
  // A freshly opened page has been nowhere, so history leaves both arrows dead; forward falls
  // back to stepping the manifest's order rather than describing its own uselessness.
  await page.goto(pageUrl);
  await expect(page.locator(".trail .here")).toHaveText("URL Shortener");
  await expect(page.locator("#back")).toBeDisabled();
  await expect(page.locator("#home")).toBeDisabled();
  await expect(page.locator("#fwd")).toBeEnabled();

  await page.locator("#fwd").click();
  await expect(page.locator(".trail .here")).toHaveText("Container");
  await expect(page.locator("#back")).toBeEnabled();
});

test("the browser's own Back and Forward step through the boards", async ({ page }) => {
  // The one place this behaviour is right: nothing else is on the page for Back to mean.
  await page.goto(pageUrl);
  await page.locator(".figure a").first().click();
  await expect(page.locator(".trail .here")).toHaveText("Container");
  await page.locator(".figure a").first().click();
  await expect(page.locator(".trail .here")).toHaveText("Component");

  await page.goBack();
  await expect(page.locator(".trail .here")).toHaveText("Container");
  await page.goForward();
  await expect(page.locator(".trail .here")).toHaveText("Component");
});

test("a #board deep link opens that board directly", async ({ page }) => {
  await page.goto(`${pageUrl}#component`);
  await expect(page.locator(".trail .here")).toHaveText("Component");
  // The breadcrumb still reads from the root, so a shared link says where it landed.
  await expect(page.locator(".trail button").first()).toHaveText("URL Shortener");
});

test("the menu jumps straight to any board", async ({ page }) => {
  await page.goto(pageUrl);
  await page.locator("#menu-btn").click();
  await page.locator('.menu button[data-board$="code"]').click();
  await expect(page.locator(".trail .here")).toHaveText("Code");
  await expect(page.locator(".menu")).toBeHidden();
});

test("keyboard steps the history and Escape closes the menu", async ({ page }) => {
  await page.goto(pageUrl);
  await page.locator(".figure a").first().click();
  await expect(page.locator(".trail .here")).toHaveText("Container");
  await page.keyboard.press("ArrowLeft");
  await expect(page.locator(".trail .here")).toHaveText("URL Shortener");
  await page.keyboard.press("ArrowRight");
  await expect(page.locator(".trail .here")).toHaveText("Container");

  await page.locator("#menu-btn").click();
  await expect(page.locator(".menu")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator(".menu")).toBeHidden();
});

test("every board is inlined — the page loads nothing at all", async ({ page }) => {
  const external: string[] = [];
  page.on("request", (request) => {
    if (!request.url().startsWith("file://")) external.push(request.url());
  });
  await page.goto(pageUrl);
  // Walk the whole stack, which is where a lazily-fetched board would give itself away.
  for (let i = 0; i < 3; i += 1) await page.locator(".figure a").first().click();
  await expect(page.locator(".trail .here")).toHaveText("Code");
  expect(external).toEqual([]);
  // `.first()`: d2 wraps its drawing in an outer sizing <svg>, so this matches two by design.
  await expect(page.locator(".figure svg").first()).toBeVisible();
});
