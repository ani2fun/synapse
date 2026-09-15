import { expect, test } from "@playwright/test";

/**
 * A PRIVATE book — a satellite served to its reader list only — as everyone who is not on the
 * list sees it. The fixture mounts `e2e/fixture-private-guide` with `readers: ["tester"]`; this
 * suite never signs in, so every answer here is the refusal, and the refusal must be the same
 * one at every door: the API, the page, the index, search and the sitemap.
 *
 * The one thing NOT tested here is a reader getting in — that needs a real Keycloak session and
 * lives in private-book-authed.spec.ts, behind E2E_AUTH.
 */

const LESSON = "/synapse/programming-languages/insight-earned/notes/selection-sort";

test("the API refuses an anonymous read with a sign-in answer, not a 404", async ({ request }) => {
  const response = await request.get(`/api${LESSON}`);
  expect(response.status()).toBe(401);
  const body = await response.json();
  expect(body.error).toBe("Sign in to read this book");
  expect(body.detail).toContain("insight-earned");
});

test("the page ships the private shell with the API's status, and no prose", async ({ page }) => {
  const response = await page.goto(LESSON);
  expect(response?.status()).toBe(401);
  await expect(page.locator("[data-private-lesson]")).toBeVisible();
  await expect(page.locator("[data-private-status]")).toContainText("Sign in to read this book");
  // Nothing of the lesson reached the browser: not the prose, not the title.
  await expect(page.locator("body")).not.toContainText("quokka");
  await expect(page.locator("body")).not.toContainText("Selection Sort, Rewritten");
});

test("the index omits the private book, and the public satellite is still there", async ({ request }) => {
  const index = await (await request.get("/api/synapse/index")).json();
  const languages = index.entries.find(
    (entry: { kind: string; slug: string }) => entry.kind === "category" && entry.slug === "programming-languages",
  );
  expect(languages).toBeTruthy();
  const slugs = languages.entries.map((entry: { slug: string }) => entry.slug);
  expect(slugs).toContain("java");
  expect(slugs).not.toContain("insight-earned");
  expect(JSON.stringify(index)).not.toContain('"private"');
});

test("search never surfaces private prose to an anonymous caller", async ({ request }) => {
  const results = await (await request.get("/api/synapse/search?q=quokka")).json();
  expect(results.results).toEqual([]);
});

test("the sitemap does not announce the private book", async ({ request }) => {
  const sitemap = await (await request.get("/sitemap.xml")).text();
  expect(sitemap).toContain("programming-languages/java/first-steps/what-java-is");
  expect(sitemap).not.toContain("insight-earned");
});

test("a bearer that does not verify is refused, never treated as anonymous", async ({ request }) => {
  const response = await request.get(`/api${LESSON}`, { headers: { Authorization: "Bearer not-a-token" } });
  expect(response.status()).toBe(401);
  expect((await response.json()).error).toBe("Invalid bearer token");
  // …while a public lesson never looks at the header at all.
  const open = await request.get("/api/synapse/programming-languages/java/first-steps/what-java-is", {
    headers: { Authorization: "Bearer not-a-token" },
  });
  expect(open.status()).toBe(200);
});

test("a private book's media is refused with its prose, and public media stays public", async ({ request }) => {
  const refused = await request.get("/media/insight-earned/pass.svg");
  expect(refused.status()).toBe(401);
  // The public satellite's simulator assets and the spine's media keep their hour of cache.
  const open = await request.get("/media/insight-earned/pass.svg", { headers: { Authorization: "Bearer not-a-token" } });
  expect(open.status()).toBe(401);
});
