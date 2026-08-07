import { expect, test } from "@playwright/test";

/**
 * A lone ```d2 fence must be drawn by the SERVER.
 *
 * This suite exists because the failure it guards is INVISIBLE. When pre-rendering breaks — the
 * wasm missing from a pruned image, the package bundled instead of externalised, the kill switch
 * left off — the page does not error: the transform emits the source placeholder, the client
 * compiles it, and the lesson renders correctly after a multi-megabyte download and a stalled
 * main thread. "Still works, just slowly" is exactly what nothing else would catch, so the
 * assertions here are deliberately about MECHANISM, not appearance:
 *
 *  - the SVG is in the HTML RESPONSE BODY, checked before a browser runs any script;
 *  - the d2 engine chunk is never requested while the lesson loads.
 *
 * The lesson's other two d2 fences are adjacent and stay a client-side slideshow by design
 * (only its first slide is ever on screen), so the engine chunk is only absent until a reader
 * steps it — which is why the network assertion scopes to the initial load.
 */

const LESSON = "/synapse/learn/smoke/intro";

test("a lone d2 fence arrives as SVG in the HTML, not as source for the client", async ({
  request,
}) => {
  const body = await (await request.get(LESSON)).text();

  // The pre-rendered card, and a real figure inside it.
  expect(body).toContain('data-prerendered="1"');
  expect(body).toMatch(/<div class="diagram__figure"><svg/);
  // The authored content reached the renderer rather than a placeholder standing in for it.
  expect(body).toContain("lone");

  // The lone block ships no source: nothing may recompile it, and re-sending the source beside
  // the SVG would be dead weight. The SLIDESHOW still carries its slides, so only the lone
  // block's own attribute is asserted absent.
  const loneBlock = body.match(/<div class="d2-block"[^>]*>/g) ?? [];
  expect(loneBlock).toHaveLength(1);
  expect(loneBlock[0]).not.toContain("data-source");
});

/**
 * Asserted by SIZE, not by chunk name. Rollup names several unrelated chunks `index.<hash>.js`,
 * so matching the name flags innocent ones; and the thing that actually hurt readers was never a
 * particular filename, it was downloading megabytes. d2 is ~5.9 MB gz and the next-largest asset
 * on any page is Monaco at ~0.85 MB, so a 2 MB ceiling separates them without being brittle.
 */
const MAX_ASSET_BYTES = 2_000_000;

test("the lesson loads without downloading the d2 engine", async ({ page }) => {
  const heavy: string[] = [];
  page.on("response", (res) => {
    const length = Number(res.headers()["content-length"] ?? 0);
    if (length > MAX_ASSET_BYTES) heavy.push(`${res.url()} (${Math.round(length / 1024)} KB)`);
  });

  await page.goto(LESSON);
  await page.waitForLoadState("networkidle");

  // The figure is on screen…
  await expect(page.locator(".d2-block[data-prerendered] svg").first()).toBeVisible();
  // …and the lesson's OTHER d2 figure — the slideshow's first slide — is too, without the engine
  // that would otherwise have to arrive before either could be drawn.
  await expect(page.locator(".diagram--slides svg").first()).toBeVisible();
  expect(heavy).toEqual([]);
});

test("the server-drawn figure still gets the Enlarge affordance", async ({ page }) => {
  await page.goto(LESSON);
  const card = page.locator(".d2-block[data-prerendered]");
  await expect(card).toHaveCount(1);
  // Hydration adopts the existing figure rather than discarding and recompiling it: the SVG is
  // still there after the island mounts, and the zoom control it owns has appeared beside it.
  await expect(card.locator("svg").first()).toBeVisible();
  await expect(card.getByRole("button", { name: "Enlarge diagram" })).toBeVisible();
});
