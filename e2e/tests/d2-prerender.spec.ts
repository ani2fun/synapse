import { expect, test } from "@playwright/test";

/**
 * Where a lone ```d2 fence is drawn, asserted against WHICHEVER mode the stack is running.
 *
 * Both modes ship. Production disables pre-rendering — one 23-diagram lesson peaks the sidecar
 * at 5.2 GB against a 256Mi limit — so `dev-tools/e2e` runs its main pass with the switch off,
 * the shape the cluster serves, and re-runs this file alone with it on so the feature cannot rot
 * while it waits for somewhere to run.
 *
 * The point of splitting it this way: each mode asserts the other's markers are ABSENT. A spec
 * that only checked the happy path would stay green if pre-rendering silently stopped — which is
 * exactly how this failure behaves, because a failed pre-render falls back to the client and
 * renders a perfectly good page.
 */

const LESSON = "/synapse/learn/smoke/intro";
const PRERENDER = (process.env.SYNAPSE_D2_PRERENDER ?? "off").toLowerCase() === "on";

/**
 * Asserted by SIZE, not by chunk name. Rollup names several unrelated chunks `index.<hash>.js`,
 * so matching the name flags innocent ones; and the thing that hurt readers was never a filename,
 * it was downloading megabytes. d2 is ~5.9 MB gz and the next-largest asset on any page is Monaco
 * at ~0.85 MB, so a 2 MB ceiling separates them without being brittle.
 */
const MAX_ASSET_BYTES = 2_000_000;

/** The lone `.d2-block`'s opening tag — the slideshow's is a different element. */
function loneBlockTag(body: string): string {
  const tags = body.match(/<div class="d2-block"[^>]*>/g) ?? [];
  expect(tags).toHaveLength(1);
  return tags[0]!;
}

test.describe("server-drawn (SYNAPSE_D2_PRERENDER=on)", () => {
  test.skip(!PRERENDER, "stack is running the prod shape, with pre-rendering off");

  test("a lone d2 fence arrives as SVG in the HTML, not as source for the client", async ({
    request,
  }) => {
    const body = await (await request.get(LESSON)).text();
    expect(body).toContain('data-prerendered="1"');
    expect(body).toMatch(/<div class="diagram__figure"><svg/);
    expect(body).toContain("lone"); // the authored content reached the renderer

    // No source ships beside the SVG: nothing may recompile it, and it would be dead weight.
    expect(loneBlockTag(body)).not.toContain("data-source");
  });

  test("the lesson loads without downloading the d2 engine", async ({ page }) => {
    const heavy: string[] = [];
    page.on("response", (res) => {
      const length = Number(res.headers()["content-length"] ?? 0);
      if (length > MAX_ASSET_BYTES) heavy.push(`${res.url()} (${Math.round(length / 1024)} KB)`);
    });

    await page.goto(LESSON);
    await page.waitForLoadState("networkidle");

    // Both figures are on screen — the lone block and the slideshow's first slide — without the
    // engine that would otherwise have to arrive before either could be drawn.
    await expect(page.locator(".d2-block[data-prerendered] svg").first()).toBeVisible();
    await expect(page.locator(".diagram--slides svg").first()).toBeVisible();
    expect(heavy).toEqual([]);
  });

  test("the server-drawn figure still gets the Enlarge affordance", async ({ page }) => {
    await page.goto(LESSON);
    const card = page.locator(".d2-block[data-prerendered]");
    await expect(card).toHaveCount(1);
    // Hydration ADOPTS the existing figure rather than discarding and recompiling it: the SVG
    // survives the island mounting, and the zoom control it owns appears beside it.
    await expect(card.locator("svg").first()).toBeVisible();
    await expect(card.getByRole("button", { name: "Enlarge diagram" })).toBeVisible();
  });
});

test.describe("client-drawn (the prod shape)", () => {
  test.skip(PRERENDER, "stack is running with pre-rendering on");

  test("a lone d2 fence ships its SOURCE, and no figure is drawn server-side", async ({
    request,
  }) => {
    const body = await (await request.get(LESSON)).text();

    // The fallback contract, stated positively so it cannot pass vacuously: source present,
    // and every marker of the server-drawn shape absent.
    expect(loneBlockTag(body)).toContain("data-source");
    expect(body).not.toContain("data-prerendered");
    expect(body).not.toMatch(/<div class="diagram__figure"><svg/);
  });

  test("the placeholder still hydrates into a diagram card", async ({ page }) => {
    await page.goto(LESSON);
    // Cheap and stable: the card and its chrome exist as soon as the island mounts. Waiting on
    // the figure would mean waiting on a multi-megabyte engine — which is the cost this mode has
    // and the reason the other one exists — so it is deliberately not gated on here.
    const card = page.locator(".d2-block .diagram");
    await expect(card).toHaveCount(1);
  });
});
