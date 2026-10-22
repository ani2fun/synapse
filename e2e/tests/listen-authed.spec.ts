// Reading a lesson aloud, as the listen list sees it — the half only a real Keycloak session can
// prove. A listed reader gets the header control and the selection popover, the units the page
// sends carry the prose and none of the code or the table, and the stream plays. A signed-in
// reader who is NOT listed sees nothing and is refused by the API.
//
// GATED (set E2E_AUTH=1), against the Keycloak-allowlisted dev origin with `fake-tts.mjs` as the
// engine — `dev-tools/e2e-auth` sets both up. The dev realm seeds `tester` onto the listen list
// (migration 0011) and deliberately not `test1`.
import { expect, test } from "./fixtures";

const READER = process.env.E2E_KC_USER ?? "tester";
const READER_PASS = process.env.E2E_KC_PASS ?? "tester";
const OUTSIDER = process.env.E2E_KC_USER2 ?? "test1";
const OUTSIDER_PASS = process.env.E2E_KC_PASS2 ?? "test1";
const LESSON = "/synapse/learn/smoke/intro";

async function signIn(page: import("@playwright/test").Page, user: string, pass: string) {
  await page.goto("/");
  await page.locator(".account-chip__signin").click();
  await page.locator("#username").fill(user);
  await page.locator("#password").fill(pass);
  await page.locator("#kc-login").click();
  await expect(page.locator(".account-chip__user")).toHaveText(`@${user}`, { timeout: 30_000 });
}

test.describe("listen — a listed reader, and one who is not", () => {
  test.skip(
    !process.env.E2E_AUTH,
    "set E2E_AUTH=1 with Keycloak up, a speech engine configured, and E2E_BASE_URL a realm-allowlisted origin",
  );

  test("the whole lesson is read without its code or its table, and the stream plays", async ({ page }) => {
    await signIn(page, READER, READER_PASS);
    await page.goto(LESSON);
    const header = page.locator("[data-listen-header]");
    await expect(header).toBeVisible({ timeout: 30_000 });

    const sent = page.waitForRequest((r) => r.url().endsWith("/api/listen") && r.method() === "POST");
    const stream = page.waitForResponse((r) => /\/api\/listen\/[0-9a-f]+\.mp3$/.test(r.url()));
    await header.click();
    const units: string[] = (await sent).postDataJSON().units;
    const said = units.join(" ");
    expect(said).toContain("deterministic");
    expect(said).toContain("A list item, so list styles are exercised.");
    expect(said, "code is never read").not.toContain("def greet");
    expect(said, "tables are never read").not.toContain("a third value");
    expect(units[0].split(/[.!?]\s/).length, "the first unit is one sentence, for a fast start").toBe(1);
    expect(units.every((u) => u.length <= 400)).toBe(true);

    const response = await stream;
    expect(response.status()).toBe(200);
    expect(response.headers()["content-type"]).toBe("audio/mpeg");
    expect(response.headers()["content-encoding"], "audio is never gzipped").toBeUndefined();
    await expect(page.locator("[data-listen-bar]")).toBeVisible();
    await expect
      .poll(() => page.locator("[data-listen-bar] audio").evaluate((a) => (a as HTMLAudioElement).currentTime))
      .toBeGreaterThan(0);
  });

  test("a selection offers Listen and From here", async ({ page }) => {
    await signIn(page, READER, READER_PASS);
    await page.goto(LESSON);
    await expect(page.locator("[data-listen-header]")).toBeVisible({ timeout: 30_000 });
    // Select the second paragraph's text, the way a reader's drag would.
    await page.locator("article.lesson-body p").nth(1).evaluate((p) => {
      const range = document.createRange();
      range.selectNodeContents(p);
      const selection = document.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
    });
    const pop = page.locator("[data-listen-pop]");
    await expect(pop).toBeVisible();

    const sent = page.waitForRequest((r) => r.url().endsWith("/api/listen") && r.method() === "POST");
    await pop.getByRole("button", { name: "Listen to the selection" }).click();
    const units: string[] = (await sent).postDataJSON().units;
    expect(units.join(" ")).toContain("a little of everything");
    expect(units.join(" "), "a selection reads only itself").not.toContain("deterministic");
  });

  test("a signed-in reader who is not listed sees nothing and is refused", async ({ page }) => {
    await signIn(page, OUTSIDER, OUTSIDER_PASS);
    await page.goto(LESSON);
    await expect(page.locator("article.lesson-body")).toContainText("deterministic");
    await page.waitForLoadState("networkidle");
    await expect(page.locator("[data-listen-header]")).toHaveCount(0);
    const status = await page.evaluate(async () => {
      const token = (window as unknown as { __synapseVizToken?: () => string | null }).__synapseVizToken?.();
      const res = await fetch("/api/listen", {
        method: "POST",
        headers: { "Content-Type": "application/json", ...(token ? { Authorization: `Bearer ${token}` } : {}) },
        body: JSON.stringify({ voice: "af_heart", units: ["Hello."] }),
      });
      return res.status;
    });
    expect(status).toBe(403);
  });
});
