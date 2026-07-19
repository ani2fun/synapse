# Step 46 — The mobile reader

*(a bug report that named the wrong cause, and the one missing line underneath it.)*

## What was reported

Two screenshots from a real iPhone 15 in portrait. Dead space down the right side, with a note:
*"we should have left the consistent spacing on left and right side to use maximum space."* Then
a second, taken after a double-tap zoom, showing the right-edge buttons clipped and text running
off the screen.

Three symptoms, which read like three bugs: asymmetric padding, misplaced buttons, and something
wrong with zoom.

## What was actually wrong

Measured against production at 393px:

```
viewport              393px
document.scrollWidth  554px      ← 161px of horizontal overflow
prose column          313px, symmetric — 40px each side
every right-edge FAB  fromRight: -141px   ← 141px BEYOND the viewport
```

**The padding was symmetric to the pixel.** There was nothing to even out. The page was simply
161px wider than the screen, so pinch-zooming out revealed a correctly-centred column sitting in
a layout that extended past it — which looks exactly like wasted space on the right.

The buttons followed from the same fact. `position: fixed; right: 20px` resolves against the
**layout** viewport, and the overflow had stretched that to 554px. The rail was positioned
perfectly against a width the screen did not have.

One cause, both screenshots. Fixing what was described — evening up padding, nudging buttons —
would have fixed neither.

## Finding it

Hiding the tables on the page dropped `scrollWidth` from 554 to exactly 393. A scan across five
pages confirmed the shape: the two with tables overflowed (+161px, +55px); the three without
measured exactly 393px.

`.synapse-prose table` was `border-collapse` and `margin` and nothing else — no width constraint
at all, so a four-column table sized to its content at 514px inside a 313px column.

But tables were only the visible symptom. Underneath sat the real enabler, and it is a single
line:

**There was no `box-sizing: border-box` reset anywhere in `client/styles/`.** Verified live by
injecting a probe: a `width: 100%` element with `padding: 0 1rem` measured **347px inside a 313px
parent**. Three shipped rules were doing precisely that — `.diagram__figure`, `.diagram-error`
and `.cmdk__input`. Every lesson with a diagram was wider than the phone; the ⌘K input overflowed
its own palette by 32px.

That is why the fix is a reset rather than three patches. Without it, the next `width: 100%` rule
anyone writes reintroduces the bug, and the failure surfaces somewhere unrelated — as a
misplaced button, most likely, which is where this started.

## The changes

**Overflow.** `box-sizing: border-box` joins the universal reset in `tokens.css`, beside the
`border-color` line that has been load-bearing since step 25. Tables get the standard scroll
idiom (`display: block; width: max-content; max-width: 100%; overflow-x: auto`) so they scroll
inside the column instead of widening the page. `.synapse-prose` gains `overflow-wrap:
break-word` — prose had **no** wrapping assistance of any kind before this step; `overflow-wrap`,
`word-break` and `hyphens` appeared zero times in the entire styles directory.

**A dead rule.** `.reader-prose iframe[src^="/c4/"]` matched nothing — the prose container is
`lesson-body synapse-prose`, and the only `reader-prose*` classes are the BEM children
`__back`/`__title`/`__lede`. So the width cap on LikeC4 embeds had never applied and an authored
`width="800"` iframe ran straight off a phone. Retargeted to `.c4-embed iframe`, which `c4.rs`
actually creates.

**Gutters.** `reader.css` had no rule below 768px — it inherited desktop gutters all the way down
to a 313px column. A 640px block (640 is already the de-facto mobile breakpoint in `shell.css`,
`practice.css` and `library.css`; `reader.css` was the outlier) takes the padding to 14px a side,
returning ~24px of line length. The pager stops being two columns sharing 150px each.

**Two real asymmetries**, both independent of the phone:

- `.lesson` set a `max-width` and was never centred. Wherever the cap binds (≈872px up) the
  column sat flush left and *all* slack piled onto the right — genuinely the reported bug, just
  on a tablet or desktop rather than a phone. `margin-inline: auto`.
- `reader.css` had two identical selectors back to back, the second existing only to add 8px on
  the right of a problem page. Collapsed to one symmetric rule.

**Justification.** `text-align: justify` with `hyphens: auto`, below the breakpoint only. The two
go together or not at all: at 313px, justify alone opens rivers of white space through words like
"Composition" and "aggregation". `<html lang="en">` was already set, so the browser has a
dictionary and `auto` engages. Headings, table cells and code opt back out — a hyphenated heading
reads as damage.

## The rail, and a correction

The plan said to move the drawer FAB from bottom-left into the right-hand column, and explicitly
argued against reserving a gutter for it: *"the buttons are small, and they overlap only the
bottom of the column."*

That was wrong, and the screenshot after the first attempt showed it immediately. Five stacked
44px buttons is **288px of an 852px screen**, and once the prose reclaimed the full width it ran
straight underneath them — words obscured mid-sentence, "cate**⬤**rized". Worse than the problem
being solved.

A vertical rail on the right will always collide with full-width text. The choice is a gutter or
a different axis, and a gutter costs 44+12px — about 14% of a 393px screen, more than this step
had just won back.

So on a phone **the rail turns 90°**: a row along the bottom edge, 38px buttons on a 46px pitch,
ordered right-to-left by how often a thumb wants them — Contents · TOC · Aa · top · focus. It
costs no width and ~50px of height at the very bottom, where phone controls belong. The overlap
zone went from 288px of live reading area to 50px at the edge the eye is not reading.

## What this deliberately does not do

**No "mobile view" preference**, though one was suggested. `prefs.rs` parses with
`let [s, l, f, w] = parts.as_slice() else { return DEFAULT_PREFS }` — a fifth field would make
every existing four-part stored string hit the else branch and reset **all four** of a reader's
saved settings, violating the module's own per-field degradation contract. That migration is
worth paying when a preference earns it. Paying it so a phone can render a readable page is the
wrong trade: a setting the reader must discover to get a working layout is a workaround for a
layout that is not right yet.

**No `visualViewport` chasing.** iOS pins `position: fixed` to the layout viewport by design.
With the overflow gone the rail lands where it should; the residual drift under active pinch-zoom
is browser behaviour, not a bug to engineer around.

## Verified

At 393×852 on the page from the screenshot:

```
scrollWidth   554 → 393    (overflow 161px → 0)
FABs         -141 → +12    all five, one row
prose column  313 → 337px  symmetric 28px
table                      scrolls inside, page does not
```

Six surfaces at 393px — lessons with tables and with diagrams, a problem page, the viz gallery,
the landing page, the blog — all report `scrollWidth` exactly equal to the viewport, with zero
leftover offenders. At 1280px the `.lesson` fix measures 48px of slack on **each** side rather
than 96px on the right, diagrams render border-box at exactly the column width, the ⌘K input now
fits its palette, and prose stays left-aligned (justification is correctly phone-only).

386 rust + 74 vitest. Critical path 637/700 KiB gz.

## The lesson

**A symptom description is a hypothesis, not evidence.** "Space on the right" names a padding
bug, and the padding was symmetric to the pixel. Had the report been taken at face value, the fix
would have been to unbalance correct padding until the screenshot looked right — leaving the
overflow, the off-screen rail, and the missing `box-sizing` in place, and making the layout
quietly wrong everywhere else. The first useful act was measuring the thing being complained
about rather than the thing being blamed.
