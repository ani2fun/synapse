# Step 41 — Code blocks: tab groups

*(a design-led change: adjacent code fences in different languages become one card with
language tabs, and every code block gains the header bar that used to be a button floating
over its first line.)*

## What prompted it

`synapse-content` grew Python translations beside its Java examples (66 blocks). Where the
two fences sit directly adjacent, the reader stacked them as two unrelated dark slabs, each
carrying its own **Try in Editor** button pinned over the top-right of the code. Two problems
in one picture: nothing said *these are the same idea in two languages*, and the action
covered the first line it sat on.

The design — *Code Blocks - Tab Groups*, authored in Claude Design and imported here — answers
both with one frame: a slim dark header bar carrying language **tabs** on the left and the
actions on the far right, over a single slab that swaps as you switch. A lone block keeps the
same bar with a `▶ Java` pill instead of tabs, so every code block on the site reads the same
way and the action never overlaps code again.

## The grouping rule

A **fence group** is a run of one or more *plain* fences that are adjacent siblings in the
markdown AST — nothing between them but blank lines. A fence joins when all hold:

- it has a language tag. Bare ``` fences are excluded outright: there are 914 of them in the
  corpus and most are the program output printed under a `run` block. Nothing to name, nothing
  to run, and a header bar would only add weight to a two-line result.
- no other transform claims it. `run`, `solution`, `problem`, `testcases`, `editorial`,
  `quiz`, `viz`, `mermaid`, `d2` all return earlier in the handler, so they are excluded
  structurally rather than by a list that could drift.
- its language is **distinct** from the ones already in the group. A repeat breaks the run —
  two Java fences cannot both be the Java tab.

The predicate (`isPlainFence`) gates the group **head as well as its siblings**, so a fence
either always gets the card or never does. That symmetry is what keeps orphans honest: a
`testcases` fence with no group above it, or a `viz` fence with no `widget=`, still renders as
bare highlighted code exactly as before.

**Two adjacent fences in unrelated languages will group** — `java` then `bash` would become
two tabs. That is deliberate, and identical to how `run`, `solution` and `d2` grouping already
behave. The escape hatch is the one authors already know: a line of prose between the fences
breaks the run. There are no such pairs in the corpus today.

## The pipeline half — a grouper that keeps its output

`render.ts`'s `code` handler gained a final branch beside the existing groupers, reusing the
same `CONSUMED` marker. It emits:

```html
<div class="fence-group" data-langs="java,python">
  <div class="fence-group__bar"></div>
  <figure data-rehype-pretty-code-figure><pre data-language="java">…</pre></figure>
  <figure data-rehype-pretty-code-figure><pre data-language="python">…</pre></figure>
</div>
```

**Unlike every other grouper, this one keeps its fences' rendered output** instead of
swallowing it into a `data-*` payload. `defaultHandlers.code` still runs per member, and
`rehypePrettyCode` — which walks the whole tree, not just top-level children — highlights each
nested `<pre>` in place. So there is no second highlighting path to keep honest, no
`highlightCode` call at mount, and no re-render when a tab changes.

Two small decisions make the client half trivial:

- the empty `.fence-group__bar` is emitted **first**, because Leptos' `mount_to` appends — the
  bar lands above the panes without a CSS reordering hack;
- `data-langs` is a plain comma list, not `encodeURIComponent(JSON…)`. Every other placeholder
  carries an encoded payload because it is *empty*; this one is not — the code is right there
  in the `<pre>`s.

## The client half

`hydrate_codebench_pills` is retired; `hydrate_fence_groups` (`execution/view/fence_group.rs`)
replaces it at the same four call sites. Per `.fence-group` it reads each pane straight off the
rendered figure — `data-language` for the alias, `pre.text_content()` for the source, the same
seam the old floating pill used — and mounts the bar.

The bar holds one piece of state, `active: RwSignal<usize>`. The panes are plain DOM, so an
effect toggles `fence-group__pane--hidden` on the others: mount-once, one visible, the practice
widget's pattern. Copy is the workbench's `copy_button` minus the Monaco buffer (a fence's
source is fixed), check-swap for 1.4 s. **Try in Editor follows the selected tab** and appears
only when `runnable_fence()` accepts that language — which is why the TypeScript side needs no
copy of the server's alias list: `RUNNABLE_FENCES` stays in Rust and the pipeline stays
language-agnostic. `bash`, `toml`, `yaml` and friends get the bar and the copy button, and no
Run affordance they could not honour.

The tab buttons carry `aria-label` and `aria-pressed`: a glyph plus a text span left them
unnamed in the accessibility tree, with nothing announcing which language was showing.

## One CSS exception, stated out loud

`.fence-group` deliberately **does not carry `not-prose`**, and it is the only hydrated widget
that doesn't. Every other one owns its own rendering and needs to escape prose styling; here
the panes *are* `<pre>` elements and we want `.synapse-prose pre:not(.not-prose *)` to keep
painting the slab and the shiki token variables. Without the exception the card would render
its code unstyled. The rule is commented in place, because it reads as an oversight otherwise.

The slab has been fixed-dark in both themes since step 25, so the bar is too — and the accent
is pinned to the **dark** `--primary` (`hsl(166 63% 48%)`) rather than following the page. A
page-following accent goes nearly invisible on a light page, because the bar underneath it is
still `#262626`. Verified both ways: on the warm paper background the card reads as a dark
island with an unchanged teal accent.

Also swept: the `<pre>`'s own `0.5rem` radius is zeroed inside the frame (the card owns the
corners now), and the slab's horizontal scrollbar is styled to the design's values — the
browser default is a pale ghost that reads as damage against `#2d2d2d`.

Step 30's `.wb__lang-tab*` segmented control was deleted from `practice.css`. Step 38 replaced
it with the `▶` pill + dropdown and it had been dead since; leaving it would have meant three
tab styles in the tree, one of them live. **The runnable workbench toolbar is unchanged** — the
design treats it as the reference chrome these plain blocks are matching, not a thing to
change.

## What the tests pin

Eleven new vitest cases (`render.test.ts`), modelled on the existing d2/run adjacency pairs:
grouping and order, prose breaking the group, the lone-fence case, the repeated-language rule,
a bare fence staying bare, a bare fence not joining the tagged fence above it (the 107
`python run` + output shape), `run` fences still becoming workbenches, `mermaid` keeping its
own placeholder, and orphan `testcases` / bare `viz` staying outside the card. The existing
shiki assertions had to keep passing unchanged — nesting the `<pre>` must not disturb them.

## Verified live

On the real singleton lesson: 23 cards, exactly one of them two-tab (`java,python`), 22 lone
pills, zero floating buttons left. Switching to Python swapped the slab with the actions
holding still; **Try in Editor opened the codebench on Python, not Java** — the selected tab,
which is the whole point of moving the button into the bar. Copy flipped to the check and
reverted cleanly. On `java-basics`, all 32 cards hydrated including 6 inside practice widgets,
with the 3 workbenches and 5 mermaid diagrams untouched. On a `python run` lesson: 9
workbenches, 0 cards, and 11 bare `plaintext` output figures with no bar — the guard that
matters most. `bash` and `toml` blocks took the bar and copy without a Run affordance.

Critical path 634/700 KiB gz (+6). 371 rust + 74 vitest.

## The two lessons worth keeping

**A grouper does not have to swallow its members.** Every earlier one emits an empty
placeholder and hands a payload to the client, because the client re-renders that content as a
widget. This one only needed *chrome*, so wrapping the already-rendered fences was both less
code and strictly better output — highlighting stays in one place and tab switching costs a
class toggle.

**Adjacency is the author's escape hatch, and it should stay that way.** The temptation was to
special-case which language pairs "should" group. Grouping breaks on any intervening node,
which is a rule authors can see in their own file and control with a blank line's worth of
prose — no vocabulary to learn and no list in the code to keep current.
