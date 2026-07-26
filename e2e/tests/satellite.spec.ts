import { expect, test } from "@playwright/test";

/**
 * A book from a SEPARATE repository, merged into the same library.
 *
 * The fixture mounts `e2e/fixture-java-guide` — a repo whose root IS the book — under the
 * `programming-languages` grouping. What matters is that none of that is visible: the reader gets
 * a URL indistinguishable from one served out of the monorepo, which is the property the whole
 * multi-repo design is built to protect.
 */

const LESSON = "/synapse/programming-languages/java/first-steps/what-java-is";

test("a satellite book renders at its grafted URL", async ({ page }) => {
  const response = await page.goto(LESSON);
  expect(response?.status()).toBe(200);
  await expect(page.locator("h1").first()).toContainText("What Java Is");
  await expect(page.locator("body")).toContainText("Served from the satellite");
});

test("the satellite's book appears under its configured grouping in the index", async ({ request }) => {
  const index = await (await request.get("/api/synapse/index")).json();
  const languages = index.entries.find(
    (entry: { kind: string; slug: string }) => entry.kind === "category" && entry.slug === "programming-languages",
  );
  expect(languages, "the grouping the satellite was placed under").toBeTruthy();
  const java = languages.entries.find((entry: { slug: string }) => entry.slug === "java");
  expect(java, "the satellite's book, grafted").toBeTruthy();
  expect(java.title).toBe("Java");
});

/**
 * `category_path` is what every URL is built from — the index, prev/next and the sitemap. A graft
 * that forgets to rewrite it links the book at one path and lists it at another, and only
 * prev/next makes the mismatch visible.
 */
test("prev/next inside a satellite carry the grafted path", async ({ request }) => {
  const payload = await (await request.get(`/api${LESSON}`)).json();
  expect(payload.prev).toBe("programming-languages/java/index");
});

test("the satellite's lessons are in the sitemap", async ({ request }) => {
  const sitemap = await (await request.get("/sitemap.xml")).text();
  expect(sitemap).toContain("programming-languages/java/first-steps/what-java-is");
});

/** The monorepo's own books keep working — merging must be additive, never a replacement. */
test("the primary checkout's book still serves alongside it", async ({ request }) => {
  const response = await request.get("/api/synapse/learn/smoke/intro");
  expect(response.status()).toBe(200);
});
