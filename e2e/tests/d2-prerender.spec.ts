import { expect, test } from "@playwright/test";

/**
 * Where a lone ```d2 fence is drawn, asserted against WHICHEVER mode the stack is running.
 *
 * Both modes ship, and both are reached in production. The cluster runs pre-rendering ON — it is
 * a file lookup, not a compile — but every fence CI has not drawn yet falls back to the client,
 * so the fallback is not a hypothetical configuration. `dev-tools/e2e` runs its main pass with
 * the switch off, measuring the budget against that heavier path, and re-runs this file alone
 * with it on, which is what the cluster serves.
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

/** The walkthrough's opening tag. A different element in BOTH modes, deliberately: the fence
 *  chooses it, not the lookup, so a repo with nothing drawn still mounts the board viewer. */
function boardsTag(body: string): string {
  const tags = body.match(/<div class="d2-boards"[^>]*>/g) ?? [];
  expect(tags).toHaveLength(1);
  return tags[0]!;
}

test.describe("server-drawn (SYNAPSE_D2_PRERENDER=on)", () => {
  test.skip(!PRERENDER, "stack is running the fallback, with pre-rendering off");

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

  test("a walkthrough arrives with its ROOT board drawn, and only that one", async ({ request }) => {
    const body = await (await request.get(LESSON)).text();
    const tag = boardsTag(body);
    expect(tag).toContain('data-prerendered="1"');
    expect(tag).toContain("data-boards="); // the graph the viewer navigates by
    expect(tag).toContain("data-fence=");
    expect(tag).not.toContain("data-source"); // nothing may recompile a drawn walkthrough

    // The other boards are a click away and the reader may never take it.
    expect(body).not.toContain("The handler");
  });

  test("drilling through two boards never pulls the engine", async ({ page }) => {
    // The assertion that carries the whole design: navigation is fetching one small pre-drawn
    // file, not compiling. If that ever regresses, this is where it shows.
    const heavy: string[] = [];
    page.on("response", (res) => {
      const length = Number(res.headers()["content-length"] ?? 0);
      if (length > MAX_ASSET_BYTES) heavy.push(`${res.url()} (${Math.round(length / 1024)} KB)`);
    });

    await page.goto(LESSON);
    const card = page.locator(".diagram--boards");
    await expect(card.locator(".diagram__figure svg").first()).toBeVisible();
    await expect(card.locator(".boards-bar__here")).toHaveText("Context");

    // Each board links to the next: Context → Inside → Deeper, clicked exactly as a reader does.
    await card.locator(".diagram__figure a").first().click();
    await expect(card.locator(".boards-bar__here")).toHaveText("Inside");
    await card.locator(".diagram__figure a").first().click();
    await expect(card.locator(".boards-bar__here")).toHaveText("Deeper");

    // Shareable, but the page's own Back is untouched — the diagram never pushes history.
    // RETRYING, because the URL is written in an effect and therefore lands a tick after the
    // breadcrumb above: reading page.url() synchronously races a frame it has no reason to win.
    await expect(page).toHaveURL(/[?&]board=deeper/);
    await page.waitForLoadState("networkidle");
    expect(heavy).toEqual([]);
  });

  test("back, forward and home walk the boards", async ({ page }) => {
    await page.goto(LESSON);
    const card = page.locator(".diagram--boards");
    await expect(card.locator(".diagram__figure svg").first()).toBeVisible();
    await card.locator(".diagram__figure a").first().click();
    await expect(card.locator(".boards-bar__here")).toHaveText("Inside");

    await card.getByRole("button", { name: "Back" }).click();
    await expect(card.locator(".boards-bar__here")).toHaveText("Context");
    await card.getByRole("button", { name: "Forward" }).click();
    await expect(card.locator(".boards-bar__here")).toHaveText("Inside");
    await card.getByRole("button", { name: "Root board" }).click();
    await expect(card.locator(".boards-bar__here")).toHaveText("Context");

    // The root board drops the parameter, so an unopened diagram has the bare lesson URL.
    await expect(page).not.toHaveURL(/[?&]board=/);
  });

  test("a deep link opens the board it names", async ({ page }) => {
    await page.goto(`${LESSON}?board=deeper`);
    const card = page.locator(".diagram--boards");
    await expect(card.locator(".boards-bar__here")).toHaveText("Deeper");
    // The breadcrumb still reads from the root, so the reader knows where they landed.
    await expect(card.locator(".boards-bar__crumb").first()).toHaveText("Context");
  });

  test("the walkthrough stays navigable inside the Enlarge overlay", async ({ page }) => {
    // The house affordance is the point: enlarging must not turn the viewer into a picture.
    await page.goto(LESSON);
    const card = page.locator(".diagram--boards");
    await expect(card.locator(".diagram__figure svg").first()).toBeVisible();
    await card.getByRole("button", { name: "Enlarge diagram" }).click();

    const overlay = page.getByRole("dialog");
    await expect(overlay).toBeVisible();
    await expect(overlay.locator(".diagram-zoom__chrome .boards-bar__here")).toHaveText("Context");
    await overlay.locator(".diagram-zoom__figure a").first().click();
    await expect(overlay.locator(".diagram-zoom__chrome .boards-bar__here")).toHaveText("Inside");

    await page.keyboard.press("Escape");
    await expect(overlay).toHaveCount(0);
    // The card behind it followed the same navigation — one state, two views of it.
    await expect(card.locator(".boards-bar__here")).toHaveText("Inside");
  });

  test("the board menu jumps straight to any board", async ({ page }) => {
    await page.goto(LESSON);
    const card = page.locator(".diagram--boards");
    await expect(card.locator(".diagram__figure svg").first()).toBeVisible();
    await card.getByRole("button", { name: "Jump to a board" }).click();
    await card.getByRole("option", { name: "Deeper" }).click();
    await expect(card.locator(".boards-bar__here")).toHaveText("Deeper");
  });
});

test.describe("client-drawn (the fallback)", () => {
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

  test("a walkthrough ships its SOURCE and still mounts the board viewer", async ({ request }) => {
    const body = await (await request.get(LESSON)).text();
    const tag = boardsTag(body);

    // The rule this whole family turns on: the ELEMENT comes from the fence, the FIGURE from the
    // lookup. Were it the other way round, every undrawn repo would quietly serve a root board
    // with dead links — the exact bug the walkthrough replaces.
    expect(tag).toContain("data-source");
    expect(tag).toContain("data-meta");
    expect(tag).not.toContain("data-prerendered");
    expect(tag).not.toContain("data-boards=");
  });
});
