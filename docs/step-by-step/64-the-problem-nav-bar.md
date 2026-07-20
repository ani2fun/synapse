# Step 64 — The problem navigation, docked

*(third time on these three controls, and the first time they stop covering the thing they help
you leave.)*

## Where they had been

Step 42 put Contents, Prev and Next in the crumb row's top-right, because that is where the empty
space was. Step 43 moved them to floating pills in the bottom corners, because empty space is not
reachable space and the top-right is the furthest point from both eye and mouse.

Both iterations shared an assumption worth naming: that these controls had to be *placed*
somewhere, over the top of a layout that was already full. Fixed positioning is what you reach for
when you have decided not to spend any height. The cost was that three pills sat permanently on top
of the panes — a page you work top-to-bottom, with a control hovering over the last paragraph of
it.

So they are a bar now: `nav.pwb__nav`, the last flex child of `.pwb`, sitting under the panes the
way the crumb row sits over them. `.pwb__panes` is already `flex: 1 1 auto; min-height: 0`, so it
gives up the height with no arithmetic anywhere. The bar's three cells are `flex: 1 1 0` with
`justify-content: flex-start / center / flex-end` — sized off the leftover space instead, the
counter would drift off true centre, because Contents and the two step buttons are different
widths.

## `kind` had to reach the index, and nearly broke it

"Problem 5 / 6" needs to know which lessons in a chapter are problems. `kind` lives on the lesson
*payload* — one lesson at a time — and the catalog index that the client already holds carried
`slug`, `title`, `order`, `essential` and nothing else. Counting a chapter's problems from payloads
would mean fetching every lesson in it.

`frontmatter::parse` already read `kind`, so `extract_kind` is a five-line sibling of
`extract_summary`; the chain from there is `Lesson.kind` → the walker → `LessonDto`. The field is
`skip_serializing_if = "Option::is_none"`, so prose — most of the 442 — adds nothing to a document
every visitor downloads. That is the same argument the `description` field's comment makes for
staying index-only, landing the other way: a summary the client already has on the payload; "is
this a problem" it cannot know at all without asking for all of them.

**The trap.** `BookEntryDto` is `#[serde(tag = "kind")]` — internally tagged. A `LessonDto` field
named `kind` emits the key **twice**, and `serde_derive` cannot catch it, because it cannot see
inside the newtype. It degrades into duplicate JSON keys on the index, silently.

So the field is `lesson_kind` (wire `lessonKind`), and the step's first action — before the server
was touched — was the round-trip test asserting `"kind"` appears exactly once. It was then proven
by temporarily renaming the field back and watching it produce precisely the predicted damage:

```
{"kind":"lesson","slug":"two-sum",…,"kind":"problem"}
```

A test that has never failed is a guess about what it checks.

## Every problem is its own chapter, and the counter had to know

`chapter_problems` scopes on paths rather than the tree, because `reading_order` flattens chapters
away: a lesson's chapter is its path minus the last segment, and two lessons are siblings when
those parents match.

That was wrong on the first run, and the browser said so — the bar rendered with an empty centre.
The real content authors a problem as `problems/<slug>/<slug>.md`, so every problem sits in a
chapter containing only itself. Raw-parent scoping gives "Problem 1 / 1" on every page, and the
version before the flattening fix returned `None` outright against a stale index.

The codebase had already met this shape: the sidebar flattens a chapter whose only child is a
lesson of the same slug — 36 of this content's 61 chapters. `counting_chapter` now agrees with it.
When the parent segment and the lesson slug match, the chapter is one level up.

**The arrows stay on the server's reading order**, over all lessons rather than problems. The dots
and the arrows answer different questions: the dots say where you are in this problem set, the
arrows say where you go next in the book. At a chapter edge "Problem 1 / 6" beside a Previous
pointing at prose is coherent rather than contradictory — and scoping the arrows to problems would
dead-end the learner at every boundary.

## Below 1024px the bar has to be sticky, not in flow

The ≤1023px block gives `.pwb` `height: auto` and `overflow: visible`, so the phone layout
**window-scrolls**. An in-flow bar there would sit below the fold for the entire scroll — which is
the one thing the old fixed pills got right. It becomes `position: sticky; bottom: 0` with a
blurred translucent strip, `margin: 0 -14px` to escape `.pwb`'s padding (the idiom `.pwb-ejump`
already uses).

Sticky rather than fixed because sticky *releases* at the true bottom of the page, where a fixed
bar would keep hovering over the final content. It works only because `.pwb` has no `overflow` —
one such declaration would kill the pinning with no error at all, so the note lives on that rule.

The rules go **inside** the existing `@media (max-width: 1023px)` block, whose own comment records
that it is last in the file on purpose. The ≤640px rules are deliberately disjoint from it: they
touch `flex-wrap`, `order` and `display` only, never `position`/`bottom`/`background`, which the
later block would win at equal specificity.

`.pwb`'s `min-height` went 480 → 544, paying back exactly the height the bar takes so short desktop
viewports keep today's pane budget.

On phones the step *titles* go and the eyebrow stays; Contents drops to its icon. That reverses
step 43's rule, deliberately — the ambiguity that kept its word came from a lone icon floating over
the panes with nothing around it, and inside a labelled bar beside a counter it is gone.

## Verified

Gates: conventions, fmt, clippy, **472 rust** (+9) + 83 vitest.

Live, on real content: "Problem 3 / 7" with seven dots and the third elongated, Previous "If Else
Adult Teen Problem" and Next "Switch Case"; Contents opens the drawer with all 72 entries; at
1024px the bar is `static` and in flow, at 1023px `sticky` and pinned to the viewport bottom,
releasing at the true end; at 375px titles drop, Contents goes icon-only, the counter takes its own
row and `scrollWidth == 375`; prose lessons have no bar and their pager cards are untouched;
console clean.

## The lesson

**The browser found the bug the tests could not.** `chapter_problems` had six native tests and
passed all of them — against fixtures I wrote, in the shape I assumed content took. The real
content puts each problem in a directory of its own, which no fixture did, and the first render
showed an empty centre. The seventh test exists now because the page told me what the content
actually looks like. Fixtures encode the author's assumptions; a running page encodes the content's.
