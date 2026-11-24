// `/code` SIGNED IN — the lab itself, behind the gate. The anonymous half (the wall, and no
// Monaco for a caller who cannot use it) is codelab.spec.ts.
//
// GATED (set E2E_AUTH=1) and run against a Keycloak-ALLOWLISTED origin: the dev server on :5373.
// `dev-tools/e2e-auth` sets it up; the default `dev-tools/e2e` run skips this file. The instance
// does not enforce the submit allowlist, so `tester` passes the gate as any signed-in account does.
//
// One test presses Run: a Python program reading stdin, the smallest real round trip through
// `/api/run` and go-judge that proves Input reaches the program and Output shows what it printed.
import { expect, test } from "./fixtures";

const USER = process.env.E2E_KC_USER ?? "tester";
const PASS = process.env.E2E_KC_PASS ?? "tester";
const LAB = "/code";

const editorLines = (page: import("@playwright/test").Page) =>
  page.locator("[data-codelab-editor] .view-lines");

test.describe("signed-in /code — the lab behind the gate", () => {
  test.skip(
    !process.env.E2E_AUTH,
    "set E2E_AUTH=1 with Keycloak up and E2E_BASE_URL a realm-allowlisted origin (e.g. http://localhost:5373)",
  );

  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.locator(".account-chip__signin").click();
    await page.locator("#username").fill(USER);
    await page.locator("#password").fill(PASS);
    await page.locator("#kc-login").click();
    await expect(page.locator(".account-chip__user")).toHaveText(`@${USER}`, { timeout: 30_000 });
    // Each test starts from the starters, not from what the previous test left behind.
    await page.evaluate(() => {
      for (const key of Object.keys(localStorage)) if (key.startsWith("code-lab")) localStorage.removeItem(key);
    });
  });

  test("the lab opens on the editor beside Input over Output", async ({ page }) => {
    await page.goto(LAB);

    await expect(page.locator(".lab-pane--l")).toBeVisible();
    await expect(page.locator(".clab__io")).toBeVisible();
    await expect(page.locator(".clab-io--in")).toBeVisible();
    await expect(page.locator(".clab-io--out")).toBeVisible();

    await expect(editorLines(page)).toContainText('print(greet("World"))', { timeout: 30_000 });
    await expect(page.locator(".clab-tab--on")).toContainText("Code 1");
    await expect(page.locator("[data-codelab-empty]")).toContainText("Ready to Run");
    await expect(page.locator("#clab-stdin")).toHaveAttribute("placeholder", "Enter input...");
  });

  test("the language menu offers every runnable language and swaps the starter", async ({ page }) => {
    await page.goto(LAB);
    await expect(editorLines(page)).toContainText("greet", { timeout: 30_000 });

    await page.locator(".clab-lang").click();
    const items = page.locator(".clab-menu__item");
    await expect(items).toHaveCount(11);
    await expect(page.locator(".clab-menu__item--on")).toContainText("Python");

    await items.filter({ hasText: /^Go$/ }).click();
    await expect(page.locator(".clab-lang")).toContainText("Go");
    await expect(editorLines(page)).toContainText("package main");
  });

  test("+ opens a second tab, and the first keeps its program", async ({ page }) => {
    await page.goto(LAB);
    await expect(editorLines(page)).toContainText("greet", { timeout: 30_000 });

    await page.locator(".clab-tabs__add").click();
    await expect(page.locator(".clab-tab")).toHaveCount(2);
    await expect(page.locator(".clab-tab--on")).toContainText("Code 2");

    await page.locator(".clab-tab__name", { hasText: "Code 1" }).click();
    await expect(editorLines(page)).toContainText('print(greet("World"))');
  });

  test("the settings popover changes the editor's font size live", async ({ page }) => {
    await page.goto(LAB);
    await expect(editorLines(page)).toContainText("greet", { timeout: 30_000 });

    await page.locator('button[aria-label="Editor settings"]').click();
    const range = page.locator(".clab-set__range");
    await range.fill("20");
    await expect(page.locator(".clab-set__px")).toHaveText("20px");
    await expect
      .poll(() => editorLines(page).evaluate((node) => getComputedStyle(node).fontSize))
      .toBe("20px");
  });

  test("a share link reopens the same program", async ({ page, context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(LAB);
    await expect(editorLines(page)).toContainText("greet", { timeout: 30_000 });

    await editorLines(page).click();
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.type("\nshared = 7");
    // By keyboard: on the dev server Astro's toolbar sits over the status line and takes the
    // pointer, and a status-line action must be reachable without one anyway.
    await page.locator('button[aria-label="Copy a share link"]').press("Enter");
    await expect(page.locator(".lab-toast")).toBeVisible();
    const url = await page.evaluate(() => navigator.clipboard.readText());
    expect(url).toContain("/code#s=");

    // A REAL load in a second tab. The reader's edited tab is saved, so it stays, and the shared
    // program opens BESIDE it: two tabs, and the toast is the only thing that says the link did it.
    await page.close();
    const fresh = await context.newPage();
    await fresh.goto(url);
    await expect(fresh.locator(".lab-toast")).toHaveText("Opened a shared program.", { timeout: 30_000 });
    await expect(fresh.locator(".clab-tab")).toHaveCount(2);
    await expect(editorLines(fresh)).toContainText("shared = 7");
    // The fragment is spent once opened, so a reload cannot clobber later edits.
    await expect.poll(() => fresh.evaluate(() => location.hash)).toBe("");

    // Pasted into the lab already open, the link changes only the fragment, which loads nothing.
    await fresh.evaluate((hash) => (location.hash = hash), new URL(url).hash);
    await expect(fresh.locator(".clab-tab")).toHaveCount(3);
    await expect.poll(() => fresh.evaluate(() => location.hash)).toBe("");
  });

  test("Run hands Input to the program and shows what it printed", async ({ page }) => {
    await page.goto(LAB);
    await expect(editorLines(page)).toContainText("greet", { timeout: 30_000 });

    await editorLines(page).click();
    await page.keyboard.press("ControlOrMeta+A");
    await page.keyboard.press("Delete");
    await page.keyboard.type("name = input()\nprint('hi ' + name)");
    await page.locator("#clab-stdin").fill("lab");

    await page.locator(".clab-run").click();
    const output = page.locator("[data-codelab-output]");
    await expect(output.locator('[data-codelab-status="Accepted"]')).toBeVisible({ timeout: 60_000 });
    await expect(output).toContainText("hi lab");
  });
});
