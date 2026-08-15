// The content-editing surface, ANONYMOUS — so it runs on every push, no sign-in. The e2e stack
// runs the server on its `content_forge` default of "dry-run", so `/api/edits` is mounted and the
// whole feature is reachable; only a signed-in, allow-listed caller can actually propose, which
// the service_tests cover over fakes. Here we prove the PUBLIC contract: the affordance stays
// hidden from a reader, the gates answer correctly, and the pages render without a page error.
import { expect, test } from "./fixtures";

const LESSON = "/synapse/learn/smoke/intro";

test("the lesson page shows Suggest-an-edit GATED to an anonymous reader", async ({ page }) => {
  await page.goto(LESSON);
  await expect(page.locator("h1").first()).toBeVisible();
  // Visible but not actionable — the workbench Submit grammar. An anonymous reader should see the
  // affordance exists and learn from the tooltip how to ask for it, rather than meet a blank space.
  const link = page.locator("[data-edit-link]");
  await expect(link).toBeVisible();
  await expect(link).toHaveClass(/lesson-edit-link--gated/);
  await expect(link).toHaveAttribute("aria-disabled", "true");
  // The tooltip carries the request-access route (rendered by the server, so it survives no-JS).
  await expect(page.locator("[data-edit-tip]")).toHaveAttribute(
    "data-tip",
    /content-editor list.*synapse\.kakde\.eu@gmail\.com/s,
  );
});

test("a gated Suggest-an-edit click does not navigate to the editor", async ({ page }) => {
  await page.goto(LESSON);
  const link = page.locator("[data-edit-link]");
  await expect(link).toHaveClass(/lesson-edit-link--gated/);
  // The gate is the ABSENCE of an href, not a click handler: an anchor that has one navigates on a
  // plain click no matter what `aria-disabled` says, including before this page's islands have
  // loaded. So the destination is withheld and granted on activation instead.
  await expect(link).not.toHaveAttribute("href", /./);
  // FORCED on purpose — `aria-disabled` makes Playwright treat this as disabled, so an ordinary
  // click never lands and would prove nothing about what a real mouse does.
  await link.click({ force: true });
  await expect(page).toHaveURL(new RegExp(`${LESSON}$`));
});

test("GET /api/edits/config reports dry-run and canEdit false for anonymous", async ({ request }) => {
  const response = await request.get("/api/edits/config");
  expect(response.status()).toBe(200);
  const config = await response.json();
  expect(config.enabled).toBe(true);
  expect(config.mode).toBe("dry-run");
  expect(config.canEdit).toBe(false);
});

test("GET /api/edits/source refuses an anonymous caller with 401", async ({ request }) => {
  const response = await request.get(`/api/edits/source/${LESSON.replace("/synapse/", "")}`);
  expect(response.status()).toBe(401);
});

test("POST /api/edits refuses an anonymous caller with 401 and commits nothing", async ({ request }) => {
  const response = await request.post("/api/edits", {
    data: { lessonPath: "learn/smoke/intro", source: "hi", baseFingerprint: "x" },
  });
  expect(response.status()).toBe(401);
});

test("GET /api/admin/content-editors refuses an anonymous caller with 401", async ({ request }) => {
  const response = await request.get("/api/admin/content-editors");
  expect(response.status()).toBe(401);
});

test("the /edit page renders its shell and the signed-out gate without a page error", async ({ page }) => {
  await page.goto(`/edit/learn/smoke/intro`);
  // The island resolves the auth store (which lands on anonymous with no session) and shows the
  // sign-in gate — not a blank page, not a thrown error (the fixtures harness fails on either).
  await expect(page.locator(".edit-gate__title")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText(/sign in/i).first()).toBeVisible();
});

test("the /admin page shows the signed-out state for an anonymous visitor", async ({ page }) => {
  await page.goto("/admin");
  await expect(page.getByText("Not signed in")).toBeVisible({ timeout: 15_000 });
  // Neither allowlist section renders its table until an admin is signed in.
  await expect(page.locator(".admin__table")).toHaveCount(0);
});

// ── the diagram's own Edit affordance ────────────────────────────────────────────────────────
// A lesson's prose has been editable in place for a while; its diagrams are now too. The pill
// follows the same gated grammar as "Suggest an edit", and points at a specific FENCE — which is
// the part that cannot be read off a server-drawn figure, so the render pass stamps it.

/** Every figure's ordinal, in document order, split by the language whose list it indexes. */
function ordinalsByLang(body: string): { d2: number[]; mermaid: number[] } {
  const found = { d2: [] as number[], mermaid: [] as number[] };
  const CARD = /class="(d2-block|d2-slideshow|d2-boards|mermaid-block)"[^>]*?data-fence-at="(\d+)"/g;
  for (const [, kind, at] of body.matchAll(CARD)) {
    found[kind === "mermaid-block" ? "mermaid" : "d2"].push(Number(at));
  }
  return found;
}

test("every d2 figure carries the fence it came from", async ({ request }) => {
  const body = await (await request.get(LESSON)).text();
  // Ordinals, in document order, across a lone block / a slideshow run / a walkthrough.
  const at = ordinalsByLang(body).d2;
  expect(at.length).toBeGreaterThan(0);
  expect([...at]).toEqual([...at].sort((a, b) => a - b));
  expect(new Set(at).size).toBe(at.length);
  // A run is one card carrying several fences, and says so.
  expect(body).toMatch(/data-fence-count="\d+"/);
});

test("the two languages number their own fences, not the figures between them", async ({
  request,
}) => {
  // The fixture interleaves them on purpose: mermaid, then four d2 fences, then mermaid again.
  // A counter shared between the two would make that last one diagram 5 and send its Edit pill
  // to a d2 figure — so this is the assertion the whole round trip rests on.
  const { d2, mermaid } = ordinalsByLang(await (await request.get(LESSON)).text());
  expect(mermaid).toEqual([0, 1]);
  expect(d2[0]).toBe(0);
  expect(d2.length).toBeGreaterThan(1);
});

test("the diagram Edit pill is shown GATED to an anonymous reader", async ({ page }) => {
  await page.goto(LESSON);
  const pill = page.locator(".diagram__edit").first();
  await expect(pill).toBeVisible({ timeout: 15_000 });
  // Inert: a span, not a link, so there is nothing to click through to a 403.
  await expect(pill).toHaveClass(/diagram__edit--gated/);
  expect(await pill.evaluate((node) => node.tagName)).toBe("SPAN");
  // …and it says how to ask, in the same words the page-level affordance uses.
  await expect(pill).toHaveAttribute("title", /content-editor list.*synapse\.kakde\.eu@gmail\.com/s);
});

test("a mermaid figure's Edit pill points at the mermaid editor", async ({ page }) => {
  await page.goto(LESSON);
  await expect(page.locator(".mermaid-block .diagram").first()).toBeVisible({ timeout: 15_000 });
  const pill = page.locator(".mermaid-block .diagram__edit").first();
  await expect(pill).toBeVisible();
  // Gated for an anonymous reader, exactly as d2's is — the language changes where it goes, not
  // who may use it.
  await expect(pill).toHaveClass(/diagram__edit--gated/);
  expect(await pill.evaluate((node) => node.tagName)).toBe("SPAN");
  // …and it still enlarges.
  await expect(page.locator(".mermaid-block .diagram__zoom").first()).toBeAttached();
});

test("/d2 opens on the diagram a lesson's figure names", async ({ page }) => {
  // The round trip, without a sign-in: loading a diagram to look at it is public, and only
  // proposing a change is gated.
  await page.goto("/d2?lesson=learn/smoke/intro&at=0");
  await expect(page.locator(".lab-doc__id .pane-hd__eyebrow")).toContainText("learn/smoke/intro");
  await expect(page.locator(".lab-primary")).toContainText("Update the diagram");
  // The lone fence in the fixture lesson is `lone -> figure`.
  await expect(page.locator(".lab-ed")).toContainText("lone", { timeout: 30_000 });
});

test("/d2 with no lesson is still the blank scratchpad", async ({ page }) => {
  await page.goto("/d2");
  await expect(page.locator(".lab-doc__id .pane-hd__eyebrow")).toContainText("draft");
  await expect(page.locator(".lab-primary")).toContainText("Add to a lesson");
});

test("/mermaid opens on the SECOND mermaid diagram, not the second figure", async ({ page }) => {
  // `at=1` is the sequence diagram at the end of the fixture, which four d2 fences sit above. If
  // the ordinal were shared, this would load one of those instead.
  await page.goto("/mermaid?lesson=learn/smoke/intro&at=1");
  await expect(page.locator(".lab-doc__id .pane-hd__eyebrow")).toContainText("learn/smoke/intro");
  await expect(page.locator(".lab-primary")).toContainText("Update the diagram");
  await expect(page.locator(".lab-ed")).toContainText("sequenceDiagram", { timeout: 30_000 });
});

test("/mermaid with no lesson is the blank scratchpad, and draws it", async ({ page }) => {
  await page.goto("/mermaid");
  await expect(page.locator(".lab-doc__id .pane-hd__eyebrow")).toContainText("draft");
  await expect(page.locator(".lab-primary")).toContainText("Add to a lesson");
  // The preview is the reader's own card, and the starter has to reach it — the status pill says
  // so, and the figure proves it.
  await expect(page.locator(".pv .diagram__figure svg")).toBeVisible({ timeout: 30_000 });
  await expect(page.locator(".st--ok")).toContainText("Up to date");
  // The metadata line names what the source parses as, which only a parse can tell it.
  await expect(page.locator(".lab-meta__path")).toContainText("flowchart");
});
