// Reading a lesson aloud, as anyone who is NOT on the listen list sees it — in both shapes a
// deployment can take. With no speech engine (`SYNAPSE_TTS_URL` unset, the main pass) the routes do
// not exist and the server-rendered Listen pill is removed. With one (the second pass, against
// `fake-tts.mjs`) the routes refuse, and the pill and the selection popover stay VISIBLE but gated —
// the "Suggest an edit" grammar: a reader learns the feature exists and how to ask. Each mode
// asserts the other's markers are absent.
//
// The listed reader's half needs a Keycloak session: `listen-authed.spec.ts`.
import { expect, test } from "./fixtures";

const ENGINE = (process.env.SYNAPSE_TTS_URL ?? "").trim() !== "";
const LESSON = "/synapse/learn/smoke/intro";
const PRIVATE_LESSON = "/synapse/programming-languages/insight-earned/notes/selection-sort";

test.describe("no speech engine configured", () => {
  test.skip(ENGINE, "stack is running with a speech engine");

  test("the listen routes are absent and the lesson offers nothing", async ({ page, request }) => {
    expect((await request.get("/api/listen/config")).status()).toBe(404);
    expect((await request.get("/api/admin/listeners")).status()).toBe(404);
    await page.goto(LESSON);
    await expect(page.locator("article.lesson-body")).toContainText("deterministic");
    // Server-rendered for everyone, then removed once the probe hears 404.
    await expect(page.locator("[data-listen-header]")).toHaveCount(0);
    await expect(page.locator("[data-listen-bar], [data-listen-pop]")).toHaveCount(0);
  });

  test("a private lesson offers no Listen to a reader who cannot open it", async ({ page }) => {
    await page.goto(PRIVATE_LESSON);
    await expect(page.locator("[data-private-status]")).toContainText("Sign in to read this book");
    await expect(page.locator("[data-listen-header]")).toBeHidden();
  });
});

test.describe("a speech engine configured", () => {
  test.skip(!ENGINE, "stack is running without a speech engine");

  test("an anonymous reader is refused, and sees Listen gated with how to ask", async ({ page, request }) => {
    const config = await request.get("/api/listen/config");
    expect(config.status()).toBe(200);
    const body = await config.json();
    expect(body.canListen).toBe(false);
    expect(body.voices.length).toBeGreaterThan(0);

    const start = await request.post("/api/listen", { data: { voice: body.voices[0].id, units: ["Hello."] } });
    expect(start.status()).toBe(401);
    expect((await request.get("/api/listen/not-a-ticket.mp3")).status()).toBe(404);
    expect((await request.get("/api/admin/listeners")).status()).toBe(401);

    await page.goto(LESSON);
    await expect(page.locator("article.lesson-body")).toContainText("deterministic");
    const pill = page.locator("[data-listen-header]");
    await expect(pill).toBeVisible();
    await expect(pill).toHaveAttribute("aria-disabled", "true");
    await expect(page.locator("[data-listen-tip]")).toHaveAttribute("data-tip", /listen list/);

    // Selecting prose shows the popover, gated, with the same explanation.
    await page.waitForLoadState("networkidle");
    await page.locator("article.lesson-body p").first().evaluate((p) => {
      const range = document.createRange();
      range.selectNodeContents(p);
      document.getSelection()?.removeAllRanges();
      document.getSelection()?.addRange(range);
    });
    const pop = page.locator('[data-listen-pop="gated"]');
    await expect(pop).toBeVisible();
    await expect(pop).toHaveAttribute("data-tip", /listen list/);
    // `force`: Playwright treats an aria-disabled control as not clickable, which is the point —
    // this proves a click that gets through anyway still starts nothing.
    await pop.getByRole("button", { name: "Listen to the selection" }).click({ force: true });
    await expect(page.locator("[data-listen-bar]"), "a gated click starts nothing").toHaveCount(0);
  });

  test("a private lesson keeps its Listen pill hidden until the lesson is open", async ({ page }) => {
    await page.goto(PRIVATE_LESSON);
    await expect(page.locator("[data-private-status]")).toContainText("Sign in to read this book");
    await page.waitForLoadState("networkidle");
    await expect(page.locator("[data-listen-header]")).toBeHidden();
  });
});
