// Reading a lesson aloud, as anyone who is NOT on the listen list sees it — in both shapes a
// deployment can take. With no speech engine (`SYNAPSE_TTS_URL` unset, the main pass) the routes do
// not exist; with one (the second pass, against `fake-tts.mjs`) they exist and refuse. Either way a
// lesson page shows no listen control at all, and each mode asserts the other's markers are absent,
// because an engine that is down still serves a good page.
//
// The listed reader's half needs a Keycloak session: `listen-authed.spec.ts`.
import { expect, test } from "./fixtures";

const ENGINE = (process.env.SYNAPSE_TTS_URL ?? "").trim() !== "";
const LESSON = "/synapse/learn/smoke/intro";

test.describe("no speech engine configured", () => {
  test.skip(ENGINE, "stack is running with a speech engine");

  test("the listen routes are absent and the lesson offers nothing", async ({ page, request }) => {
    expect((await request.get("/api/listen/config")).status()).toBe(404);
    expect((await request.get("/api/admin/listeners")).status()).toBe(404);
    await page.goto(LESSON);
    await expect(page.locator("article.lesson-body")).toContainText("deterministic");
    await expect(page.locator("[data-listen-header], [data-listen-bar], [data-listen-pop]")).toHaveCount(0);
  });
});

test.describe("a speech engine configured", () => {
  test.skip(!ENGINE, "stack is running without a speech engine");

  test("an anonymous reader is told no, and the page loads no listen control", async ({ page, request }) => {
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
    // Give the probe time to answer, then prove it answered "no".
    await page.waitForLoadState("networkidle");
    await expect(page.locator("[data-listen-header], [data-listen-bar], [data-listen-pop]")).toHaveCount(0);
  });
});
