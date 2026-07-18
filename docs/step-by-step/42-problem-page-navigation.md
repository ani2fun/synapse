# Step 42 — Getting out of a problem page

*(a user-reported gap: a problem page was a room with one door — and fixing it surfaced a
drawer bug that had been shipping since step 33.)*

## The gap

Steps 37 and 39 deliberately gave problem pages the whole viewport: no sidebar column, no
1600px cap, TOC and reading-preferences chrome suppressed. The two-pane workbench earns every
pixel, and that decision still stands.

What went with it, unnoticed, was every route onward. A prose lesson has a sidebar and a pair
of pager cards at the bottom; a problem page had neither — no next problem, no previous one,
no book contents. The only way out was the `Home` breadcrumb, back to the library, and then
down through the tree again. On a book like DSA, where problems come in runs of twenty-two,
that is the difference between practising and navigating.

This is not a regression — nothing broke — but the design was incomplete, which from the
reader's seat is the same thing.

## Where the navigation went

Not back into a sidebar column: the full-viewport layout is worth keeping. Both jobs move into
the **breadcrumb row**, which was carrying three words and a lot of empty space.

```
HOME › DSA › Pattern 2                    [⊞ CONTENTS]  [‹ PREV]  [NEXT ›]
```

**Prev/next** needed no new data. `payload.prev` / `payload.next` have ridden on the lesson DTO
since the reader's pager cards were built — the problem page simply never spent them. They walk
the book's whole reading order, so `pattern-01`'s Prev correctly crosses the chapter boundary
into the previous chapter's last lesson.

**Contents** opens the reader's existing nav drawer — the same off-canvas panel the mobile FAB
has driven since step 33, with the same `ReaderSidebar` inside it, current lesson marked and
chapters expanded. Nothing new was built; the drawer just gained a second caller.

That made the open state shared, so it moved from a local signal into `ChromeState`, which both
components already reach through context. `.reader-nav--pinned` (set from `chrome.is_problem`)
keeps the drawer reachable above the 1024px breakpoint, where it is normally hidden because the
sidebar column takes over — except on problem pages, which have no column at any width.

The FAB stays mobile-only. A floating button over the left pane's content is the wrong
affordance when there is a bar with room in it; below the breakpoint the reverse is true, so the
crumb-row Contents button stands down there and the row wraps rather than clipping prev/next off
the right edge.

## The bug underneath

Opening the drawer at desktop width showed its head — `CONTENTS` and a ✕ — sitting *under* the
fixed site header. The ✕ was not merely hidden: `elementFromPoint` at its centre returned
`header__mid`. It had been unclickable since step 33, on every phone, closable only by scrim,
Escape, or tapping a link. Enabling the drawer at a second breakpoint is just what finally put
eyes on it.

The first fix was `top: 65px` — the offset `.reader-sidebar` and `.reader-sticky` already use.
It worked at desktop and failed on a phone, where the header wraps to about 93px. Chasing a
header height that changes with the viewport is the wrong shape of fix.

The drawer now goes **above** the header instead (z-index 56, scrim 55, versus the header's 50)
and returns to `top: 0`. Taking over the viewport is what an off-canvas drawer is supposed to
do, it cannot drift out of sync with the header's height, and it stays under the two rungs that
must outrank it — the diagram scrim at 70 and ⌘K at 300.

## Verified live

At 1280px on `dsa/logic-building-pattern/pattern-02`: Contents, ‹ Prev and Next › in the crumb
row; Next navigated to Pattern 3 with the workbench remounted and the nav re-pointed at
pattern-02/pattern-04; the drawer opened with all 33 DSA entries, Pattern 02 expanded and
Pattern 2 marked current; ✕, scrim and Escape all closed it; the FAB stayed hidden. At 375px:
crumbs wrapped, prev/next fully visible, Contents stood down, the FAB opened the same drawer and
its ✕ was reachable for the first time. On a prose lesson at 1280px: 280px sidebar, no FAB,
pager cards intact — untouched.

379 rust + 74 vitest. (Step 41's 371 was correct for step 41 — the extra eight arrived with
`fix(submission)`, which landed between the two steps and is unrelated to either.)

## The lesson

**A layout decision has a second half.** Removing the sidebar from problem pages was right, and
the review at the time checked that the two panes looked correct — which they did. What no one
asked was where the things the sidebar had been carrying were supposed to go. The chrome you
delete is doing jobs beyond the one you deleted it for; those jobs do not disappear with it.
