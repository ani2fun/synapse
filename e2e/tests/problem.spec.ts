import { test, expect } from "./fixtures";

/**
 * The problem-page smoke: the two-pane workbench frame is a page kind of its own — SSR frame
 * plus the extraction island — and until this spec, nothing in CI opened one.
 * The fixture problem (`learn/smoke/problems/threshold/threshold`) is ours and stable, so the
 * path is hardcoded rather than discovered — a rename here is a deliberate edit, not drift.
 *
 * Deliberately NOT exercised: Run/Submit (go-judge is not part of this suite's stack) and the
 * signed-in gates (Keycloak is not either) — the Think canvas's SAVE path therefore lives in
 * `canvas-authed.spec.ts`, and what is asserted here is everything a signed-out reader sees. The fixtures' pageerror guard rides along, so a
 * hydration crash anywhere on the page fails the spec even where no assertion looks.
 */

const PROBLEM = "/synapse/learn/smoke/problems/threshold/threshold";

/** Open the Think tab, having first waited for the island to hydrate.
 *
 *  The tab BUTTONS are server-rendered and inert until `islands/problem` wires them, so a click
 *  sent straight after `goto` lands on markup with no handler and silently does nothing. The
 *  workbench appearing in the Code pane is the signal that the island has run — the same wait the
 *  editorial tab's spec above makes for the same reason. */
async function openThink(page: import("@playwright/test").Page): Promise<void> {
  await page.goto(PROBLEM);
  // Think is where a problem opens, so the canvas appearing IS the hydration signal.
  await expect(page.locator(".pcanvas")).toBeVisible();
}

test("the problem page renders its frame and extracts the workbench", async ({ page }) => {
  await page.goto(PROBLEM);

  // The SSR frame: crumbs visible (not clipped under the fixed header — a past padding
  // regression), the four tabs, the docked nav.
  const crumbs = page.locator(".pwb__crumbs");
  await expect(crumbs).toBeVisible();
  expect((await crumbs.boundingBox())?.y ?? 0).toBeGreaterThan(60);
  await expect(page.locator(".problem-tab")).toHaveCount(4);
  await expect(page.locator(".pwb__nav")).toBeVisible();

  // The extraction island: the FIRST description workbench lands in the right pane, with the
  // toolbar's Run button and the tests panel's case chips. A problem OPENS on Think now, so the
  // editor is one click away — this spec is about the extraction, so it goes and looks. The tab
  // buttons are SSR'd and inert until the island wires them, so wait for it to have run first.
  await expect(page.locator(".pcanvas")).toBeVisible();
  await page.locator(".pwb__rtab--code").click();
  const right = page.locator(".pwb__right");
  await expect(right.locator(".runnable")).toBeVisible();
  await expect(right.locator(".runnable__run")).toBeVisible();
  await expect(right.locator(".wb__chip").first()).toBeVisible();

  // The page itself must not scroll — the panes own all scrolling.
  const overflow = await page.evaluate(
    () => document.documentElement.scrollHeight - window.innerHeight,
  );
  expect(overflow).toBeLessThanOrEqual(0);
});

test("the editorial tab mounts its stepper", async ({ page }) => {
  await page.goto(PROBLEM);
  // Any sign the island has run will do; the canvas is what the right pane opens with.
  await expect(page.locator(".pcanvas")).toBeVisible();

  await page.locator(".problem-tab--editorial").click();
  // The editorial stepper island renders on first open: the pane scroller plus at least one
  // Jump pill.
  await expect(page.locator(".pwb-escroll")).toBeVisible();
  await expect(page.getByRole("button", { name: /intuition/i }).first()).toBeVisible();
});

test("the editorial's solution reveals real code, not an empty box", async ({ page }) => {
  await page.goto(PROBLEM);
  await expect(page.locator(".pcanvas")).toBeVisible();
  await page.locator(".problem-tab--editorial").click();
  await expect(page.locator(".pwb-escroll")).toBeVisible();

  // The code is behind a gate, so nothing is mounted until this is pressed.
  const reveal = page.locator(".pwb-ereveal");
  await expect(reveal).toBeVisible();
  await reveal.click();

  // The assertion that matters is the TEXT, not the chrome. Monaco arrives as a lazy chunk, and
  // when that import fails -- a stale dev-server dep cache, a chunk 404 after a deploy -- the
  // viewer still renders its bar, its language pill and its Copy to editor button, so everything
  // an element-presence check would look at is present and correct. The only symptom is that the
  // editor is empty, and the only other trace is a console error. Assert the source is on screen.
  const solution = page.locator(".pwb-ereveal-open .runnable.solution");
  await expect(solution.locator(".wb__eyebrow")).toContainText("SOLUTION");
  await expect(solution.locator(".view-lines")).toContainText("int(input())", { timeout: 30_000 });
});

test("a slow-loading editor still ends up showing its lines", async ({ page }) => {
  // Coverage for a slow chunk, not a guard on the measure race. The reveal broadcasts RELAYOUT
  // before the lazy editor exists, so the viewer re-measures itself on arrival instead of relying
  // on that event -- but the blank-editor case this protects against only reproduces when the
  // container is genuinely 0x0 at construction, which a delay alone does not force here, and
  // forcing it by hiding the pane would test the listener rather than the arrival. So this pins
  // the honest half: a chunk that takes its time still ends up with lines on screen.
  await page.route(/monaco/i, async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 1200));
    await route.continue();
  });

  await page.goto(PROBLEM);
  await expect(page.locator(".pcanvas")).toBeVisible();
  await page.locator(".problem-tab--editorial").click();
  await expect(page.locator(".pwb-escroll")).toBeVisible();
  await page.locator(".pwb-ereveal").click();

  // Mounted AND measured. An editor that missed its nudge renders no lines at all while every
  // other part of it -- the box, the bar, the language pill -- looks entirely correct.
  const solution = page.locator(".pwb-ereveal-open .runnable.solution");
  await expect(solution.locator(".monaco-editor")).toBeVisible({ timeout: 30_000 });
  await expect(solution.locator(".view-lines")).toContainText("int(input())", { timeout: 30_000 });
});

test("a solution whose editor cannot load still shows its code", async ({ page }) => {
  // The lazy chunk, made unreachable. This is the shape of two real failures: a dev server that
  // has re-optimized its dependencies and answers the open page's stale URLs with 504, and a
  // deploy that retires the hashed chunk a still-open tab is about to ask for.
  await page.route(/monaco/i, (route) => route.abort());

  await page.goto(PROBLEM);
  await expect(page.locator(".pcanvas")).toBeVisible();
  await page.locator(".problem-tab--editorial").click();
  await expect(page.locator(".pwb-escroll")).toBeVisible();
  await page.locator(".pwb-ereveal").click();

  // No editor -- and the code is on screen regardless, in the same box at the same size. The
  // reader loses highlighting and nothing else, which is the whole point of the fallback.
  const solution = page.locator(".pwb-ereveal-open .runnable.solution");
  await expect(solution.locator(".monaco-editor")).toHaveCount(0);
  await expect(solution.locator(".runnable__preview")).toContainText("int(input())", { timeout: 30_000 });
});

/**
 * The solution is its own document in the index, and its result row has to keep the promise it
 * makes. "comparison" is written in the editorial and NOWHERE else in the fixture library — not
 * in the problem statement, not in a title — so a hit on it can only be the walkthrough.
 */
test("a search hit on a solution opens the editorial, not the problem statement", async ({ page }) => {
  await page.goto("/");
  await page.locator(".header__search").click();
  await page.locator(".cmdk__input").fill("comparison");

  // `.first()` because the editorial says "comparison" twice and every occurrence is marked.
  const row = page.locator(".cmdk__result").first();
  await expect(row.locator("mark").first()).toHaveText("comparison");
  // The chip is the spoiler warning: a reader must be able to see this is the answer before
  // clicking, because the row's title is the PROBLEM'S.
  await expect(row.locator(".cmdk__result-kind")).toHaveText("Solution");

  await row.click();
  await expect(page).toHaveURL(`${PROBLEM}#editorial`);
  // Landing on the page is not the promise — landing on the TAB is.
  await expect(page.locator(".problem-tab--editorial")).toHaveClass(/problem-tab--active/);
  await expect(page.locator(".pwb-escroll")).toBeVisible();
});

test("the contents pill opens the book drawer", async ({ page }) => {
  await page.goto(PROBLEM);
  await expect(page.locator(".pcanvas")).toBeVisible();

  await page.locator(".pwb__contents").click();
  // The drawer must be genuinely visible (it once mounted into display:none at desktop width).
  const drawer = page.locator(".reader-nav-drawer");
  await expect(drawer).toBeVisible();
  await expect(drawer.locator("a").first()).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(drawer).toHaveCount(0);
});

// ─────────────────────────────────────────────────────────────────────────────
// THINK — the design canvas beside the code editor
// ─────────────────────────────────────────────────────────────────────────────

test("the right pane opens on Think, with Code one click away", async ({ page }) => {
  await page.goto(PROBLEM);

  await expect(page.locator(".pwb__rtab")).toHaveCount(2);
  // The plan comes before the typing — that is the whole premise of the page.
  await expect(page.locator(".pwb__rtab--think")).toHaveClass(/pwb__rtab--active/);
  await expect(page.locator(".pcanvas")).toBeVisible();
  await expect(page.locator('.pwb__rpane[data-rpane="code"]')).toHaveClass(/hidden/);

  // And the editor is still one click away, with the workbench in ITS pane — not loose in the
  // column.
  await page.locator(".pwb__rtab--code").click();
  await expect(page.locator('.pwb__rpane[data-rpane="code"] .runnable')).toBeVisible();
  await expect(page.locator('.pwb__rpane[data-rpane="think"]')).toHaveClass(/hidden/);

  // The nudge that explains why Think is first.
  await expect(page.locator(".pwb__plan-pill")).toBeVisible();
});

test("the pin remembers which side problems open on", async ({ page }) => {
  await page.goto(PROBLEM);
  await expect(page.locator(".pcanvas")).toBeVisible();

  // Think is the default, so the pin reads as already-set while Think is showing.
  const pin = page.locator("[data-rpin]");
  await expect(pin).toHaveAttribute("aria-pressed", "true");

  // Visiting Code does NOT change the default — a peek is not a preference.
  await page.locator(".pwb__rtab--code").click();
  await expect(pin).toHaveAttribute("aria-pressed", "false");
  await page.reload();
  await expect(page.locator(".pwb__rtab--think")).toHaveClass(/pwb__rtab--active/);

  // Pinning Code does, and it survives a reload.
  await page.locator(".pwb__rtab--code").click();
  await pin.click();
  await expect(pin).toHaveAttribute("aria-pressed", "true");
  await page.reload();
  await expect(page.locator(".pwb__rtab--code")).toHaveClass(/pwb__rtab--active/);
  await expect(page.locator('.pwb__rpane[data-rpane="code"] .runnable')).toBeVisible();
  // Pinned to Code, the canvas is never mounted — an unopened Think costs nothing.
  await expect(page.locator(".pcanvas")).toHaveCount(0);

  // And back again.
  await page.locator(".pwb__rtab--think").click();
  await page.locator("[data-rpin]").click();
  await page.reload();
  await expect(page.locator(".pwb__rtab--think")).toHaveClass(/pwb__rtab--active/);
});

test("Think mounts the canvas with every area of the method", async ({ page }) => {
  await openThink(page);

  // The eight areas, by their headings, in READING order — which is also DOM order, so this
  // doubles as the tab order. The spec block is placed by named grid areas, so the layout can be
  // rearranged without this list changing; a change HERE is a change to the sequence a keyboard
  // walks, which is worth noticing.
  const titles = page.locator(".pcanvas__card-title");
  await expect(titles).toHaveText([
    "Problem",
    "Inputs",
    "Constraints",
    "Maintenance",
    "Return",
    "Error / N/A",
    "Ideas",
    "Tests",
  ]);
  // The two starter ideas, named but empty — the prompt to write a brute force AND a refinement.
  await expect(page.locator(".pcanvas__idea")).toHaveCount(2);
  // A brand-new canvas reads as empty despite those names.
  await expect(page.locator(".pcanvas__meter-label")).toHaveText("0 / 8");

  // Code survives the round trip: the workbench mounts on ITS first open and stays.
  await page.locator(".pwb__rtab--code").click();
  await expect(page.locator('.pwb__rpane[data-rpane="code"] .runnable')).toBeVisible();
  await expect(page.locator('.pwb__rpane[data-rpane="think"]')).toHaveClass(/hidden/);
});

test("every area explains itself, and Constraints links the handout", async ({ page }) => {
  await openThink(page);

  // One info button per area — guidance is not optional on some of them.
  await expect(page.locator(".pcanvas__info")).toHaveCount(8);

  await page.locator('.pcanvas__info[aria-label="What is expected in constraints"]').click();
  const modal = page.locator(".pcanvas__modal");
  await expect(modal).toBeVisible();
  await expect(modal.locator(".pcanvas__modal-title")).toHaveText("Constraints");
  await expect(modal.locator(".pcanvas__modal-list li").first()).toBeVisible();
  // Two ways out, in order: the in-app appendix that covers the area at length, then the outside
  // source it credits. The source link matters as PROVENANCE — a paraphrase with no attribution
  // would be the failure this asserts against.
  const links = modal.locator(".pcanvas__modal-link");
  await expect(links).toHaveCount(2);
  await expect(links.first()).toHaveAttribute(
    "href",
    "/synapse/dsa/appendix/algorithm-design-canvas#2--constraints",
  );
  await expect(links.nth(1)).toHaveAttribute(
    "href",
    "https://www.hiredintech.com/files/the-common-constraints-handout.pdf",
  );

  await page.keyboard.press("Escape");
  await expect(modal).toHaveCount(0);
});

test("a chip plants a line and the meter follows the writing", async ({ page }) => {
  await openThink(page);

  const constraints = page.locator('.pcanvas__field[aria-label="Constraints"]');
  await expect(page.locator(".pcanvas__meter-label")).toHaveText("0 / 8");

  await page.locator(".pcanvas__chip", { hasText: "max N" }).first().click();
  await expect(constraints).toHaveValue("· max N — ");
  // One area filled — and Constraints is one area however many chips land in it.
  await expect(page.locator(".pcanvas__meter-label")).toHaveText("1 / 8");

  await page.locator(".pcanvas__chip", { hasText: "sorted?" }).first().click();
  await expect(constraints).toHaveValue("· max N — \n· sorted? — ");
  await expect(page.locator(".pcanvas__meter-label")).toHaveText("1 / 8");

  // The draft survives a reload — planning that evaporates is worse than no planning surface.
  // The write is debounced, so wait for it to LAND rather than racing the timer: the assertion
  // is about persistence, and a flake here would read as a persistence bug.
  await expect
    .poll(() =>
      page.evaluate(() => Object.keys(localStorage).some((k) => k.startsWith("canvas-draft:"))),
    )
    .toBe(true);

  await page.reload();
  await expect(page.locator('.pcanvas__field[aria-label="Constraints"]')).toHaveValue(
    "· max N — \n· sorted? — ",
  );
  await expect(page.locator(".pcanvas__meter-label")).toHaveText("1 / 8");
});

test("an anonymous reader can plan, but saving asks for a sign-in", async ({ page }) => {
  await openThink(page);

  // The Saved view is honest about why it is empty rather than pretending there is nothing.
  await page.locator(".pcanvas__seg-btn", { hasText: "Saved" }).click();
  await expect(page.locator(".pcanvas__note")).toContainText("Sign in to keep entries");

  await page.locator(".pcanvas__seg-btn", { hasText: "Canvas" }).click();
  await page.locator('.pcanvas__field[aria-label="Problem"]').fill("Is n a palindrome?");
  await page.locator(".pcanvas__btn", { hasText: "Save entry" }).click();
  await expect(page.locator(".pcanvas__toast")).toContainText("Sign in to save an entry");

  // Export stays open to everyone: the draft is theirs, account or not.
  await expect(page.locator(".pcanvas__btn", { hasText: "Export draft" })).toBeEnabled();
});

test("a test case the reader types survives a chip switch and a reload", async ({ page }) => {
  await openThink(page);
  await page.locator(".pwb__rtab--code").click();
  const right = page.locator(".pwb__right");
  await expect(right.locator(".wb__chip").first()).toBeVisible();

  // Append a case of the reader's own and give it a value the authored suite does not hold.
  await right.locator(".wb__chip--add").click();
  const input = right.locator(".wb__input").first();
  await input.fill("777");

  // THE regression this guards: `values` is a scratch buffer that `switchTo` re-seeds from the
  // suite, so before `TestsState.setValue` wrote through, visiting another chip and coming back
  // discarded the value with no reload involved at all.
  await right.locator(".wb__chip", { hasText: "Case 1" }).click();
  await expect(input).toHaveValue("12");
  await right.locator(".wb__chip", { hasText: "Case 3" }).click();
  await expect(input).toHaveValue("777");

  // And it outlives the tab: the suite is persisted per problem (islands/workbench/testsDraft).
  await page.reload();
  await expect(page.locator(".pcanvas")).toBeVisible();
  await page.locator(".pwb__rtab--code").click();
  await expect(right.locator(".wb__chip", { hasText: "Case 3" })).toBeVisible();
  await expect(right.locator(".wb__input").first()).toHaveValue("777");

  // Inputs only. The code buffer is re-minted from the authored starter on every load, so a
  // restored verdict or output would describe a run whose source is gone.
  await expect(right.locator(".runnable__out")).toHaveCount(0);
});

test("a revealed solution can be copied to the clipboard, not just into the editor", async ({ page }) => {
  await page.goto(PROBLEM);
  await expect(page.locator(".pcanvas")).toBeVisible();
  await page.locator(".problem-tab--editorial").click();
  await page.locator(".pwb-ereveal").click();

  // The clipboard button lives INSIDE the editor box (where the hover-reveal rule applies and
  // where the workbench puts its own), and is a different verb from "Copy to editor" beside the
  // language pill — that one loads the solution into the workbench tab.
  const solution = page.locator(".pwb-ereveal-open .runnable.solution");
  await expect(solution.locator(".runnable__editor .editor-copy")).toBeVisible();
  await expect(solution.locator(".wb__ghost", { hasText: "Copy to editor" })).toBeVisible();
});

test("an anonymous reader sees a disabled Edit and a buffer that takes no typing", async ({ page }) => {
  await openThink(page);
  await page.locator(".pwb__rtab--code").click();
  const bench = page.locator(".pwb__right .runnable");
  await expect(bench).toBeVisible();

  // The lock is shown to the reader it applies to, with the tooltip saying why. Reset is there
  // for everyone and idle while the buffer still matches the starter.
  const edit = bench.locator(".wb__actions .wb__ghost", { hasText: "Edit" });
  await expect(edit).toBeVisible();
  await expect(edit).toBeDisabled();
  const reset = bench.locator('button[aria-label="Reset"]');
  await expect(reset).toBeVisible();
  await expect(reset).toBeDisabled();

  // And the editor itself is read-only — typing is swallowed, not applied.
  const lines = bench.locator(".view-lines");
  await expect(lines).toBeVisible({ timeout: 30_000 });
  await lines.click();
  await page.keyboard.type("zzz-anon");
  await expect(lines).not.toContainText("zzz-anon");
});

test("an anonymous reader can still load a solution into the editor, and Reset takes it back", async ({ page }) => {
  await page.goto(PROBLEM);
  await expect(page.locator(".pcanvas")).toBeVisible();
  await page.locator(".pwb__rtab--code").click();
  const bench = page.locator(".pwb__right .runnable");
  const lines = bench.locator(".view-lines");
  await expect(lines).toBeVisible({ timeout: 30_000 });
  const reset = bench.locator('button[aria-label="Reset"]');
  await expect(reset).toBeDisabled();

  // The lock stops TYPING, and only typing: "Copy to editor" replaces the buffer through
  // LOAD_CODE, for anyone. Which is exactly why Reset cannot be gated on sign-in.
  await page.locator(".problem-tab--editorial").click();
  await page.locator(".pwb-ereveal").click();
  const solution = page.locator(".pwb-ereveal-open .runnable.solution");
  await solution.locator(".wb__ghost", { hasText: "Copy to editor" }).click();
  // The fixture solution carries a comment the starter does not, so the buffer is now dirty.
  await expect(lines).toContainText("threshold is inclusive", { timeout: 30_000 });
  await expect(reset).toBeEnabled();

  await reset.click();
  await expect(reset).toBeDisabled();
});
